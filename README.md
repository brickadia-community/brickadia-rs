# brickadia-rs

A Brickadia save file (.brs) reader/writer library for Rust.

Supports save versions <= 10.

### Features

This library serves as a replacement for the officially supported [brs](https://github.com/brickadia/brs)
library, which at the time of writing can only read/write saves <= version 4.

`brickadia-rs` currently supports missing features from `brs` like save previews, components, brick owners,
and so on. It will also be maintained as the `.brs` spec continues to change in the future.

#### Serde support

By using the optional feature `serialize`, you can seamlessly serialize/deserialize into/from the
[brs-js](https://github.com/brickadia-community/brs-js) JSON spec.

## Installation

Add the following to your `Cargo.toml`'s dependencies:

```toml
brickadia = "0.1"
```

## Usage

### Reading

Below will read the file `my_brs_file.brs` and display brick count, map, and list each brick's position.

```rs
use std::fs;

use brickadia::read::SaveReader;

fn main() {
    let mut reader = SaveReader::new(fs::File::open("my_brs_file.brs").unwrap()).unwrap();
    let save = reader.read_all().unwrap();

    println!("Brick count: {}", save.header1.brick_count);
    println!("Map: {}", save.header1.map);

    for brick in save.bricks.iter() {
        println!("There's a brick at {}.", brick.position);
    }
}
```

### Writing

Below will create a 10x10 grid of bricks and save to `brickadia-rs.brs`.

```rs
use std::fs;

use brickadia::{
    save::{Brick, BrickColor, BrickOwner, Color, SaveData, Size, User},
    write::SaveWriter,
};

fn main() {
    let me = User {
        name: "x".into(),
        id: "3f5108a0-c929-4e77-a115-21f65096887b".parse().unwrap(),
    };

    let mut save = SaveData::default();

    // set the first header
    save.header1.author = me.clone();
    save.header1.host = Some(me.clone());
    save.header1.description = "This was saved with brickadia-rs!".into();

    // set the second header
    save.header2
        .brick_owners
        .push(BrickOwner::from_user_bricks(me.clone(), 100));

    // set the preview image
    let preview_bytes = std::fs::read("examples/write_preview.png").unwrap();
    save.preview = Some(preview_bytes);

    // add some bricks
    for y in 0..10 {
        for x in 0..10 {
            let mut brick = Brick::default();
            brick.position = (x * 10, y * 10, 10);
            brick.size = Size::Procedural(5, 5, 6);
            brick.color = BrickColor::Unique(Color {
                r: (x as f32 / 10.0 * 255.0) as u8,
                g: (y as f32 / 10.0 * 255.0) as u8,
                b: 255,
                a: 255,
            });
            save.bricks.push(brick);
        }
    }

    // write out the save
    let mut writer = SaveWriter::new(File::create("brickadia-rs.brs").unwrap(), save);
    writer.write().unwrap();

    println!("Save written!");
}
```
