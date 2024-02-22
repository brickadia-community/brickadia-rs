//! Octree implementation for performing spatial lookups on bricks.
//!
//! Generally, you will want to use a [`SaveOctree`](SaveOctree),
//! not other exposed items from this module.

use std::cmp;
use std::collections::HashSet;
use std::fmt::Display;
use std::hash::Hash;

use crate::save::{Brick, Direction, SaveData};

use super::get_axis_size;

/// The size, in units, of an octree chunk.
pub const CHUNK_SIZE: i32 = 1024;
pub const RIGHT: i32 = 1;
pub const BACK: i32 = 2;
pub const BOTTOM: i32 = 4;

/// An integer point in space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl Point {
    /// Initialize a new Point.
    pub fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    /// Get a chunk from a point.
    pub fn chunk(&self) -> Self {
        fn div_floor(a: i32) -> i32 {
            if a >= 0 {
                a / CHUNK_SIZE
            } else {
                (a - CHUNK_SIZE - 1) / CHUNK_SIZE
            }
        }

        Self::new(div_floor(self.x), div_floor(self.y), div_floor(self.z))
    }

    /// Check if a point is contained by the bounds formed by `min` and `max`.
    #[rustfmt::skip]
    pub fn is_in(self, min: Self, max: Self) -> bool {
        min.x <= self.x && self.x <= max.x
            && min.y <= self.y && self.y <= max.y
            && min.z <= self.z && self.z <= max.z
    }

    /// Get the octant the point should be in for a node.
    pub fn octant(self, point: Self) -> i32 {
        (if point.x >= self.x { RIGHT } else { 0 })
            | (if point.y >= self.y { BACK } else { 0 })
            | (if point.z >= self.z { BOTTOM } else { 0 })
    }

    /// Get the midpoint of a chunk.
    pub fn chunk_midpoint(self) -> Self {
        Self::new(
            self.x * CHUNK_SIZE + CHUNK_SIZE / 2,
            self.y * CHUNK_SIZE + CHUNK_SIZE / 2,
            self.z * CHUNK_SIZE + CHUNK_SIZE / 2,
        )
    }

    /// Shift a point by some offset.
    pub fn shifted(self, x: i32, y: i32, z: i32) -> Self {
        Self::new(self.x + x, self.y + y, self.z + z)
    }
}

impl Display for Point {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, {}, {})", self.x, self.y, self.z)
    }
}

impl From<(i32, i32, i32)> for Point {
    fn from(p: (i32, i32, i32)) -> Self {
        Self {
            x: p.0,
            y: p.1,
            z: p.2,
        }
    }
}

/// A node in the octree.
#[derive(Debug, PartialEq)]
pub struct Node<T: PartialEq + Eq + Hash + Copy> {
    pub point: Point,
    pub depth: u32,
    pub value: NodeValue<T>,
    pub half: u32,
}

/// A node value in the octree.
#[derive(Debug, PartialEq)]
pub enum NodeValue<T: PartialEq + Eq + Hash + Copy> {
    Value(Option<T>),
    Children(Vec<Node<T>>),
}

#[allow(dead_code)]
impl<T: PartialEq + Eq + Hash + Copy> Node<T> {
    /// Instantiate a new `Node<T>`.
    pub fn new(point: Point, depth: u32, value: Option<T>) -> Self {
        Self {
            point,
            depth,
            value: NodeValue::Value(value),
            half: 2u32.pow(cmp::max(depth, 1) - 1),
        }
    }

    pub fn would_fill_node(&self, min: Point, max: Point) -> bool {
        let size = 1 << self.depth;
        (max.x - min.x) == size && (max.y - min.y) == size && (max.z - min.z) == size
    }

    pub fn is_inside(&self, min: Point, max: Point) -> bool {
        let half = self.half as i32;
        self.point.x - half >= min.x
            && self.point.x + half <= max.x
            && self.point.y - half >= min.y
            && self.point.y + half <= max.y
            && self.point.z - half >= min.z
            && self.point.z + half <= max.z
    }

