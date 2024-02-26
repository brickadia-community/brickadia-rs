use std::io::{self, Write};

use bitstream_io::BitWrite;
use byteorder::{BigEndian, ByteOrder, LittleEndian, WriteBytesExt};
use chrono::prelude::*;
use uuid::Uuid;

use crate::save::{Color, UnrealType};

pub trait WriteExt: Write {
    fn write_string(&mut self, string: String) -> io::Result<()> {
        if string.is_empty() {
            // write out a 0 and nothing else
            self.write_i32::<LittleEndian>(0)?;
            return Ok(());
        }

        if string.is_ascii() {
            // write utf-8: positive length
            self.write_i32::<LittleEndian>(string.len() as i32 + 1)?;
            self.write_all(string.as_bytes())?;
            self.write_u8(0)?; // write a null terminator
            Ok(())
        } else {
            // write ucs-2: negative length
            self.write_i32::<LittleEndian>(-(string.len() as i32))?;
            string
                .encode_utf16()
                .try_for_each(|c| self.write_u16::<LittleEndian>(c))?;
            self.write_u8(0)?; // write a null terminator
            Ok(())
        }
    }

    fn write_uuid(&mut self, uuid: Uuid) -> io::Result<()> {
        let mut bytes = [0; 4];
        BigEndian::read_u32_into(uuid.as_bytes(), &mut bytes);
        for &e in bytes.iter() {
            self.write_u32::<LittleEndian>(e)?;
        }

        Ok(())
    }

    fn write_datetime(&mut self, datetime: Option<DateTime<Utc>>) -> io::Result<()> {
        let datetime = match datetime {
            Some(datetime) => datetime,
            None => Utc::now(),
        };
        let epoch = Utc.with_ymd_and_hms(1, 1, 1, 0, 0, 0).unwrap();
        let duration = datetime - epoch;
        let ticks_secs = i64::try_from(duration.num_seconds() * 10_000_000).unwrap();
        let ticks_nanos = i64::from(duration.subsec_nanos() / 100);
        self.write_i64::<LittleEndian>(ticks_secs + ticks_nanos)?;
        Ok(())
    }

    fn write_color_bgra(&mut self, color: Color) -> io::Result<()> {
        self.write_u8(color.b)?;
        self.write_u8(color.g)?;
        self.write_u8(color.r)?;
        self.write_u8(color.a)?;
        Ok(())
    }

    fn write_array<F: FnMut(&mut Self, T) -> io::Result<()>, T>(
        &mut self,
        vec: Vec<T>,
        mut operation: F,
    ) -> io::Result<()> {
        self.write_i32::<LittleEndian>(vec.len() as i32)?;
        for item in vec.into_iter() {
            operation(self, item)?;
        }
        Ok(())
    }
}

impl<W> WriteExt for W where W: Write {}

pub trait BitWriteExt: BitWrite {
    fn write_i32(&mut self, i: i32) -> io::Result<()> {
        let mut bytes = [0u8; 4];
        LittleEndian::write_i32(&mut bytes, i);
        self.write_bytes(&bytes)
    }

    fn write_u16(&mut self, i: u16) -> io::Result<()> {
        let mut bytes = [0u8; 2];
        LittleEndian::write_u16(&mut bytes, i);
        self.write_bytes(&bytes)
    }

    fn write_string(&mut self, string: String) -> io::Result<()> {
        if string.is_empty() {
            self.write_i32(0)?;
            return Ok(());
        }

        if string.is_ascii() {
            // write utf-8: positive length
            self.write_i32(string.len() as i32 + 1)?;
            self.write_bytes(string.as_bytes())?;
            self.write_bytes(&[0])?; // write a null terminator
            Ok(())
        } else {
            // write ucs-2: negative length
            self.write_i32(-(string.len() as i32))?;
            string.encode_utf16().try_for_each(|c| self.write_u16(c))?;
            self.write_bytes(&[0])?; // write a null terminator
            Ok(())
        }
    }

    fn write_uint(&mut self, value: u32, max: u32) -> io::Result<()> {
        assert!(max >= 2);

        if value >= max {
            return Err(io::Error::from(io::ErrorKind::InvalidInput));
        }

        let mut new_value = 0;
        let mut mask = 1;

        while new_value + mask < max && mask != 0 {
            self.write_bit(value & mask != 0)?;
            if value & mask != 0 {
                new_value |= mask;
            }
            mask <<= 1;
        }

        Ok(())
    }

    fn write_bits(&mut self, src: &[u8], len: usize) -> io::Result<()> {
        for bit in 0..len {
            self.write_bit((src[bit >> 3] & (1 << (bit & 7))) != 0)?;
        }
        Ok(())
    }

    fn write_uint_packed(&mut self, mut value: u32) -> io::Result<()> {
        loop {
            let src = [(value & 0b111_1111) as u8];
            value >>= 7;
            self.write_bit(value != 0)?;
            self.write_bits(&src, 7)?;
            if value == 0 {
                break;
            }
        }
        Ok(())
    }

    fn write_int_packed(&mut self, value: i32) -> io::Result<()> {
        self.write_uint_packed((value.unsigned_abs() << 1) | if value >= 0 { 1 } else { 0 })
    }

    fn write_f32(&mut self, value: f32) -> io::Result<()> {
        let mut bytes = [0u8; 4];
        LittleEndian::write_f32(&mut bytes, value);
        self.write_bytes(&bytes)
    }

    fn write_array<F: FnMut(&mut Self, &T) -> io::Result<()>, T>(
        &mut self,
        vec: &[T],
        mut operation: F,
    ) -> io::Result<()> {
        let mut len_bytes = [0u8; 4];
        LittleEndian::write_i32(&mut len_bytes, vec.len() as i32);
        self.write_bytes(&len_bytes)?;

        for item in vec {
            operation(self, item)?;
        }
        Ok(())
    }

    fn write_unreal(&mut self, unreal: UnrealType) -> io::Result<()> {
        match unreal {
            UnrealType::Boolean(bool) => self.write_i32(if bool { 1 } else { 0 })?,
            UnrealType::Byte(byte) => self.write_bytes(&[byte])?,
            UnrealType::Class(str) => self.write_string(str)?,
            UnrealType::String(str) => self.write_string(str)?,
            UnrealType::Color(color) => self.write_bytes(&[color.b, color.g, color.r, color.a])?,
            UnrealType::Float(float) => self.write_f32(float)?,
            UnrealType::Rotator(x, y, z) => {
                self.write_f32(x)?;
                self.write_f32(y)?;
                self.write_f32(z)?;
            }
        }
        Ok(())
    }
}

impl<W> BitWriteExt for W where W: BitWrite {}
