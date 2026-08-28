//! A pool of tiles that can be allocated and deallocated.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

pub type TileHandle = Arc<TileBuffer>;

/// Allocated from a [`TilePool`]; returned to the pool when dropped. Deref to a `[f32]` slice for the data.
#[derive(Debug, Clone)]
pub struct TileBuffer {
    data: Vec<f32>,
    pool: Arc<TilePool>
}
impl TileBuffer {
    pub fn size(&self) -> usize {
        self.pool.tile_length()
    }

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
        self.pool
            .free
            .lock()
            .unwrap()
            .push(std::mem::take(&mut self.data));
    }
}

#[derive(Debug)]
pub struct TilePool {
    free: Mutex<Vec<Vec<f32>>>,
    tile_length: usize,
    /// Never shrunk back to the allocator, so this also doubles as the pool's current heap footprint in tiles.
    allocated_tiles: AtomicUsize
}
impl TilePool {
    /// Pools are shared by nodes across the graph, so they're always handed out behind an [`Arc`].
    pub fn new(tile_length: usize) -> Arc<Self> {
        Arc::new(Self {
            free: Mutex::new(Vec::new()),
            tile_length,
            allocated_tiles: AtomicUsize::new(0)
        })
    }

    /// If the pool is empty, a new zero-filled tile is created.
    pub fn allocate(self: &Arc<Self>) -> TileBuffer {
        let tile = self.free.lock().unwrap().pop().unwrap_or_else(|| {
            self.allocated_tiles.fetch_add(1, Ordering::Relaxed);
            vec![0.0; self.tile_length * self.tile_length]
        });
        TileBuffer {
            data: tile,
            pool: Arc::clone(self)
        }
    }

    pub fn tile_length(&self) -> usize {
        self.tile_length
    }
}
