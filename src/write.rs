use std::{
    cmp,
    collections::{hash_map::Entry, HashMap},
    io::{self, Write},
};

use bitstream_io::{BitWrite, BitWriter};
use byteorder::{LittleEndian, WriteBytesExt};
use flate2::{write::ZlibEncoder, Compression};
use thiserror::Error;

use crate::{
    ext::write::*,
    save::{BrickColor, SaveData, Size, UnrealType},
    MAGIC_BYTES, SAVE_VERSION,
};

/// A write error.
#[derive(Error, Debug)]
pub enum WriteError {
    #[error("generic io error")]
    IoError(#[from] io::Error),
    #[error("brick is missing a component property")]
    ComponentBrickError,
}

/// A save writer, which writes its `data` to its `writer` (a `Write`).
pub struct SaveWriter<W: Write> {
    writer: W,
    data: SaveData,
    compressed: bool,
}

impl<W: Write> SaveWriter<W> {
    pub fn new(writer: W, data: SaveData) -> SaveWriter<W> {
        SaveWriter { writer, data, compressed: true }
    }

    pub fn uncompressed(writer: W, data: SaveData) -> SaveWriter<W> {
        SaveWriter { writer, data, compressed: false }
    }

    pub fn write(mut self) -> Result<(), WriteError> {
        // write header 0
        {
            self.writer.write_all(&MAGIC_BYTES)?;
            self.writer.write_u16::<LittleEndian>(SAVE_VERSION)?;
            self.writer
                .write_i32::<LittleEndian>(self.data.game_version)?;
        }

        let brick_count = self.data.bricks.len();
        let asset_name_count = cmp::max(self.data.header2.brick_assets.len(), 2);
        let material_count = cmp::max(self.data.header2.materials.len(), 2);
        let physical_material_count = cmp::max(self.data.header2.physical_materials.len(), 2);
        let color_count = cmp::max(self.data.header2.colors.len(), 2);

        // write header 1
        {
            // this Vec<u8> will store the bytes to the header, and eventually
            // will be compressed when necessary
            let mut w: Vec<u8> = vec![];
            w.write_string(self.data.header1.map)?;
            w.write_string(self.data.header1.author.name.to_owned())?;
            w.write_string(self.data.header1.description)?;
            w.write_uuid(self.data.header1.author.id)?;

            // if the host is None, then we assume it to be the
            // same as the author. can safely write the same value
            let host = self.data.header1.host.unwrap_or(self.data.header1.author);
            w.write_string(host.name)?;
            w.write_uuid(host.id)?;

            w.write_all(&self.data.header1.save_time)?;
            w.write_i32::<LittleEndian>(self.data.bricks.len() as i32)?;

            write_compressed(&mut self.writer, w, self.compressed)?;
        }

        // write header 2
        {
            // see above for compression methods
            let mut w: Vec<u8> = vec![];

            w.write_array(self.data.header2.mods, |writer, string| {
                writer.write_string(string)
            })?;

            w.write_array(self.data.header2.brick_assets, |writer, string| {
                writer.write_string(string)
            })?;

            w.write_array(self.data.header2.colors, |writer, color| {
                writer.write_color_bgra(color)
            })?;

            w.write_array(self.data.header2.materials, |writer, string| {
                writer.write_string(string)
            })?;

            w.write_array(
                self.data.header2.brick_owners,
                |writer, brick_owner| -> io::Result<()> {
                    writer.write_uuid(brick_owner.id)?;
                    writer.write_string(brick_owner.name)?;
                    writer.write_i32::<LittleEndian>(brick_owner.bricks as i32)?;
                    Ok(())
                },
            )?;

            w.write_array(self.data.header2.physical_materials, |writer, string| {
                writer.write_string(string)
            })?;

            write_compressed(&mut self.writer, w, self.compressed)?;
        }

        // write preview
        {
            let preview_type = self.data.preview.type_byte();
            self.writer.write_u8(preview_type)?;
            match preview_type {
                0 => (),
                _ => {
                    let bytes = self.data.preview.unwrap();
                    self.writer.write_i32::<LittleEndian>(bytes.len() as i32)?;
                    self.writer.write_all(&bytes)?
                }
            }
        }

        // write bricks and components
        {
            let mut vec = vec![];
            let mut bits = BitWriter::endian(&mut vec, bitstream_io::LittleEndian);

            let mut component_bricks: HashMap<String, Vec<(u32, HashMap<String, UnrealType>)>> =
                HashMap::new();

            for (i, brick) in self.data.bricks.into_iter().enumerate() {
                bits.byte_align()?;

                // write asset name index: <asset_name_index: u32; N>
                bits.write_uint(brick.asset_name_index, asset_name_count as u32)?;

                // write brick size:
                // <procedural?: bit>[x: uint_packed][y: uint_packed][z: uint_packed]
                match brick.size {
                    Size::Procedural(x, y, z) => {
                        bits.write_bit(true)?;
                        bits.write_uint_packed(x)?;
                        bits.write_uint_packed(y)?;
                        bits.write_uint_packed(z)?;
                    }
                    Size::Empty => bits.write_bit(false)?,
                }

                // write position:
                // <x: int_packed><y: int_packed><z: int_packed>
                bits.write_int_packed(brick.position.0)?;
                bits.write_int_packed(brick.position.1)?;
                bits.write_int_packed(brick.position.2)?;

                // write orientation: <orientation: uint; 24>
                let orientation = ((brick.direction as u32) << 2) | (brick.rotation as u32);
                bits.write_uint(orientation, 24)?;

                // write collision bits:
                // <player: bit><weapon: bit><interaction: bit><tool: bit>
                bits.write_bit(brick.collision.player)?;
                bits.write_bit(brick.collision.weapon)?;
                bits.write_bit(brick.collision.interaction)?;
                bits.write_bit(brick.collision.tool)?;

                // write visibility: <visibility: bit>
                bits.write_bit(brick.visibility)?;

                // write material index: <material_index: u32; N>
                bits.write_uint(brick.material_index, material_count as u32)?;

                // write physical index: <physical_index: u32; N>
                bits.write_uint(brick.physical_index, physical_material_count as u32)?;

                // write material intensity: <material_intensity: u32; 11>
                bits.write_uint(brick.material_intensity, 11)?;

                // write color:
                // <unique?: bit 0><index: uint; N> OR
                // <unique?: bit 1><r: byte><g: byte><b: byte>
                match brick.color {
                    BrickColor::Index(ind) => {
                        bits.write_bit(false)?;
                        bits.write_uint(ind, color_count as u32)?;
                    }
                    BrickColor::Unique(color) => {
                        bits.write_bit(true)?;
                        let bytes = [color.r, color.g, color.b];
                        bits.write_bytes(&bytes)?;
                    }
                }

                // write owner index: <owner_index: uint packed>
                bits.write_uint_packed(brick.owner_index)?;

                for (key, props) in brick.components.into_iter() {
                    let entry = (i as u32, props);

                    match component_bricks.entry(key) {
                        Entry::Occupied(mut v) => {
                            v.get_mut().push(entry);
                        }
                        Entry::Vacant(v) => {
                            v.insert(vec![entry]);
                        }
                    }
                }
            }

            bits.byte_align()?;

            write_compressed(&mut self.writer, vec, self.compressed)?;

            let mut vec: Vec<u8> = vec![];
            vec.write_i32::<LittleEndian>(self.data.components.len() as i32)?;

            for (name, component) in self.data.components.into_iter() {
                vec.write_string(name.to_owned())?;

                let mut bits = BitWriter::endian(vec, bitstream_io::LittleEndian);

                // write version
                bits.write_i32(component.version)?;

                // write brick indices
                if let Some(brick_list) = component_bricks.get(name.as_str()) {
                    bits.write_array(brick_list, |writer, (i, _)| {
                        writer.write_uint(*i, cmp::max(brick_count as u32, 2))
                    })?;
                } else {
                    bits.write_i32(0)?;
                }

                // write properties
                let properties = component.properties.into_iter().collect::<Vec<_>>();

                bits.write_array(&properties, |writer, (key, val)| -> io::Result<()> {
                    writer.write_string(key.clone())?;
                    writer.write_string(val.clone())?;
                    Ok(())
                })?;

                // read brick indices
                if let Some(brick_list) = component_bricks.remove(name.as_str()) {
                    for (_, mut props) in brick_list.into_iter() {
                        for (p, _) in properties.iter() {
                            bits.write_unreal(
                                props.remove(p).ok_or(WriteError::ComponentBrickError)?,
                            )?;
                        }
                    }
                }

                bits.byte_align()?;
                vec = bits.into_writer();
            }

            write_compressed(&mut self.writer, vec, self.compressed)?;
        }

        Ok(())
    }
}

/// Write a `Vec<u8>` out to a `Write`, following the BRS spec for compression.
fn write_compressed(writer: &mut impl Write, vec: Vec<u8>, should_compress: bool) -> io::Result<()> {
    if !should_compress {
        writer.write_i32::<LittleEndian>(vec.len() as i32)?;
        writer.write_i32::<LittleEndian>(0)?;
        writer.write_all(&vec[..])?;
        return Ok(());
    }

    let compressed = ZlibEncoder::new(vec.clone(), Compression::default()).finish()?;

    writer.write_i32::<LittleEndian>(vec.len() as i32)?;

    if compressed.len() < vec.len() {
        // compressed is smaller, write (unc_size: i32, c_size: i32, bytes)
        writer.write_i32::<LittleEndian>(compressed.len() as i32)?;
        writer.write_all(&compressed[..])?;
    } else {
        // write uncompressed (unc_size: i32, c_size: i32 = 0, bytes)
        writer.write_i32::<LittleEndian>(0)?;
        writer.write_all(&vec[..])?;
    }

    Ok(())
}
