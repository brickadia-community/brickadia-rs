use std::fs::File;

use brickadia::read::{ReadError, SaveReader};

fn main() -> Result<(), ReadError> {
    let mut reader = SaveReader::new(File::open("examples/read.brs")?)?;
    println!("Initialized reader, version: {}", reader.version);
    let header1 = reader.read_header1()?;
    println!("Read header 1: {:?}\n", header1);
    let header2 = reader.read_header2()?;
    println!("Read header 2: {:?}\n", header2);
    let preview = reader.read_preview()?;
    println!("Read preview, present? {}", if preview.is_some() { "yes" } else { "no" });
    let (bricks, components) = reader.read_bricks(&header1, &header2)?;
    println!("Read bricks:");
    for brick in bricks {
        println!("{:?}", brick);
    }

    Ok(())
}
