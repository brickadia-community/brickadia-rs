use std::{collections::HashMap};

use num_enum::{IntoPrimitive, TryFromPrimitive};
use uuid::Uuid;

#[cfg(feature = "serialize")]
use {
    std::fmt,
    serde::{
        de::{self, Visitor},
        ser::SerializeTuple,
        Deserialize, Deserializer, Serialize, Serializer,
    },
    serde_repr::{Deserialize_repr, Serialize_repr},
};

use crate::SAVE_VERSION;

/// Every part of a save file.
#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct SaveData {
    /// The version of the save. Only relevant for reads; this automatically uses `SAVE_VERSION` when writing.
    pub version: u16,

    /// The game version the save was saved on.
    pub game_version: i32,

    /// The first header of the save.
    #[cfg_attr(feature = "serialize", serde(flatten))]
    pub header1: Header1,

    /// The second header of the save.
    #[cfg_attr(feature = "serialize", serde(flatten))]
    pub header2: Header2,

    /// The preview of the save, if any.
    #[cfg_attr(feature = "serialize", serde(skip))]
    pub preview: Option<Vec<u8>>,

    /// The bricks in the save.
    pub bricks: Vec<Brick>,

    /// The components in the save.
    pub components: HashMap<String, Component>,
}

impl Default for SaveData {
    fn default() -> Self {
        SaveData {
            version: SAVE_VERSION,
            game_version: 0,
            header1: Header1::default(),
            header2: Header2::default(),
            preview: None,
            bricks: vec![],
            components: HashMap::new(),
        }
    }
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct Header1 {
    /// The map the save was saved on.
    pub map: String,

    /// The description given to the save.
    pub description: String,

    /// The user who saved this save file.
    pub author: User,

    /// The host of the server in which the save was saved. Only available in save versions 8+.
    pub host: Option<User>,

    /// The save time of the save.
    pub save_time: [u8; 8],

    /// The number of bricks in the save.
    pub brick_count: u32,
}

impl Default for Header1 {
    fn default() -> Self {
        Header1 {
            map: "Unknown".into(),
            description: String::new(),
            author: User::default(),
            host: None,
            save_time: [0u8; 8],
            brick_count: 0,
        }
    }
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct Header2 {
    /// A list of mods, each a String.
    pub mods: Vec<String>,

    /// A list of brick assets, each a String.
    pub brick_assets: Vec<String>,

    /// A list of colors in the save. Brick color indexes refer to this list.
    pub colors: Vec<Color>,

    /// A list of materials used in the save. Brick material indexes refer to this list.
    pub materials: Vec<String>,

    /// A list of brick owners.
    pub brick_owners: Vec<BrickOwner>,

    /// A list of physical materials. Empty if save version is
    pub physical_materials: Vec<String>,
}

impl Default for Header2 {
    fn default() -> Self {
        Header2 {
            mods: vec![],
            brick_assets: vec!["PB_DefaultBrick".into()],
            colors: vec![],
            materials: vec!["BMC_Plastic".into()],
            brick_owners: vec![],
            physical_materials: vec!["BPMC_Default".into()],
        }
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize), serde(untagged))]
pub enum UnrealType {
    Class(String),
    Boolean(bool),
    Float(f32),
    Color(Color),
    Byte(u8),
    Rotator(f32, f32, f32),
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct User {
    /// The user's name.
    pub name: String,

    /// The user's ID, a UUID.
    pub id: Uuid,
}

impl Default for User {
    fn default() -> Self {
        User {
            name: "Unknown".into(),
            id: Uuid::default(),
        }
    }
}

/// A brick owner. Similar to a user, but stores an u32 representing bricks in save.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct BrickOwner {
    /// The brick owner's name.
    pub name: String,
    pub id: Uuid,
    pub bricks: u32,
}

impl From<User> for BrickOwner {
    fn from(user: User) -> Self {
        BrickOwner {
            name: user.name,
            id: user.id,
            bricks: 0,
        }
    }
}

impl BrickOwner {
    pub fn from_user_bricks(user: User, bricks: u32) -> Self {
        BrickOwner {
            name: user.name,
            id: user.id,
            bricks,
        }
    }
}

/// A color, in RGBA.
#[derive(Debug, Clone)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[cfg(feature = "serialize")]
impl Serialize for Color {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut tup = serializer.serialize_tuple(4)?;
        tup.serialize_element(&self.r)?;
        tup.serialize_element(&self.g)?;
        tup.serialize_element(&self.b)?;
        tup.serialize_element(&self.a)?;
        tup.end()
    }
}

#[cfg(feature = "serialize")]
struct ColorVisitor;

#[cfg(feature = "serialize")]
impl<'de> Visitor<'de> for ColorVisitor {
    type Value = Color;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "a color (an array of either 3 or 4 bytes)")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        let r = seq
            .next_element()?
            .ok_or(de::Error::invalid_length(0, &"3 or 4"))?;
        let g = seq
            .next_element()?
            .ok_or(de::Error::invalid_length(1, &"3 or 4"))?;
        let b = seq
            .next_element()?
            .ok_or(de::Error::invalid_length(2, &"3 or 4"))?;
        let a = seq.next_element()?.unwrap_or(255);

        Ok(Color { r, g, b, a })
    }
}

#[cfg(feature = "serialize")]
impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(ColorVisitor)
    }
}

impl Color {
    /// Converts a slice of 4 bytes (bgra) to a Color (rgba).
    pub fn from_bytes_bgra(slice: [u8; 4]) -> Self {
        Color {
            r: slice[2],
            g: slice[1],
            b: slice[0],
            a: slice[3],
        }
    }

