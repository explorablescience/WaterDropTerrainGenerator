//! Spatial context handed to a node's `process` call, letting it sample a coordinate frame that's
//! consistent across chunk boundaries (see [`crate::core::chunk_grid::ChunkGrid`]).

use crate::core::chunk_grid::ChunkCoord;

/// Everything a [`Node::process`](crate::core::node::Node::process) implementation needs to know
/// about *where* it's computing. Position-aware nodes (noise generators, world-space masks) use
/// this to sample consistently across chunk borders; kernel-only nodes (erosion, blur, combine)
/// can ignore it entirely, same as they already ignore the tile pool today.
#[derive(Debug, Clone, Copy)]
pub struct TileContext {
    /// The chunk being computed. `None` during the single whole-terrain pass used to evaluate a
    /// `Global` node (see `NodeLocality`) - there is no single chunk in that case, the tile being
    /// produced covers the entire terrain.
    pub chunk: Option<ChunkCoord>,
    pub chunks_x: u32,
    pub chunks_y: u32,
    /// World-space position of this tile's (0, 0) texel. For a chunk, already offset to account
    /// for the margin ring around its core region.
    pub world_origin: (f32, f32),
    /// World units covered by one texel, per axis.
    pub world_step: (f32, f32),
    /// World-space size of the whole terrain, regardless of which chunk (if any) this context is
    /// for - lets a node normalize a world position into `[0, 1]` across the full terrain.
    pub world_extent: (f32, f32)
}
impl TileContext {
    /// World-space position of texel `(x, y)` of the tile being produced.
    pub fn world_pos(&self, x: usize, y: usize) -> (f32, f32) {
        (
            self.world_origin.0 + x as f32 * self.world_step.0,
            self.world_origin.1 + y as f32 * self.world_step.1
        )
    }

    /// Normalizes a world-space position into `[0, 1]` across the whole terrain's extent - handy
    /// for a node (like a heightmap import) that maps a single image across the whole terrain.
    pub fn normalize(&self, world: (f32, f32)) -> (f32, f32) {
        (world.0 / self.world_extent.0, world.1 / self.world_extent.1)
    }

    /// Returns the world-space size of the whole terrain
    pub fn world_size(&self) -> (f32, f32) {
        self.world_extent
    }
}
