//! Save reading.

use std::{
    cmp,
    collections::HashMap,
    convert::TryFrom,
    io::{self, Cursor, Read},
};

use bitstream_io::{BitRead, BitReader};
use byteorder::{LittleEndian, ReadBytesExt};
use flate2::read::ZlibDecoder;
use thiserror::Error;

use crate::{ext::*, save::*, MAGIC_BYTES};

lazy_static::lazy_static! {
    static ref DEFAULT_MATERIALS: Vec<String> = vec!["BMC_Hologram", "BMC_Plastic", "BMC_Glow", "BMC_Metallic", "BMC_Glass"].into_iter().map(|s| s.into()).collect();
}

/// A read error.
#[derive(Error, Debug)]
pub enum ReadError {
    #[error("generic io error: {0}")]
    IoError(#[from] io::Error),
    #[error("bad magic bytes (expected 'BRS')")]
    BadHeader,
    #[error("invalid data in header 1")]
    InvalidDataHeader1,
    #[error("invalid data in header 2")]
    InvalidDataHeader2,
    #[error("must read in sequence: header 1, header 2, [preview], bricks")]
    BadSectionReadOrder,
    #[error("invalid compressed section")]
    InvalidCompression,
}

/// A save reader, which reads data from its `reader` (a `Read + Seek`).
pub struct SaveReader<R: Read> {
    reader: R,
    pub version: u16,
    pub game_version: i32,

    header1_read: bool,
    header2_read: bool,
    preview_read: bool,
}

impl<R: Read> SaveReader<R> {
    /// Create a new save reader from an existing `reader`, a `Read + Seek`.
    pub fn new(mut reader: R) -> Result<Self, ReadError> {
        let mut magic = [0u8; 3];
        reader.read_exact(&mut magic)?;
        if &magic != MAGIC_BYTES {
            return Err(ReadError::BadHeader);
        }

        let version = reader.read_u16::<LittleEndian>()?;
        let game_version = if version >= 8 {
            reader.read_i32::<LittleEndian>()?
        } else {
            0
        };

        Ok(SaveReader {
            version,
            game_version,
            reader,
            header1_read: false,
            header2_read: false,
            preview_read: version < 8,
        })
    }

    /// Skip the first header.
    pub fn skip_header1(&mut self) -> Result<(), ReadError> {
        skip_compressed(&mut self.reader)?;
        self.header1_read = true;
        Ok(())
    }

    /// Read the first header.
    pub fn read_header1(&mut self) -> Result<Header1, ReadError> {
        let (mut cursor, _) = read_compressed(&mut self.reader)?;

        // match map: a string
        let map = cursor.read_string()?;

        // match author name: a string
        let author_name = cursor.read_string()?;

        // match description: a string
        let description = cursor.read_string()?;

        // match author id: a uuid
        let author_uuid = cursor.read_uuid()?;

        // match host:
        // version >= 8: match a user (string followed by uuid)
        //         else: not provided
        let host = match self.version {
            _ if self.version >= 8 => {
                let name = cursor.read_string()?;
                let id = cursor.read_uuid()?;
                Some(User { name, id })
            }
            _ => None,
        };

        // match save time:
        // version >= 4: match 8 bytes
        //         else: not provided
        let save_time = match self.version {
            _ if self.version >= 4 => {
                let mut bytes = [0u8; 8]; // todo: figure out how to parse this
                cursor.read_exact(&mut bytes)?;
                Some(bytes)
            }
            _ => None,
        };

        // match brick count: an i32
        let brick_count = match cursor.read_i32::<LittleEndian>()? {
            count if count >= 0 => count,
            _ => return Err(ReadError::InvalidDataHeader1),
        } as u32;

        self.header1_read = true;
        Ok(Header1 {
            map,
            author: User {
                name: author_name,
                id: author_uuid,
            },
            description,
            host,
            save_time: save_time.unwrap_or([0u8; 8]),
            brick_count,
        })
    }

    /// Skip the second header.
    pub fn skip_header2(&mut self) -> Result<(), ReadError> {
        skip_compressed(&mut self.reader)?;
        self.header2_read = true;
        Ok(())
    }

