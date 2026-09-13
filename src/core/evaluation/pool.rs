//! A pool of tiles that can be allocated and deallocated.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub type TileHandle = Arc<TileBuffer>;

/// Allocated from a [`TilePool`]; returned to its backing [`TileArena`] when dropped. Deref to a `[f32]` slice for the data.
#[derive(Debug, Clone)]
pub struct TileBuffer {
    data: Vec<f32>,
    size: usize,
    arena: Arc<TileArena>
}
impl TileBuffer {
    /// The tile's side length (texels per row/column), not its total element count.
    pub fn size(&self) -> usize {
        self.size
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
            .recycle(self.size, std::mem::take(&mut self.data));
    }
}

const INITIAL_POOL_CAPACITY: usize = 16;

#[derive(Debug, Default)]
pub struct TileArena {
    free: Mutex<HashMap<usize, Vec<Vec<f32>>>>,
    /// Never shrunk back to the allocator, so this also doubles as each size bucket's current heap footprint in tiles.
    allocated_tiles: Mutex<HashMap<usize, usize>>
}
impl TileArena {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    fn allocate(&self, tile_length: usize) -> Vec<f32> {
        let pooled = self
            .free
            .lock()
            .unwrap()
            .get_mut(&tile_length)
            .and_then(Vec::pop);
        match pooled {
            Some(tile) => tile,
            None => {
                *self
                    .allocated_tiles
                    .lock()
                    .unwrap()
                    .entry(tile_length)
                    .or_insert(0) += 1;
                vec![0.0; tile_length * tile_length]
            }
        }
    }

    fn recycle(&self, tile_length: usize, tile: Vec<f32>) {
        self.free
            .lock()
            .unwrap()
            .entry(tile_length)
            .or_default()
            .push(tile);
    }

    /// Heap bytes reserved across every tile size this arena has ever handed out.
    pub fn allocated_bytes(&self) -> usize {
        self.allocated_tiles
            .lock()
            .unwrap()
            .iter()
            .map(|(&tile_length, &count)| {
                count * tile_length * tile_length * std::mem::size_of::<f32>()
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
            if let std::collections::hash_map::Entry::Vacant(entry) = free.entry(tile_length) {
                let prefilled = (0..INITIAL_POOL_CAPACITY)
                    .map(|_| vec![0.0; tile_length * tile_length])
                    .collect();
                *arena
                    .allocated_tiles
                    .lock()
                    .unwrap()
                    .entry(tile_length)
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

    /// If the arena's free list for this size is empty, a new zero-filled tile is created.
    /// When the returned [`TileBuffer`] is dropped, its buffer is returned to the arena for reuse.
    pub fn allocate(self: &Arc<Self>) -> TileBuffer {
        TileBuffer {
            data: self.arena.allocate(self.tile_length),
            size: self.tile_length,
            arena: Arc::clone(&self.arena)
        }
    }

    pub fn tile_length(&self) -> usize {
        self.tile_length
    }
}
