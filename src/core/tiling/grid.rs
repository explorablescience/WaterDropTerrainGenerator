//! Describes how the terrain is partitioned into a grid of chunks, each evaluated independently.

use crate::core::tiling::context::TileContext;

/// Identifies one chunk in a [`ChunkGrid`] by its integer grid position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkCoord(pub i32, pub i32);

/// The terrain-level layout that chunked evaluation is parameterized by: how many chunks make up
/// the terrain, how many texels each chunk covers, and how texel indices map to a shared
/// world-space coordinate frame so position-aware nodes (e.g. noise generators) sample
/// consistently across chunk boundaries.
#[derive(Debug, Clone, Copy)]
pub struct ChunkGrid {
    chunks_x: u32,
    chunks_y: u32,
    /// Core (non-margin) texels per chunk edge.
    tile_size: usize,
    /// World units covered by one texel.
    world_scale: f32
}
impl ChunkGrid {
    pub fn new(chunks_x: u32, chunks_y: u32, tile_size: usize, world_scale: f32) -> Self {
        assert!(
            chunks_x > 0 && chunks_y > 0,
            "a chunk grid needs at least one chunk"
        );
        Self {
            chunks_x,
            chunks_y,
            tile_size,
            world_scale
        }
    }

    /// A degenerate 1x1 grid spanning the whole terrain in a single chunk.
    pub fn single(tile_size: usize) -> Self {
        Self::new(1, 1, tile_size, 1.0 / tile_size as f32)
    }

    pub fn chunks_x(&self) -> u32 {
        self.chunks_x
    }
    pub fn chunks_y(&self) -> u32 {
        self.chunks_y
    }
    pub fn tile_size(&self) -> usize {
        self.tile_size
    }
    pub fn world_scale(&self) -> f32 {
        self.world_scale
    }
    pub fn chunk_count(&self) -> u32 {
        self.chunks_x * self.chunks_y
    }

    /// Every chunk coordinate in the grid, in row-major order.
    pub fn coords(&self) -> impl Iterator<Item = ChunkCoord> + '_ {
        (0..self.chunks_y)
            .flat_map(move |y| (0..self.chunks_x).map(move |x| ChunkCoord(x as i32, y as i32)))
    }

    /// World-space size of the whole terrain covered by this grid.
    pub fn world_extent(&self) -> (f32, f32) {
        (
            self.chunks_x as f32 * self.tile_size as f32 * self.world_scale,
            self.chunks_y as f32 * self.tile_size as f32 * self.world_scale
        )
    }

    /// World-space position of `chunk`'s core (0, 0) texel, i.e. excluding any margin.
    fn chunk_world_origin(&self, chunk: ChunkCoord) -> (f32, f32) {
        let (ex, ey) = self.world_extent();
        (
            chunk.0 as f32 * self.tile_size as f32 * self.world_scale - ex * 0.5,
            chunk.1 as f32 * self.tile_size as f32 * self.world_scale - ey * 0.5
        )
    }

    /// Context for computing `chunk`, whose tile carries `margin` texels of padding on every side
    /// (see `NodeGraph::required_internal_tile_size`) so kernel-based nodes can sample past the
    /// chunk's own edge without seams.
    pub fn chunk_context(&self, chunk: ChunkCoord, margin: usize) -> TileContext {
        let (ox, oy) = self.chunk_world_origin(chunk);
        let step = self.world_scale;
        TileContext {
            chunk: Some(chunk),
            world_origin: (ox - margin as f32 * step, oy - margin as f32 * step),
            world_step: (step, step),
            world_extent: self.world_extent()
        }
    }
}