    /// Read the second header.
    pub fn read_header2(&mut self) -> Result<Header2, ReadError> {
        if !self.header1_read {
            return Err(ReadError::BadSectionReadOrder);
        }

        let (mut cursor, _) = read_compressed(&mut self.reader)?;

        // match mods: an array of strings
        let mods = cursor.read_array(|r| r.read_string())?;

        // match brick assets: an array of strings
        let brick_assets = cursor.read_array(|r| r.read_string())?;

        // match colors: an array of 4 bytes each, BGRA
        let colors = cursor.read_array(|r| -> io::Result<Color> {
            let mut bytes = [0u8; 4];
            r.read_exact(&mut bytes)?;
            Ok(Color::from_bytes_bgra(bytes))
        })?;

        // match materials:
        // version >= 2: an array of strings
        //         else: a list of default materials (see top of file)
        let materials = match self.version {
            _ if self.version >= 2 => cursor.read_array(|r| r.read_string())?,
            _ => DEFAULT_MATERIALS.clone(),
        };

        // match brick owners:
        // version >= 3: match brick owner:
        //               version >= 8: a user (uuid followed by string), then an i32 for brick count
        //                       else: a user (uuid followed by string)
        let brick_owners = match self.version {
            _ if self.version >= 3 => cursor.read_array(|r| -> io::Result<BrickOwner> {
                match self.version {
                    _ if self.version >= 8 => {
                        let id = r.read_uuid()?;
                        let name = r.read_string()?;
                        let bricks = r.read_i32::<LittleEndian>()? as u32;
                        Ok(BrickOwner { name, id, bricks })
                    }
                    _ => {
                        let id = r.read_uuid()?;
                        let name = r.read_string()?;
                        Ok(BrickOwner::from(User { name, id }))
                    }
                }
            })?,
            _ => vec![],
        };

        // match physical materials
        // version >= 9: an array of strings
        //         else: not provided
        let physical_materials = match self.version {
            _ if self.version >= 9 => cursor.read_array(|r| r.read_string())?,
            _ => vec![],
        };

        self.header2_read = true;
        Ok(Header2 {
            mods,
            brick_assets,
            colors,
            materials,
            brick_owners,
            physical_materials,
        })
    }

    /// Read the preview in the save.
    ///
    /// The preview is an `Preview`, which might not exist (Preview::None).
    pub fn read_preview(&mut self) -> Result<Preview, ReadError> {
        if !self.header2_read {
            return Err(ReadError::BadSectionReadOrder);
        }

        if self.version < 8 {
            return Ok(Preview::None);
        }

        let preview = Preview::from_reader(&mut self.reader)?;
        self.preview_read = true;
        Ok(preview)
    }

    /// Skip over the preview section.
    pub fn skip_preview(&mut self) -> Result<(), ReadError> {
        if !self.header2_read {
            return Err(ReadError::BadSectionReadOrder);
        }

        if self.version < 8 {
            return Ok(());
        }

        if self.reader.read_u8()? != 0 {
            let len = self.reader.read_i32::<LittleEndian>()?;
            io::copy(&mut self.reader.by_ref().take(len as u64), &mut io::sink())?;
        }

        self.preview_read = true;
        Ok(())
    }

    /// Read the bricks and components from a save.
    pub fn read_bricks(
        &mut self,
        header1: &Header1,
        header2: &Header2,
    ) -> Result<(Vec<Brick>, HashMap<String, Component>), ReadError> {
        if !self.preview_read || !self.header2_read {
            return Err(ReadError::BadSectionReadOrder);
        }

        let (cursor, len) = read_compressed(&mut self.reader)?;
        let mut bits = BitReader::<_, bitstream_io::LittleEndian>::new(cursor);

        let brick_asset_count = cmp::max(header2.brick_assets.len(), 2);
        let material_count = cmp::max(header2.materials.len(), 2);
        let physical_material_count = cmp::max(header2.physical_materials.len(), 2);

        let inital_bricks_capacity = cmp::min(header1.brick_count as usize, 10_000_000);
        let mut bricks = Vec::with_capacity(inital_bricks_capacity);
        let mut components = HashMap::new();

        // loop over each brick
        loop {
            // align and break out of the loop if we've seeked far enough ahead
            bits.byte_align();
            if bricks.len() >= header1.brick_count as usize
                || bits.reader().unwrap().position() >= len as u64
            {
                break;
            }

            let asset_name_index = bits.read_uint(brick_asset_count as u32)?;

            let size = match bits.read_bit()? {
                true => Size::Procedural(
                    bits.read_uint_packed()?,
                    bits.read_uint_packed()?,
                    bits.read_uint_packed()?,
                ),
                false => Size::Empty,
            };

            let position = (
                bits.read_int_packed()?,
                bits.read_int_packed()?,
                bits.read_int_packed()?,
            );

            let orientation = bits.read_uint(24)?;
            let direction = Direction::try_from(((orientation >> 2) % 6) as u8).unwrap();
            let rotation = Rotation::try_from((orientation & 3) as u8).unwrap();

            let collision = match self.version {
                _ if self.version >= 10 => Collision {
                    player: bits.read_bit()?,
                    weapon: bits.read_bit()?,
                    interaction: bits.read_bit()?,
                    tool: bits.read_bit()?,
                },
                _ => Collision::for_all(bits.read_bit()?),
            };

            let visibility = bits.read_bit()?;

            let material_index = match self.version {
                _ if self.version >= 8 => bits.read_uint(material_count as u32)?,
                _ => {
                    if bits.read_bit()? {
                        bits.read_uint_packed()?
                    } else {
                        1
                    }
                }
            };

            let physical_index = match self.version {
                _ if self.version >= 9 => bits.read_uint(physical_material_count as u32)?,
                _ => 0,
            };

            let material_intensity = match self.version {
                _ if self.version >= 9 => bits.read_uint(11)?,
                _ => 5,
            };

            let color = match bits.read_bit()? {
                true => match self.version {
                    _ if self.version >= 9 => {
                        let mut bytes = [0u8; 3];
                        bits.read_bytes(&mut bytes)?;
                        BrickColor::Unique(Color::from_bytes_rgb(bytes))
                    }
                    _ => {
                        let mut bytes = [0u8; 4];
                        bits.read_bytes(&mut bytes)?;
                        BrickColor::Unique(Color::from_bytes_bgra(bytes))
                    }
                },
                false => BrickColor::Index(bits.read_uint(header2.colors.len() as u32)?),
            };

            let owner_index = if self.version >= 3 {
                bits.read_uint_packed()?
            } else {
                0
            };

            let brick = Brick {
                asset_name_index,
                size,
                position,
                direction,
                rotation,
                collision,
                visibility,
                material_index,
                physical_index,
                material_intensity,
                color,
                owner_index,
                components: HashMap::new(),
            };

            bricks.push(brick);
        }

        bricks.shrink_to_fit();
        let brick_count = cmp::max(bricks.len(), 2);

        // components
        if self.version >= 8 {
            let (mut cursor, _) = read_compressed(&mut self.reader)?;
            let len = cursor.read_i32::<LittleEndian>()?;

            for _ in 0..len {
                let name = cursor.read_string()?;

                let mut bit_bytes = vec![0u8; cursor.read_i32::<LittleEndian>()? as usize];
                cursor.read_exact(&mut bit_bytes)?;
                let mut bits =
                    BitReader::endian(Cursor::new(bit_bytes), bitstream_io::LittleEndian);

                let version = bits.read_i32_le()?;
                let brick_indices = bits.read_array(|r| r.read_uint(brick_count as u32))?;

                let properties = bits
                    .read_array(|r| Ok((r.read_string()?, r.read_string()?)))?
                    .into_iter()
                    .collect::<Vec<_>>();

                // components for each brick
                for &i in brick_indices.iter() {
                    let mut props = HashMap::new();
                    for (n, ty) in properties.iter() {
                        props.insert(n.to_owned(), bits.read_unreal_type(ty)?);
                    }
                    bricks[i as usize].components.insert(name.to_owned(), props);
                }

                components.insert(
                    name,
                    Component {
                        version,
                        brick_indices,
                        properties: properties.into_iter().collect(),
                    },
                );
            }
        }

        Ok((bricks, components))
    }

