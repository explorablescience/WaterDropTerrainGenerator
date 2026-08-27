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

    /// Total bytes currently reserved on the heap for this pool's tiles (free or in use).
    pub fn allocated_bytes(&self) -> usize {
        self.allocated_tiles.load(Ordering::Relaxed)
            * self.tile_length
            * self.tile_length
            * std::mem::size_of::<f32>()
    }
}

/// Clamps to the grid's edge when `x, y` fall outside `[0, size - 1]`.
pub fn bilinear_sample(data: &[f32], size: usize, x: f32, y: f32) -> f32 {
    let px = |x: usize, y: usize| data[y * size + x];

    let fx = x.clamp(0.0, (size - 1) as f32);
    let fy = y.clamp(0.0, (size - 1) as f32);
    let x0 = fx as usize;
    let y0 = fy as usize;
    let x1 = (x0 + 1).min(size - 1);
    let y1 = (y0 + 1).min(size - 1);
    let tx = fx - x0 as f32;
    let ty = fy - y0 as f32;

    let top = px(x0, y0) * (1.0 - tx) + px(x1, y0) * tx;
    let bottom = px(x0, y1) * (1.0 - tx) + px(x1, y1) * tx;
    top * (1.0 - ty) + bottom * ty
}

/// Crops off the kernel-padding margin around a node's requested output.
pub fn crop_center(data: &[f32], full_size: usize, target_size: usize) -> Vec<f32> {
    if full_size == target_size {
        return data.to_vec();
    }
    let padding = (full_size - target_size) / 2;
    let mut cropped = Vec::with_capacity(target_size * target_size);
    for z in 0..target_size {
        let row_start = (z + padding) * full_size + padding;
        cropped.extend_from_slice(&data[row_start..row_start + target_size]);
    }
    cropped
}
