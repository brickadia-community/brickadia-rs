use std::{env, fs::File};

use brickadia::read::SaveReader;

fn main() {
    let read_location = env::args().nth(1).unwrap_or("examples/read.brs".into());
    let mut reader = SaveReader::new(File::open(read_location).unwrap()).unwrap();
    let save = reader.read_all().unwrap();
    println!("Serialized: {}", serde_json::to_string(&save).unwrap());
}
