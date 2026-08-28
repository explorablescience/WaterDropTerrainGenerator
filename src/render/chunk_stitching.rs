use std::collections::HashMap;

use bevy::prelude::*;

use crate::{
    core::tiling::{ChunkCoord, ChunkGrid},
    render::terrain_preview::ChunkPreview
};

/// World-space position of `chunk`'s local `(0, 0)` texel
pub(super) fn chunk_origin(chunk: ChunkCoord, grid: &ChunkGrid) -> Vec3 {
    let step = grid.tile_size() as f32 * grid.world_scale();
    let extent_x = grid.chunks_x() as f32 * step;
    let extent_y = grid.chunks_y() as f32 * step;
    Vec3::new(
        chunk.0 as f32 * step - extent_x * 0.5,
        0.0,
        chunk.1 as f32 * step - extent_y * 0.5
    )
}

/// Builds `chunk`'s `(tile_size + 3) x (tile_size + 3)` heightmap for [`heightmap_to_mesh`](super::mesh_generation::heightmap_to_mesh)
pub(super) fn padded_heightmap(
    chunk: ChunkCoord,
    tile_size: usize,
    chunks: &HashMap<ChunkCoord, ChunkPreview>
) -> Vec<f32> {
    let padded = tile_size + 3;
    let mut out = vec![0.0; padded * padded];
    for pz in 0..padded {
        for px in 0..padded {
            let lx = px as isize - 1;
            let lz = pz as isize - 1;
            out[pz * padded + px] = sample_across_chunks(chunk, tile_size, chunks, lx, lz);
        }
    }
    out
}

/// Samples core-tile texel `(lx, lz)` relative to `chunk`'s own origin
fn sample_across_chunks(
    chunk: ChunkCoord,
    tile_size: usize,
    chunks: &HashMap<ChunkCoord, ChunkPreview>,
    lx: isize,
    lz: isize
) -> f32 {
    let size = tile_size as isize;
    let (dx, sx) = locate(lx, size);
    let (dz, sz) = locate(lz, size);
    let neighbor = ChunkCoord(chunk.0 + dx, chunk.1 + dz);
    if let Some(preview) = chunks.get(&neighbor) {
        return preview.core_data[sz as usize * tile_size + sx as usize];
    }
    // No such chunk (grid edge) or no data yet: clamp within this chunk's own tile, same as an unchunked tile always did at its own edge.
    let Some(own) = chunks.get(&chunk) else {
        return 0.0;
    };
    let csx = lx.clamp(0, size - 1) as usize;
    let csz = lz.clamp(0, size - 1) as usize;
    own.core_data[csz * tile_size + csx]
}

/// Splits a core-tile-relative coordinate into which neighboring chunk to sample from and which texel within that chunk's tile to read.
fn locate(l: isize, size: isize) -> (i32, isize) {
    (l.div_euclid(size) as i32, l.rem_euclid(size))
}