    /// Read all parts of a save into a `SaveData`.
    pub fn read_all(&mut self) -> Result<SaveData, ReadError> {
        let header1 = self.read_header1()?;
        let header2 = self.read_header2()?;
        let preview = self.read_preview()?;
        let (bricks, components) = self.read_bricks(&header1, &header2)?;

        Ok(SaveData {
            version: self.version,
            game_version: self.game_version,
            header1,
            header2,
            preview,
            bricks,
            components,
        })
    }

    /// Read all parts of a save (except the preview) into a `SaveData`.
    pub fn read_all_skip_preview(&mut self) -> Result<SaveData, ReadError> {
        let header1 = self.read_header1()?;
        let header2 = self.read_header2()?;
        self.skip_preview()?;
        let (bricks, components) = self.read_bricks(&header1, &header2)?;

        Ok(SaveData {
            version: self.version,
            game_version: self.game_version,
            header1,
            header2,
            preview: Preview::None,
            bricks,
            components,
        })
    }
}

/// Read a compressed section from a `Read`, following the BRS spec for compressed sections.
fn read_compressed(reader: &mut impl Read) -> Result<(Cursor<Vec<u8>>, i32), ReadError> {
    let (uncompressed_size, compressed_size) = (
        reader.read_i32::<LittleEndian>()?,
        reader.read_i32::<LittleEndian>()?,
    );
    if uncompressed_size < 0 || compressed_size < 0 || compressed_size > uncompressed_size {
        return Err(ReadError::InvalidCompression);
    }

    let mut bytes = vec![0u8; uncompressed_size as usize];

    if compressed_size == 0 {
        // no need to decompress first
        reader.read_exact(&mut bytes)?;
    } else {
        // decompress first, then read
        let mut compressed = vec![0u8; compressed_size as usize];
        reader.read_exact(&mut compressed)?;
        ZlibDecoder::new(&compressed[..]).read_exact(&mut bytes)?;
    }

    Ok((Cursor::new(bytes), uncompressed_size))
}

/// Read a compressed section from a `Read`, discarding its contents.
fn skip_compressed(reader: &mut impl Read) -> Result<(), ReadError> {
    let (uncompressed_size, compressed_size) = (
        reader.read_i32::<LittleEndian>()?,
        reader.read_i32::<LittleEndian>()?,
    );
    if uncompressed_size < 0 || compressed_size < 0 || compressed_size > uncompressed_size {
        return Err(ReadError::InvalidCompression);
    }

    io::copy(
        &mut reader.take(if compressed_size == 0 {
            uncompressed_size as u64
        } else {
            compressed_size as u64
        }),
        &mut io::sink(),
    )?;

    Ok(())
}
