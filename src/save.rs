//! General save file types and helpers.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::io::Read;

use byteorder::{LittleEndian, ReadBytesExt};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use uuid::Uuid;

#[cfg(feature = "serialize")]
use {
    serde::{
        de::{self, Visitor},
        ser::SerializeTuple,
        Deserialize, Deserializer, Serialize, Serializer,
    },
    serde_repr::{Deserialize_repr, Serialize_repr},
    std::fmt,
};

use crate::read::ReadError;
use crate::SAVE_VERSION;

/// An entire save file.
///
/// Represents data that can be written out with a [`SaveWriter`], or read with a [`SaveReader`].
/// Composed of its [`Header1`](Header1), [`Header2`](Header2), and more information.
///
/// [`SaveWriter`]: crate::write::SaveWriter
/// [`SaveReader`]: crate::read::SaveReader
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize), serde(default))]
pub struct SaveData {
    /// The version of the save. Only relevant for reads; this automatically uses [`SAVE_VERSION`](crate::SAVE_VERSION) when writing.
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
    pub preview: Preview,

    /// The bricks in the save.
    pub bricks: Vec<Brick>,

    /// The components in the save.
    pub components: HashMap<String, Component>,
}

impl SaveData {
    /// Convert this `SaveData` into a `SaveOctree` for quick traversal of bricks in space.
    #[cfg(feature = "util")]
    pub fn into_octree(self) -> crate::util::octree::SaveOctree {
        crate::util::octree::SaveOctree::new(self)
    }
}

impl Default for SaveData {
    fn default() -> Self {
        SaveData {
            version: SAVE_VERSION,
            game_version: 0,
            header1: Header1::default(),
            header2: Header2::default(),
            preview: Preview::None,
            bricks: vec![],
            components: HashMap::new(),
        }
    }
}

/// The first header in a save file. Contains basic save information.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize), serde(default))]
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
    #[cfg_attr(feature = "serialize", serde(skip))]
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

/// The second header in a save file. Contains universal brick metadata.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize), serde(default))]
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

    /// A list of physical materials. Possibly empty, if the game version is too old.
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

/// An image preview embedded in a save, represented by its bytes.
#[derive(Debug, Clone)]
pub enum Preview {
    /// No preview.
    None,
    /// A PNG preview.
    PNG(Vec<u8>),
    /// A JPEG preview.
    JPEG(Vec<u8>),
    /// An unknown preview type.
    Unknown(
        /// The type byte of this unknown preview type.
        u8,
        Vec<u8>,
    ),
}

impl Preview {
    /// Create a `Preview` from a reader.
    pub fn from_reader(r: &mut impl Read) -> Result<Self, ReadError> {
        fn read_bytes(r: &mut impl Read) -> Result<Vec<u8>, ReadError> {
            let len = r.read_i32::<LittleEndian>()?;
            let mut vec = vec![0u8; len as usize];
            r.read_exact(&mut vec)?;
            Ok(vec)
        }

        let mode = r.read_u8()?;
        Ok(match mode {
            0 => Self::None,
            1 => Self::PNG(read_bytes(r)?),
            2 => Self::JPEG(read_bytes(r)?),
            other => Self::Unknown(other, read_bytes(r)?),
        })
    }

    pub fn type_byte(&self) -> u8 {
        match self {
            Preview::None => 0,
            Preview::PNG(_) => 1,
            Preview::JPEG(_) => 2,
            Preview::Unknown(byte, _) => *byte,
        }
    }

    /// Consume the `Preview`, extracting its bytes, or `None` if no preview was set.
    pub fn into_bytes(self) -> Option<Vec<u8>> {
        match self {
            Preview::None => None,
            Preview::PNG(bytes) => Some(bytes),
            Preview::JPEG(bytes) => Some(bytes),
            Preview::Unknown(_, bytes) => Some(bytes),
        }
    }

    /// Whether or not the `Preview` was unset.
    pub fn is_none(&self) -> bool {
        matches!(self, Preview::None)
    }

    /// Whether or not the `Preview` was set.
    pub fn is_some(&self) -> bool {
        !self.is_none()
    }