    pub fn is_outside(&self, min: Point, max: Point) -> bool {
        let half = self.half as i32;
        self.point.x + half <= min.x
            || self.point.x - half >= max.x
            || self.point.y + half <= min.y
            || self.point.y - half >= max.y
            || self.point.z + half <= min.z
            || self.point.z - half >= max.z
    }

    pub fn reduce(&mut self) {
        let nodes = match &mut self.value {
            NodeValue::Children(nodes) => nodes,
            _ => return,
        };

        for i in 0..8 {
            // reduce the node if possible
            nodes[i].reduce();

            // if the node still has children, we can't proceed
            // we also can't proceed if the value isn't equivalent
            if let NodeValue::Children(_) = nodes[i].value {
                return;
            } else if i != 0 && nodes[i].value != nodes[0].value {
                return;
            }
        }

        self.value = NodeValue::Value(match nodes.pop().unwrap().value {
            NodeValue::Value(v) => v,
            _ => unreachable!(),
        });
    }

    pub fn insert(&mut self, value: T, min: Point, max: Point) {
        if self.is_inside(min, max) {
            self.value = NodeValue::Value(Some(value));
            return;
        }

        if self.depth == 0 {
            return;
        }

        if let NodeValue::Value(old_value) = self.value {
            let child_depth = self.depth - 1;
            let child_shift = (self.half / 2) as i32;
            self.value = NodeValue::Children(vec![
                Node::new(
                    self.point.shifted(-child_shift, -child_shift, -child_shift),
                    child_depth,
                    old_value,
                ),
                Node::new(
                    self.point.shifted(child_shift, -child_shift, -child_shift),
                    child_depth,
                    old_value,
                ),
                Node::new(
                    self.point.shifted(-child_shift, child_shift, -child_shift),
                    child_depth,
                    old_value,
                ),
                Node::new(
                    self.point.shifted(child_shift, child_shift, -child_shift),
                    child_depth,
                    old_value,
                ),
                Node::new(
                    self.point.shifted(-child_shift, -child_shift, child_shift),
                    child_depth,
                    old_value,
                ),
                Node::new(
                    self.point.shifted(child_shift, -child_shift, child_shift),
                    child_depth,
                    old_value,
                ),
                Node::new(
                    self.point.shifted(-child_shift, child_shift, child_shift),
                    child_depth,
                    old_value,
                ),
                Node::new(
                    self.point.shifted(child_shift, child_shift, child_shift),
                    child_depth,
                    old_value,
                ),
            ]);
        }

        if let NodeValue::Children(nodes) = &mut self.value {
            for node in nodes.iter_mut() {
                if !node.is_outside(min, max) {
                    // !node.is_outside(min, max) {
                    node.insert(value, min, max);
                }
            }
        }
    }

    pub fn search(&self, min: Point, max: Point, set: &mut HashSet<T>) {
        if let NodeValue::Value(value) = self.value {
            if let Some(value) = value {
                set.insert(value);
            }
            return;
        }

        let nodes = match self.value {
            NodeValue::Children(ref nodes) => nodes,
            _ => unreachable!(),
        };

        for node in nodes.iter() {
            if !node.is_outside(min, max) {
                // !node.is_outside(min, max) {
                node.search(min, max, set);
            }
        }
    }

    pub fn get(&self, point: Point) -> Option<&T> {
        match &self.value {
            NodeValue::Value(value) => value.as_ref(),
            NodeValue::Children(nodes) => nodes[self.point.octant(point) as usize].get(point),
        }
    }
}

/// A series of chunks.
#[derive(Default)]
pub struct ChunkTree<T: PartialEq + Eq + Hash + Copy> {
    pub chunks: Vec<(Node<T>, Point)>,
}

impl<T: PartialEq + Eq + Hash + Copy> ChunkTree<T> {
    /// Instantiate an empty chunk tree.
    pub fn new() -> Self {
        ChunkTree { chunks: vec![] }
    }

    /// Reduce all child trees.
    pub fn reduce(&mut self) {
        for (node, _) in self.chunks.iter_mut() {
            node.reduce();
        }
    }

