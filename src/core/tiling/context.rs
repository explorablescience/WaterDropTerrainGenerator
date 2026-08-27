//! Spatial context handed to a node's `process` call, letting it sample a coordinate frame that's
//! consistent across chunk boundaries (see [`crate::core::tiling::ChunkGrid`]).

use crate::core::tiling::grid::ChunkCoord;

/// Position-aware nodes (noise generators, world-space masks) use this to sample consistently across chunk borders; kernel-only nodes can ignore it entirely.
#[derive(Debug, Clone, Copy)]
pub struct TileContext {
    /// `None` during the whole-terrain pass used to evaluate a `Global` node (see [`TileContext::for_global`]), which computes its own bare result rather than any one chunk.
    pub chunk: Option<ChunkCoord>,
    /// World-space position of this tile's (0, 0) texel. For a chunk, already offset for the margin ring around its core region.
    pub world_origin: (f32, f32),
    /// World units covered by one texel, per axis.
    pub world_step: (f32, f32),
    /// World-space size of the tile being produced, per axis.
    pub world_extent: (f32, f32)
}
impl TileContext {
    /// For a `Global` node, evaluated once for the whole terrain at its own `native_resolution`.
    pub fn for_global(native_resolution: usize) -> TileContext {
        let step = 1.0 / native_resolution as f32;
        TileContext {
            chunk: None,
            world_origin: (-0.5, -0.5),
            world_step: (step, step),
            world_extent: (1.0, 1.0)
        }
    }

    /// World-space position of texel `(x, y)` of the tile being produced.
    pub fn world_pos(&self, x: usize, y: usize) -> (f32, f32) {
        (
            self.world_origin.0 + x as f32 * self.world_step.0,
            self.world_origin.1 + y as f32 * self.world_step.1
        )
    }

    /// Alias of [`Self::world_pos`]: inside a `Global` node this context's frame *is* its bare local space, so `local_pos` reads truer at the call site.
    pub fn local_pos(&self, x: usize, y: usize) -> (f32, f32) {
        self.world_pos(x, y)
    }

    /// Inverse of [`Self::world_pos`]/[`Self::local_pos`]; e.g. an integration node uses this to turn a `Global` input's local position into texel coordinates to bilinearly sample it at.
    pub fn to_texel(&self, pos: (f32, f32)) -> (f32, f32) {
        (
            (pos.0 - self.world_origin.0) / self.world_step.0,
            (pos.1 - self.world_origin.1) / self.world_step.1
        )
    }

    /// Normalizes a world-space position into `[0, 1)` across this context's `world_extent`.
    pub fn normalize(&self, world: (f32, f32)) -> (f32, f32) {
        (
            world.0 / self.world_extent.0 + 0.5,
            world.1 / self.world_extent.1 + 0.5
        )
    }

    /// Maps world-space into an arbitrarily placed frame's local space, given that frame's `position` (world origin) and `scale` (world units per local unit). Counterpart to [`Self::to_texel`].
    pub fn to_local(world: (f32, f32), position: (f32, f32), scale: f32) -> (f32, f32) {
        (
            (world.0 - position.0) / scale,
            (world.1 - position.1) / scale
        )
    }
}
