mod ext;
pub mod read;
pub mod save;
pub mod write;

static MAGIC_BYTES: [u8; 3] = [b'B', b'R', b'S'];
static SAVE_VERSION: u16 = 10;
