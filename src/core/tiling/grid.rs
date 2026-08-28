//! Describes how the terrain is partitioned into a grid of chunks, each evaluated independently.

use crate::core::tiling::context::TileContext;

/// Identifies one chunk in a [`ChunkGrid`] by its integer grid position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkCoord(pub i32, pub i32);

/// A grid of chunks, each with a square tile of texels, that together cover the whole terrain.
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
            "A chunk grid needs at least one chunk"
        );
        Self {
            chunks_x,
            chunks_y,
            tile_size,
            world_scale
        }
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