    /// Converts a slice of 3 bytes (rgb) to a Color (rgba), assuming a = 255.
    pub fn from_bytes_rgb(slice: [u8; 3]) -> Self {
        Color {
            r: slice[0],
            g: slice[1],
            b: slice[2],
            a: 255,
        }
    }
}

/// A brick.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct Brick {
    /// The asset name index of the brick, referring to `Header2`'s `brick_assets`.
    pub asset_name_index: u32,

    /// The size of the brick. Bricks that are not procedural should have this set to `Size::Empty`.
    pub size: Size,

    /// The position of the brick.
    pub position: (i32, i32, i32),

    /// The direction of the brick.
    pub direction: Direction,

    /// The rotation of the brick.
    pub rotation: Rotation,

    /// The collision flags of the brick.
    pub collision: Collision,

    /// Whether or not the brick is visible.
    pub visibility: bool,

    /// The material index of the brick.
    pub material_index: u32,

    /// The physical index of the brick.
    pub physical_index: u32,

    /// The material intensity of the brick.
    pub material_intensity: u32,

    /// The color of the brick. When referring to an index from the colors array in `Header2`, use `BrickColor::Index`. Otherwise, use `BrickColor::Unique(Color)`.
    pub color: BrickColor,

    /// The owner index of the brick. When 0, this brick's owner is PUBLIC. Otherwise, it refers to `Header2`'s `brick_owners`, 1-indexed.
    pub owner_index: u32,

    /// The components on this brick.
    pub components: HashMap<String, HashMap<String, UnrealType>>,
}

impl Default for Brick {
    fn default() -> Self {
        Brick {
            asset_name_index: 0,
            size: Size::Empty,
            position: (0, 0, 0),
            direction: Direction::ZPositive,
            rotation: Rotation::Deg0,
            collision: Collision::for_all(true),
            visibility: true,
            material_index: 0,
            physical_index: 0,
            material_intensity: 5,
            color: BrickColor::Index(0),
            owner_index: 0,
            components: HashMap::new(),
        }
    }
}

/// Represents a brick's direction.
#[repr(u8)]
#[derive(Debug, Clone, IntoPrimitive, TryFromPrimitive)]
#[cfg_attr(feature = "serialize", derive(Serialize_repr, Deserialize_repr))]
pub enum Direction {
    XPositive,
    XNegative,
    YPositive,
    YNegative,
    ZPositive,
    ZNegative,
}

/// Represents a brick's rotation.
#[repr(u8)]
#[derive(Debug, Clone, IntoPrimitive, TryFromPrimitive)]
#[cfg_attr(feature = "serialize", derive(Serialize_repr, Deserialize_repr))]
pub enum Rotation {
    Deg0,
    Deg90,
    Deg180,
    Deg270,
}

/// Represents a storable brick size.
///
/// Procedural bricks should use `Size::Procedural`.
/// Static mesh bricks should use `Size::Empty`.
#[derive(Debug, Clone)]
pub enum Size {
    /// A singularity (used for non-procedural bricks).
    Empty,

    /// A brick that is procedural.
    Procedural(u32, u32, u32),
}

#[cfg(feature = "serialize")]
impl Serialize for Size {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let (x, y, z) = match self {
            Size::Empty => (&0, &0, &0),
            Size::Procedural(x, y, z) => (x, y, z),
        };

        let mut tup = serializer.serialize_tuple(3)?;
        tup.serialize_element(x)?;
        tup.serialize_element(y)?;
        tup.serialize_element(z)?;
        tup.end()
    }
}

#[cfg(feature = "serialize")]
struct SizeVisitor;

#[cfg(feature = "serialize")]
impl<'de> Visitor<'de> for SizeVisitor {
    type Value = Size;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "an array of 3 numbers")
    }

    fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let x = seq
            .next_element()?
            .ok_or(de::Error::invalid_length(0, &"3 numbers"))?;
        let y = seq
            .next_element()?
            .ok_or(de::Error::invalid_length(1, &"3 numbers"))?;
        let z = seq
            .next_element()?
            .ok_or(de::Error::invalid_length(2, &"3 numbers"))?;

        if x == 0 && y == 0 && z == 0 {
            Ok(Size::Empty)
        } else {
            Ok(Size::Procedural(x, y, z))
        }
    }
}

#[cfg(feature = "serialize")]
impl<'de> Deserialize<'de> for Size {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(SizeVisitor)
    }
}

/// Represents a brick's color.
///
/// Bricks that refer to a color in their save should use `BrickColor::Index`.
/// Bricks defining their own `Color` should use `BrickColor::Unique`.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize), serde(untagged))]
pub enum BrickColor {
    /// A color that links to an index in the save palette.
    Index(u32),

    /// A unique color for this brick.
    Unique(Color),
}

/// Represents a brick's collision flags.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct Collision {
    pub player: bool,
    pub weapon: bool,
    pub interaction: bool,
    pub tool: bool,
}

impl Collision {
    pub fn for_all(state: bool) -> Self {
        Collision {
            player: state,
            weapon: state,
            interaction: state,
            tool: state,
        }
    }
}

impl Default for Collision {
    fn default() -> Self {
        Self::for_all(true)
    }
}

/// A component.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct Component {
    pub version: i32,

    /// The indices of bricks this component is on.
    pub brick_indices: Vec<u32>,

    /// A map from property name to Unreal type (see `UnrealType`).
    pub properties: HashMap<String, String>,
}

impl Default for Component {
    fn default() -> Self {
        Component {
            version: 1,
            brick_indices: vec![],
            properties: HashMap::new(),
        }
    }
}
