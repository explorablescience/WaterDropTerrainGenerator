//! A pool of tiles that can be allocated and deallocated.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub type TileHandle = Arc<TileBuffer>;

/// Allocated from a [`TilePool`]; returned to its backing [`TileArena`] when dropped. Deref to a
/// `[f32]` slice for the data - multi-channel tiles are stored as consecutive `size*size` planes.
#[derive(Debug, Clone)]
pub struct TileBuffer {
    data: Vec<f32>,
    size: usize,
    channels: usize,
    arena: Arc<TileArena>
}
impl TileBuffer {
    /// The tile's side length (texels per row/column), not its total element count.
    pub fn size(&self) -> usize {
        self.size
    }

    pub fn channels(&self) -> usize {
        self.channels
    }

    pub fn plane(&self, channel: usize) -> &[f32] {
        let n = self.size * self.size;
        &self.data[channel * n..(channel + 1) * n]
    }

    pub fn plane_mut(&mut self, channel: usize) -> &mut [f32] {
        let n = self.size * self.size;
        &mut self.data[channel * n..(channel + 1) * n]
    }
}
impl std::ops::Deref for TileBuffer {
    type Target = [f32];
    fn deref(&self) -> &Self::Target {
        &self.data
    }
}
impl std::ops::DerefMut for TileBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}
impl Drop for TileBuffer {
    fn drop(&mut self) {
        self.arena
            .recycle((self.size, self.channels), std::mem::take(&mut self.data));
    }
}

const INITIAL_POOL_CAPACITY: usize = 16;

type TileShape = (usize, usize); // (tile_length, channels)

#[derive(Debug, Default)]
pub struct TileArena {
    free: Mutex<HashMap<TileShape, Vec<Vec<f32>>>>,
    /// Never shrunk back to the allocator, so this also doubles as each shape bucket's current heap footprint in tiles.
    allocated_tiles: Mutex<HashMap<TileShape, usize>>
}
impl TileArena {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    fn allocate(&self, shape: TileShape) -> Vec<f32> {
        let pooled = self.free.lock().unwrap().get_mut(&shape).and_then(Vec::pop);
        match pooled {
            Some(tile) => tile,
            None => {
                *self
                    .allocated_tiles
                    .lock()
                    .unwrap()
                    .entry(shape)
                    .or_insert(0) += 1;
                vec![0.0; shape.0 * shape.0 * shape.1]
            }
        }
    }

    fn recycle(&self, shape: TileShape, tile: Vec<f32>) {
        self.free
            .lock()
            .unwrap()
            .entry(shape)
            .or_default()
            .push(tile);
    }

    /// Heap bytes reserved across every tile shape this arena has ever handed out.
    pub fn allocated_bytes(&self) -> usize {
        self.allocated_tiles
            .lock()
            .unwrap()
            .iter()
            .map(|(&(tile_length, channels), &count)| {
                count * tile_length * tile_length * channels * std::mem::size_of::<f32>()
            })
            .sum()
    }
}

/// A single tile-size view onto a [`TileArena`]
#[derive(Debug)]
pub struct TilePool {
    arena: Arc<TileArena>,
    tile_length: usize
}
impl TilePool {
    /// Standalone pool with its own private arena - handy for tests or one-off callers that don't
    /// need to share memory accounting with the rest of the graph.
    pub fn new(tile_length: usize) -> Arc<Self> {
        Self::from_arena(&TileArena::new(), tile_length)
    }

    /// A view over `arena` at `tile_length` - the constructor used by the graph so every pass
    /// shares the same two long-lived arenas instead of spawning its own.
    pub fn from_arena(arena: &Arc<TileArena>, tile_length: usize) -> Arc<Self> {
        {
            let mut free = arena.free.lock().unwrap();
            let shape = (tile_length, 1);
            if let std::collections::hash_map::Entry::Vacant(entry) = free.entry(shape) {
                let prefilled = (0..INITIAL_POOL_CAPACITY)
                    .map(|_| vec![0.0; tile_length * tile_length])
                    .collect();
                *arena
                    .allocated_tiles
                    .lock()
                    .unwrap()
                    .entry(shape)
                    .or_insert(0) += INITIAL_POOL_CAPACITY;
                entry.insert(prefilled);
            }
        }
        Arc::new(Self {
            arena: Arc::clone(arena),
            tile_length
        })
    }

    pub fn arena(&self) -> &Arc<TileArena> {
        &self.arena
    }

    /// If the arena's free list for this shape is empty, a new zero-filled tile is created.
    /// When the returned [`TileBuffer`] is dropped, its buffer is returned to the arena for reuse.
    pub fn allocate(self: &Arc<Self>) -> TileBuffer {
        self.allocate_channels(1)
    }

    /// Like [`Self::allocate`] but for a tile with `channels` planes (e.g. 3 for an RGB `Color` tile).
    pub fn allocate_channels(self: &Arc<Self>, channels: usize) -> TileBuffer {
        TileBuffer {
            data: self.arena.allocate((self.tile_length, channels)),
            size: self.tile_length,
            channels,
            arena: Arc::clone(&self.arena)
        }
    }

    pub fn tile_length(&self) -> usize {
        self.tile_length
    }
}
