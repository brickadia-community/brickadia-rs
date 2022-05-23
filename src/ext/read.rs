use std::{
    cmp,
    io::{self, Read, Result},
};

use bitstream_io::BitRead;
use byteorder::{BigEndian, ByteOrder, LittleEndian, ReadBytesExt};
use uuid::Uuid;

use crate::save::{Color, UnrealType};

pub trait ReadExt: Read {
    fn read_string(&mut self) -> Result<String> {
        match self.read_i32::<LittleEndian>()? {
            size if size >= 0 => {
                let mut chars = vec![0u8; cmp::max(0, size - 1) as usize];
                self.read_exact(&mut chars)?;
                if size > 0 {
                    self.read_u8()?;
                } // read a null terminator
                String::from_utf8(chars)
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid string data"))
            }
            size if size < 0 => {
                let size = -size;
                match size % 2 {
                    0 => {
                        let mut chars = vec![0; size as usize / 2];
                        self.read_u16_into::<LittleEndian>(&mut chars)?;
                        String::from_utf16(&chars).map_err(|_| {
                            io::Error::new(io::ErrorKind::InvalidData, "invalid UCS-2 string data")
                        })
                    }
                    1 => Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "invalid UCS-2 size",
                    )),
                    _ => unreachable!(),
                }
            }
            _ => unreachable!(),
        }
    }

    fn read_uuid(&mut self) -> Result<Uuid> {
        let mut le_bytes = [0; 4];
        self.read_u32_into::<LittleEndian>(&mut le_bytes)?;
        let mut bytes = [0u8; 16];
        BigEndian::write_u32_into(&le_bytes, &mut bytes);
        Ok(Uuid::from_bytes(bytes))
    }

    fn read_array<F, T>(&mut self, mut operation: F) -> Result<Vec<T>>
    where
        F: FnMut(&mut Self) -> Result<T>,
    {
        let len = self.read_i32::<LittleEndian>()?;
        let mut vec = Vec::with_capacity(len as usize);
        for _ in 0..len {
            vec.push(operation(self)?);
        }
        Ok(vec)
    }
}

impl<R> ReadExt for R where R: Read {}

pub trait BitReadExt: BitRead {
    fn read_array<F, T>(&mut self, mut operation: F) -> Result<Vec<T>>
    where
        F: FnMut(&mut Self) -> Result<T>,
    {
        let len = self.read_i32_le()?;
        let mut vec = Vec::with_capacity(len as usize);
        for _ in 0..len {
            vec.push(operation(self)?);
        }
        Ok(vec)
    }

    fn read_uint(&mut self, max: u32) -> Result<u32> {
        let mut value = 0;
        let mut mask = 1;

        while value + mask < max && mask != 0 {
            if self.read_bit()? {
                value |= mask;
            }
            mask <<= 1;
        }

        Ok(value)
    }

    fn read_uint_packed(&mut self) -> Result<u32> {
        let mut value = 0;

        for i in 0..5 {
            let has_next = self.read_bit()?;
            let mut part = 0;
            for shift in 0..7 {
                part |= (self.read_bit()? as u32) << shift;
            }
            value |= part << (7 * i);
            if !has_next {
                break;
            }
        }

        Ok(value)
    }

    fn read_int_packed(&mut self) -> Result<i32> {
        let value = self.read_uint_packed()?;
        Ok((value >> 1) as i32 * if value & 1 != 0 { 1 } else { -1 })
    }

    fn read_string(&mut self) -> Result<String> {
        match self.read_i32_le()? {
            size if size >= 0 => {
                let mut chars = vec![0u8; cmp::max(0, size - 1) as usize];
                self.read_bytes(&mut chars)?;
                if size > 0 {
                    self.read_bytes(&mut [0])?;
                } // read a null terminator
                String::from_utf8(chars)
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid string data"))
            }
            size if size < 0 => {
                let size = -size * 2;
                match size % 2 {
                    0 => {
                        let mut chars = vec![0; (size / 2) as usize];
                        self.read_u16_le_into(&mut chars)?;
                        String::from_utf16(&chars).map_err(|_| {
                            io::Error::new(io::ErrorKind::InvalidData, "invalid UCS-2 string data")
                        })
                    }
                    1 => Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "invalid UCS-2 size",
                    )),
                    _ => unreachable!(),
                }
            }
            _ => unreachable!(),
        }
    }

    fn read_i32_le(&mut self) -> Result<i32> {
        let mut bytes = [0u8; 4];
        self.read_bytes(&mut bytes)?;

        let mut value = 0i32;
        for (i, &byte) in bytes.iter().enumerate() {
            value |= (byte as i32) << (8 * i);
        }
        Ok(value)
    }

    fn read_u16_le(&mut self) -> Result<u16> {
        let mut bytes = [0u8; 2];
        self.read_bytes(&mut bytes)?;

        let mut value = 0u16;
        for (i, &byte) in bytes.iter().enumerate() {
            value |= (byte as u16) << (8 * i);
        }
        Ok(value)
    }

    fn read_u16_le_into(&mut self, slice: &mut [u16]) -> Result<()> {
        for elem in slice {
            *elem = self.read_u16_le()?;
        }
        Ok(())
    }

    fn read_f32_le(&mut self) -> Result<f32> {
        let mut bytes = [0u8; 4];
        self.read_bytes(&mut bytes)?;
        Ok(LittleEndian::read_f32(&bytes))
    }

    fn read_unreal_type(&mut self, t: &str) -> Result<UnrealType> {
        match t {
            "Class" | "Object" => Ok(UnrealType::Class(self.read_string()?)),
            "String" => Ok(UnrealType::String(self.read_string()?)),
            "Boolean" => Ok(UnrealType::Boolean(self.read_i32_le()? != 0)),
            "Float" => Ok(UnrealType::Float(self.read_f32_le()?)),
            "Color" => {
                let mut bytes = [0u8; 4];
                self.read_bytes(&mut bytes)?;
                Ok(UnrealType::Color(Color::from_bytes_bgra(bytes)))
            }
            "Byte" => {
                let mut byte = [0u8; 1];
                self.read_bytes(&mut byte)?;
                Ok(UnrealType::Byte(byte[0]))
            }
            "Rotator" => Ok(UnrealType::Rotator(
                self.read_f32_le()?,
                self.read_f32_le()?,
                self.read_f32_le()?,
            )),
            invalid => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid unreal type specified: {}", invalid),
            )),
        }
    }
}

impl<R> BitReadExt for R where R: BitRead {}
