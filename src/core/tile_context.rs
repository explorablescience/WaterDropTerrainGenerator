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
    /// `Global` node (see `NodeLocality`) - a `Global` node computes its own bare, self-contained
    /// result (see [`TileContext::for_global`]), not any one chunk of the terrain.
    pub chunk: Option<ChunkCoord>,
    /// World-space position of this tile's (0, 0) texel. For a chunk, already offset to account
    /// for the margin ring around its core region.
    pub world_origin: (f32, f32),
    /// World units covered by one texel, per axis.
    pub world_step: (f32, f32),
    /// World-space size of the tile being produced, per axis.
    pub world_extent: (f32, f32)
}
impl TileContext {
    /// Constructs a `TileContext` for a `Global` node, which is evaluated once for the whole
    /// terrain at its own `native_resolution` (see [`NodeLocality`]).
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

    /// Alias of [`Self::world_pos`] for use inside a `Global` node's own `process()`: there, this
    /// context's frame *is* the node's bare local space (see [`Self::for_global`]), so a call site
    /// reads truer to what it actually means as `local_pos` than as `world_pos`.
    pub fn local_pos(&self, x: usize, y: usize) -> (f32, f32) {
        self.world_pos(x, y)
    }

    /// A position already expressed in this context's own frame -> fractional texel indices into
    /// the tile it describes. The inverse of [`Self::world_pos`]/[`Self::local_pos`] - e.g. an
    /// integration node uses this to turn a `Global` input's own local position (from
    /// [`Self::to_local`]) into texel coordinates it can bilinearly sample that input at.
    pub fn to_texel(&self, pos: (f32, f32)) -> (f32, f32) {
        (
            (pos.0 - self.world_origin.0) / self.world_step.0,
            (pos.1 - self.world_origin.1) / self.world_step.1
        )
    }

    /// Normalizes a world-space position into `[0, 1)` across this context's `world_extent`.
    pub fn normalize(&self, world: (f32, f32)) -> (f32, f32) {
        (world.0 / self.world_extent.0 + 0.5, world.1 / self.world_extent.1 + 0.5)
    }

    /// Maps a world-space position into an arbitrarily placed frame's own local space, given that
    /// frame's `position` (world-space location of its local origin) and `scale` (world units its
    /// local unit spans). This is what an integration node applies to go from the terrain's world
    /// position to wherever its `Global` input's own bare, self-centered result (see
    /// [`Self::for_global`]) should be read from - the counterpart to [`Self::to_texel`], which
    /// then turns that local position into actual texel coordinates.
    pub fn to_local(world: (f32, f32), position: (f32, f32), scale: f32) -> (f32, f32) {
        ((world.0 - position.0) / scale, (world.1 - position.1) / scale)
    }
}
