//! Spatial context handed to a node's `process` call, letting it sample a coordinate frame that's
//! consistent across chunk boundaries (see [`crate::core::tiling::ChunkGrid`]).

use crate::core::tiling::grid::{ChunkCoord, ChunkGrid};

/// Position-aware nodes (noise generators, world-space masks) use this to sample consistently across chunk borders; kernel-only nodes can ignore it entirely.
#[derive(Debug, Clone, Copy)]
pub struct TileContext {
    /// `None` during the whole-terrain pass used to evaluate a `Global` node (see [`TileContext::for_global`]), which computes the whole terrain in one tile rather than any one chunk.
    pub chunk: Option<ChunkCoord>,
    /// World-space position of this tile's (0, 0) texel. For a chunk, already offset for the margin ring around its core region.
    pub world_origin: (f32, f32),
    /// World units covered by one texel, per axis.
    pub world_step: (f32, f32),
    /// World-space size of the tile being produced, per axis.
    pub world_extent: (f32, f32)
}
impl TileContext {
    /// Creates a `TileContext` for a `Global` node, covering the entire terrain.
    pub fn for_global(chunk_grid: &ChunkGrid, native_resolution: usize) -> TileContext {
        let (ex, ey) = chunk_grid.world_extent();
        TileContext {
            chunk: None,
            world_origin: (-ex * 0.5, -ey * 0.5),
            world_step: (ex / native_resolution as f32, ey / native_resolution as f32),
            world_extent: (ex, ey)
        }
    }

    /// World-space position of texel `(x, y)` of the tile being produced.
    pub fn world_pos(&self, x: usize, y: usize) -> (f32, f32) {
        (
            self.world_origin.0 + x as f32 * self.world_step.0,
            self.world_origin.1 + y as f32 * self.world_step.1
        )
    }

    /// Inverse of [`Self::world_pos`]: converts a world position to fractional texel coordinates.
    pub fn to_texel(&self, pos: (f32, f32)) -> (f32, f32) {
        (
            (pos.0 - self.world_origin.0) / self.world_step.0,
            (pos.1 - self.world_origin.1) / self.world_step.1
        )
    }
}
