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

#### Util module

The optional feature `util` includes some utilities like getting brick size from brick asset, handling with
rotations and orientations, and so on. It is enabled by default. To disable it, use `default_features = false`
in your `Cargo.toml` dependency for `brickadia`, e.g. `brickadia = { version = "0.1", default_features = false }`.

#### Octree module

Included in the `util` module is a module named `octree`, which adds an octree constructor and traversal object
you can wrap around your `SaveData` that will allow you to quickly traverse through bricks in space. Here is
some example usage:

```rs
// ... assume we have a `SaveData` named `save`
let octree = SaveOctree::new(save); // or `save.into_octree();`

// find the first brick that has a color of (0, 0) in the palette
let base_brick = octree.data().bricks.iter().find(|b| b.color == BrickColor::Index(0)).unwrap();

// fetch a list of bricks above it
let bricks_above = octree.brick_side(base_brick, Direction::ZPositive);
```

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
use std::{env, fs::File};

use brickadia::{
    save::{Brick, BrickColor, BrickOwner, Color, Preview, SaveData, Size, User},
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
    save.preview = Preview::PNG(preview_bytes);

    // add some bricks
    for y in 0..10 {
        for x in 0..10 {
            let mut brick = Brick::default();
            brick.position = (x * 10, y * 10, 10);
            brick.size = Size::Procedural(5, 5, 6);
            brick.color = BrickColor::Unique(Color {
                r: (x as f32 / 10.0 * 255.0) as u8,
                g: 255,
                b: (y as f32 / 10.0 * 255.0) as u8,
                a: 255,
            });
            save.bricks.push(brick);
        }
    }

    // write out the save
    let save_location = env::args()
        .nth(1)
        .unwrap_or("examples/write.out.brs".into());
    SaveWriter::new(File::create(save_location).unwrap(), save)
        .write()
        .unwrap();

    println!("Save written");
}
```

## Credits

* [voximity](https://github.com/voximity) - creator, maintainer
* [Meshiest](https://github.com/Meshiest) - [brs-js](https://github.com/brickadia-community/brs-js) reference, octree implementation from JS
* [qoh](https://github.com/qoh) - [brs](https://github.com/brickadia/brs), original library