    /// Get a list of chunks contained by a `min` and a `max`.
    pub fn chunks_from_bounds(&self, min: Point, max: Point) -> Vec<(Point, Point)> {
        let min_chunk = min.chunk();
        let max_chunk = max.shifted(-1, -1, -1).chunk();
        if min_chunk.x > max_chunk.x || min_chunk.y > max_chunk.y || min_chunk.z > max_chunk.z {
            panic!("max chunk too small");
        }

        if min_chunk != max_chunk {
            let mut v = vec![];
            for x in min_chunk.x..=max_chunk.x {
                let min_x = if x == min_chunk.x {
                    min.x
                } else {
                    x * CHUNK_SIZE
                };
                let max_x = cmp::min(
                    (min_x as f64 / CHUNK_SIZE as f64 + 1.0).floor() as i32 * CHUNK_SIZE,
                    max.x,
                );

                for y in min_chunk.y..=max_chunk.y {
                    let min_y = if y == min_chunk.y {
                        min.y
                    } else {
                        y * CHUNK_SIZE
                    };
                    let max_y = cmp::min(
                        (min_y as f64 / CHUNK_SIZE as f64 + 1.0).floor() as i32 * CHUNK_SIZE,
                        max.y,
                    );

                    for z in min_chunk.z..=max_chunk.z {
                        let min_z = if z == min_chunk.z {
                            min.z
                        } else {
                            z * CHUNK_SIZE
                        };
                        let max_z = cmp::min(
                            (min_z as f64 / CHUNK_SIZE as f64 + 1.0).floor() as i32 * CHUNK_SIZE,
                            max.z,
                        );

                        v.push((
                            Point::new(min_x, min_y, min_z),
                            Point::new(max_x, max_y, max_z),
                        ));
                    }
                }
            }
            v
        } else {
            vec![(min, max)]
        }
    }

    /// Get a reference to chunk at `point`, if it exists.
    pub fn chunk_at(&self, point: Point) -> Option<&Node<T>> {
        let chunk_pos = point.chunk();
        self.chunks
            .iter()
            .find(|(_, p)| *p == chunk_pos)
            .map(|(node, _)| node)
    }

    /// Get a mutable reference to the chunk at `point`, if it exists.
    pub fn chunk_at_mut(&mut self, point: Point) -> Option<&mut Node<T>> {
        let chunk_pos = point.chunk();
        self.chunks
            .iter_mut()
            .find(|(_, p)| *p == chunk_pos)
            .map(|(node, _)| node)
    }

    /// Search for `T`s contained within `min_bound` and `max_bound`.
    pub fn search(&self, min_bound: Point, max_bound: Point) -> HashSet<T> {
        let mut set = HashSet::new();
        for (min, max) in self.chunks_from_bounds(min_bound, max_bound).into_iter() {
            if let Some(chunk) = self.chunk_at(min) {
                chunk.search(min, max, &mut set);
            }
        }
        set
    }

    /// Get the `T` at exactly `point`.
    pub fn get(&self, point: Point) -> Option<&T> {
        self.chunk_at(point).and_then(|node| node.get(point))
    }

    /// Insert a `T` into the chunks, from `min_bound` to `max_bound`.
    pub fn insert(&mut self, value: T, min_bound: Point, max_bound: Point) {
        for (min, max) in self.chunks_from_bounds(min_bound, max_bound).into_iter() {
            match self.chunk_at_mut(min) {
                Some(node) => node.insert(value, min, max),
                None => {
                    let chunk_pos = min.chunk();
                    let mut chunk = Node::new(chunk_pos.chunk_midpoint(), 10, None);
                    chunk.insert(value, min, max);
                    self.chunks.push((chunk, chunk_pos));
                }
            }
        }
    }
}

/// A wrapper around some save data to fetch bricks quickly.
pub struct SaveOctree {
    /// The save data.
    data: SaveData,
    /// The chunks in the octree.
    tree: ChunkTree<usize>,
}

