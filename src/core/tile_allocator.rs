//! A pool of tiles that can be allocated and deallocated.

use std::sync::{Arc, Mutex};

/// A tile of data produced by a node's output socket and consumed by downstream nodes.
pub type TileHandle = Arc<TileBuffer>;

/// A buffer that holds the data of a tile.
/// It can be allocated from a [`TilePool`] and will be returned to the pool when dropped.
/// To access the data, you can dereference the buffer to get a slice of f32 values.
pub struct TileBuffer {
    data: Vec<f32>,
    pool: Arc<TilePool>,
}
impl TileBuffer {
    /// Returns the size of the tile (number of f32 values).
    pub fn size(&self) -> usize {
        self.pool.tile_length()
    }

    /// Returns true if the tile is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
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
        // Put the tile back into the pool when the buffer is dropped.
        self.pool
            .free
            .lock()
            .unwrap()
            .push(std::mem::take(&mut self.data));
    }
}

/// A pool of tiles that can be allocated and deallocated.
/// Each tile is represented by a [`TileBuffer`].
pub struct TilePool {
    free: Mutex<Vec<Vec<f32>>>,
    tile_length: usize,
}
impl TilePool {
    /// Creates a new tile pool with the given tile length.
    ///
    /// # Arguments
    /// * `tile_length` - The length of each tile in the pool in a given number of texels (e.g., 3 for a 3x3 tile, 5 for a 5x5 tile, etc.).
    ///
    /// Pools are shared by nodes across the graph, so they're always handed out behind an [`Arc`].
    pub fn new(tile_length: usize) -> Arc<Self> {
        Arc::new(Self {
            free: Mutex::new(Vec::new()),
            tile_length,
        })
    }

    /// Allocates a tile from the pool and returns a [`TileBuffer`] that holds the data of the tile.
    /// If the pool is empty, a new zero-filled tile is created.
    pub fn allocate(self: &Arc<Self>) -> TileBuffer {
        let tile = self
            .free
            .lock()
            .unwrap()
            .pop()
            .unwrap_or_else(|| vec![0.0; self.tile_length * self.tile_length]);
        TileBuffer {
            data: tile,
            pool: Arc::clone(self),
        }
    }

    pub fn tile_length(&self) -> usize {
        self.tile_length
    }
}
