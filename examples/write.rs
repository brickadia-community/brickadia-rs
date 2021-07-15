use std::{env, fs::File};

use brickadia::{save::{Brick, BrickColor, BrickOwner, Color, Direction, Rotation, SaveData, Size, User}, write::SaveWriter};
use uuid::Uuid;

fn main() {
    let me = User { name: "x".into(), id: "3f5108a0-c929-4e77-a115-21f65096887b".parse().unwrap() };

    let mut save = SaveData::default();

    // set the first header
    save.header1.author = me.clone();
    save.header1.host = Some(me.clone());
    save.header1.description = "This was saved with brickadia-rs!".into();
    
    // set the second header
    save.header2.brick_owners.push(BrickOwner::from_user_bricks(me.clone(), 100));

    // set the preview image
    let preview_bytes = std::fs::read("examples/write_preview.png").unwrap();
    save.preview = Some(preview_bytes);

    // add some bricks
    for y in 0..10 {
        for x in 0..10 {
            let mut brick = Brick::default();
            brick.position = (x * 10, y * 10, 10);
            brick.size = Size::Procedural(5, 5, 6);
            brick.color = BrickColor::Unique(Color { r: (x / 10 * 255) as u8, g: (y / 10 * 255) as u8, b: 255, a: 255 });
            save.bricks.push(brick);
        }
    }

    // write out the save
    let save_location = env::args().nth(1).unwrap_or("examples/write.out.brs".into());
    let mut writer = SaveWriter::new(File::create(save_location).unwrap(), save);
    writer.write().unwrap();

    println!("Save written");
}