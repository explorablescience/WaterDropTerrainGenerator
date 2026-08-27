//! Spatial partitioning and buffer primitives the graph engine is built on: how the terrain is
//! carved into chunks, where texel buffers come from, and generic per-texel math over them.

mod context;
mod grid;
mod image_io;
mod pool;
mod sampling;

pub use context::TileContext;
pub use grid::{ChunkCoord, ChunkGrid};
pub use image_io::save_heightmap_png;
pub use pool::{TileBuffer, TileHandle, TilePool};
pub use sampling::{bilinear_sample, crop_center};
