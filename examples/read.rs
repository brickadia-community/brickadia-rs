use std::{env, fs::File, io::Write};

use brickadia::read::SaveReader;

fn main() {
    let read_location = env::args().nth(1).unwrap_or("examples/read.brs".into());
    let mut reader = SaveReader::new(File::open(read_location).unwrap()).unwrap();
    println!("Initialized reader, version: {}", reader.version);
    let header1 = reader.read_header1().unwrap();
    println!("Read header 1: {:?}\n", header1);
    let header2 = reader.read_header2().unwrap();
    println!("Read header 2: {:?}\n", header2);
    let preview = reader.read_preview().unwrap();
    println!(
        "Read preview, present? {}",
        if preview.is_some() { "yes" } else { "no" }
    );
    if preview.is_some() {
        let mut file = File::create("examples/save_preview.out.png").unwrap();
        file.write_all(&preview.unwrap()).unwrap();
        println!("Wrote preview to save_preview.out.png");
    }

    let (bricks, components) = reader.read_bricks(&header1, &header2).unwrap();
    println!("Read bricks:");
    for brick in bricks {
        println!("{:?}", brick);
    }

    println!("\nRead components:");
    for (name, component) in components {
        println!("{}: {:?}", name, component);
    }
}
