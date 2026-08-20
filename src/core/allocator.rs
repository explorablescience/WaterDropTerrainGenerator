//! A pool of tiles that can be allocated and deallocated.

use std::sync::{Arc, Mutex};

type TileData = Vec<f32>;

/// A tile of data produced by a node's output socket and consumed by downstream nodes.
pub type TileHandle = Arc<TileBuffer>;

/// A buffer that holds the data of a tile.
/// It can be allocated from a [`TilePool`] and will be returned to the pool when dropped.
/// To access the data, you can dereference the buffer to get a slice of f32 values.
pub struct TileBuffer {
    data: TileData,
    pool: Arc<TilePool>,
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
    free: Mutex<Vec<TileData>>,
    tile_length: usize,
}
impl TilePool {
    /// Creates a new tile pool with the given tile length.
    ///
    /// # Arguments
    /// * `tile_length` - The length of each tile in the pool (total number of f32 values per tile).
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
            .unwrap_or_else(|| vec![0.0; self.tile_length]);
        TileBuffer {
            data: tile,
            pool: Arc::clone(self),
        }
    }

    /// The length of each tile in the pool (total number of f32 values per tile).
    pub fn tile_length(&self) -> usize {
        self.tile_length
    }
}
