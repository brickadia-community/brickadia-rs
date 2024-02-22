//! A library that supports [reading](crate::read::SaveReader) and
//! [writing](crate::write::SaveWriter) [Brickadia](https://brickadia.com/)
//! [save files](crate::save::SaveData).

#[allow(clippy::type_complexity)]
mod ext;
pub mod read;
pub mod save;
pub mod write;

#[cfg(feature = "util")]
pub mod util;

static MAGIC_BYTES: &[u8; 3] = b"BRS";

/// The current save version that can be read by brickadia-rs.
pub static SAVE_VERSION: u16 = 10;