impl SaveOctree {
    /// Construct a `SaveOctree` over a `SaveData`, consuming it.
    pub fn new(data: SaveData) -> Self {
        let mut tree = SaveOctree {
            data,
            tree: ChunkTree::new(),
        };
        for (i, brick) in tree.data.bricks.iter().enumerate() {
            let (min, max) = tree.brick_bounds(brick);
            if min != max {
                tree.tree.insert(i, min.into(), max.into());
            }
        }
        tree
    }

    /// Take a reference to the inner `SaveData`.
    ///
    /// This cannot be mutable as the octree would have to rebuild.
    /// If you need to alter the SaveData and traverse again, instead use
    /// `into_inner()` to take out the `SaveData`, make your changes,
    /// and reconstruct with `new(SaveData)`.
    pub fn data(&self) -> &SaveData {
        &self.data
    }

    /// Get the size of a brick. This is its absolute size, regardless of rotation.
    pub fn brick_size(&self, brick: &Brick) -> (u32, u32, u32) {
        (
            get_axis_size(brick, &self.data.header2.brick_assets, 0),
            get_axis_size(brick, &self.data.header2.brick_assets, 1),
            get_axis_size(brick, &self.data.header2.brick_assets, 2),
        )
    }

    /// Gets the bounds of a brick as two points in space.
    pub fn brick_bounds(&self, brick: &Brick) -> ((i32, i32, i32), (i32, i32, i32)) {
        let s = self.brick_size(brick);
        (
            (
                brick.position.0 - s.0 as i32,
                brick.position.1 - s.1 as i32,
                brick.position.2 - s.2 as i32,
            ),
            (
                brick.position.0 + s.0 as i32,
                brick.position.1 + s.1 as i32,
                brick.position.2 + s.2 as i32,
            ),
        )
    }

    /// Fetch all bricks within some volume in space. This includes bricks that are partially
    /// in this volume.
    pub fn bricks_in(&self, min: (i32, i32, i32), max: (i32, i32, i32)) -> Vec<&Brick> {
        self.tree
            .search(min.into(), max.into())
            .into_iter()
            .map(|idx| &self.data.bricks[idx])
            .collect()
    }

    /// Fetch all bricks that bound a volume on one of its sides. This includes bricks that are partially
    /// in this volume.
    pub fn bounds_side(
        &self,
        min: (i32, i32, i32),
        max: (i32, i32, i32),
        dir: Direction,
    ) -> Vec<&Brick> {
        let indices = match dir {
            Direction::XPositive => self.tree.search(
                Point::new(max.0, min.1, min.2),
                Point::new(max.0 + 1, max.1, max.2),
            ),
            Direction::XNegative => self.tree.search(
                Point::new(min.0 - 1, min.1, min.2),
                Point::new(min.0, max.1, max.2),
            ),
            Direction::YPositive => self.tree.search(
                Point::new(min.0, max.1, min.2),
                Point::new(max.0, max.1 + 1, max.2),
            ),
            Direction::YNegative => self.tree.search(
                Point::new(min.0, min.1 - 1, min.2),
                Point::new(max.0, min.1, max.2),
            ),
            Direction::ZPositive => self.tree.search(
                Point::new(min.0, min.1, max.2),
                Point::new(max.0, max.1, max.2 + 1),
            ),
            Direction::ZNegative => self.tree.search(
                Point::new(min.0, min.1, min.2 - 1),
                Point::new(max.0, max.1, min.2),
            ),
        };

        indices
            .into_iter()
            .map(|idx| &self.data.bricks[idx])
            .collect()
    }

    /// Fetch all bricks that bound a brick on one of its sides. This includes bricks that are partially
    /// in the bounding volume.
    pub fn brick_side(&self, brick: &Brick, dir: Direction) -> Vec<&Brick> {
        let (min, max) = self.brick_bounds(brick);
        self.bounds_side(min, max, dir)
    }

    /// Return the inner `SaveData`, consuming this `SaveOctree`.
    pub fn into_inner(self) -> SaveData {
        self.data
    }
}