    /// Equivalent to `.into_bytes().unwrap()`. Panics if no bytes are set (the preview was unset).
    pub fn unwrap(self) -> Vec<u8> {
        self.into_bytes().unwrap()
    }
}

/// An Unreal type, used as values to fields in components.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize), serde(untagged))]
pub enum UnrealType {
    Class(String),
    String(String),
    Boolean(bool),
    Float(f32),
    Color(Color),
    Byte(u8),
    Rotator(f32, f32, f32),
}

/// A user.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize), serde(default))]
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

/// A brick owner. Similar to a [`User`](User), but stores a `u32` representing bricks in save.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct BrickOwner {
    /// The brick owner's name.
    pub name: String,
    /// The owner's ID.
    pub id: Uuid,
    /// The amount of bricks placed by the owner.
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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize), serde(default))]
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
    #[cfg_attr(feature = "serialize", serde(serialize_with = "brick_color_serialize"))]
    pub color: BrickColor,

    /// The owner index of the brick. When 0, this brick's owner is PUBLIC. Otherwise, it refers to `Header2`'s `brick_owners`, 1-indexed.
    pub owner_index: u32,

    /// The components on this brick.
    pub components: HashMap<String, HashMap<String, UnrealType>>,
}

#[cfg(feature = "serialize")]
fn brick_color_serialize<S: Serializer>(color: &BrickColor, s: S) -> Result<S::Ok, S::Error> {
    match color {
        BrickColor::Index(index) => s.serialize_u32(*index),
        BrickColor::Unique(color) => {
            let mut tup = s.serialize_tuple(3)?;
            tup.serialize_element(&color.r)?;
            tup.serialize_element(&color.g)?;
            tup.serialize_element(&color.b)?;
            tup.end()
        }
    }
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

// Manual Hash impl necessary as `HashMap` is not `Hash`.
impl Hash for Brick {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.asset_name_index.hash(state);
        self.size.hash(state);
        self.position.hash(state);
        self.direction.hash(state);
        self.rotation.hash(state);
        self.collision.hash(state);
        self.visibility.hash(state);
        self.material_index.hash(state);
        self.physical_index.hash(state);
        self.material_intensity.hash(state);
        self.color.hash(state);
        self.owner_index.hash(state);
    }
}

/// Represents a brick's direction.
#[repr(u8)]
#[derive(
    Debug, Copy, Clone, IntoPrimitive, TryFromPrimitive, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
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
#[derive(
    Debug, Copy, Clone, IntoPrimitive, TryFromPrimitive, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
#[cfg_attr(feature = "serialize", derive(Serialize_repr, Deserialize_repr))]
pub enum Rotation {
    Deg0,
    Deg90,
    Deg180,
    Deg270,
}

/// Represents a storable brick size.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Size {
    /// A singularity (used for non-procedural, static-mesh bricks).
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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize), serde(untagged))]
pub enum BrickColor {
    /// A color that links to an index in the save palette.
    Index(u32),

    /// A unique color for this brick.
    Unique(Color),
}

/// Represents a brick's collision flags.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize), serde(default))]
pub struct Collision {
    /// Whether or not players collide with the brick.
    pub player: bool,
    /// Whether or not bullets, slashes, and other projectiles collide with the brick.
    pub weapon: bool,
    /// Whether or not player interactions collide with the brick.
    pub interaction: bool,
    /// Whether or not the brick can be considered in tool clicks.
    ///
    /// When false, this brick cannot be removed with the hammer, painted, etc.
    pub tool: bool,
}

impl Collision {
    /// Create a `Collision` with all flags set to `state`.
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

/// A brick component.
///
/// ### Known component names
///
/// Below are a list of known component names as keys to `properties`.
///
/// * `BCD_SpotLight`
/// * `BCD_PointLight`
/// * `BCD_ItemSpawn`
/// * `BCD_Interact`
/// * `BCD_AudioEmitter`
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct Component {
    /// The version of this component.
    pub version: i32,

    /// The indices of bricks this component is on.
    pub brick_indices: Vec<u32>,

    /// A map from property name to Unreal type (see `UnrealType`).
    ///
    /// See above for a list of known component names to use as keys to this map.
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
