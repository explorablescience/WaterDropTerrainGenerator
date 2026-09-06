use std::collections::HashMap;
use wde::prelude::*;

use bevy::prelude::*;
use wde::wde_renderer::{
    assets::{Mesh, MeshBbox},
    passes::Vertex
};

use crate::{
    core::tiling::{ChunkCoord, ChunkGrid},
    render::generate_chunks::ChunkPreview
};

/// Builds the single flat grid [`Mesh`] shared by every chunk instance.
pub fn build_shared_chunk_mesh(size: usize) -> Mesh {
    let _span = debug_span!("build_shared_chunk_mesh", size = size).entered();
    let verts = size + 1;

    let mut vertices = Vec::with_capacity(verts * verts);
    for z in 0..verts {
        for x in 0..verts {
            vertices.push(Vertex {
                position: [x as f32, 0.0, z as f32],
                uv: [x as f32 / size as f32, z as f32 / size as f32],
                normal: [0.0, 1.0, 0.0],
                tangent: [1.0, 0.0, 0.0, 1.0]
            });
        }
    }

    // Build indices for a triangle list
    let mut indices = Vec::with_capacity(size * size * 6);
    for z in 0..size {
        for x in 0..size {
            let row = verts as u32;
            let v0 = z as u32 * row + x as u32;
            let v1 = v0 + 1;
            let v2 = v0 + row;
            let v3 = v2 + 1;
            indices.extend_from_slice(&[v0, v2, v1, v1, v2, v3]);
        }
    }

    Mesh {
        label: "terrain-preview-shared-chunk".to_string(),
        vertices,
        indices,
        bbox: MeshBbox {
            min: Vec3::ZERO,
            max: Vec3::new(size as f32, 0.0, size as f32)
        },
        use_ssbo: false
    }
}

/// `chunk`'s local `(0, 0)` texel, in world space.
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

/// Builds `chunk`'s `(tile_size + 3)²` heightmap, padded with neighbor-sampled edges for smooth border normals.
pub(super) fn padded_heightmap(
    chunk: ChunkCoord,
    tile_size: usize,
    chunks: &HashMap<ChunkCoord, ChunkPreview>
) -> Vec<f32> {
    let _span = debug_span!("padded_heightmap", chunk = ?chunk, tile_size = tile_size).entered();
    let padded = tile_size + 3;
    let mut out = vec![0.0; padded * padded];

    // Fast path: the core (padded index [1, tile_size]) is always this chunk's own data, so copy
    // it directly instead of a per-texel HashMap lookup. Guarded by a length check: a chunk whose
    // background job hasn't caught up with a just-changed tile size yet still holds data sized
    // for the *old* tile_size, which this array's indexing would otherwise overrun.
    if let Some(own) = chunks.get(&chunk)
        && own.core_data.len() == tile_size * tile_size
    {
        for z in 0..tile_size {
            let src = &own.core_data[z * tile_size..(z + 1) * tile_size];
            let dst = (z + 1) * padded + 1;
            out[dst..dst + tile_size].copy_from_slice(src);
        }
    }

    // Slow path: padding is asymmetric (1 texel low, 2 high — heightmap_to_mesh needs vertex
    // indices -1..=tile_size+1), so only these can come from a neighboring chunk.
    let border = || (0..=0usize).chain(tile_size + 1..=tile_size + 2);
    for pz in border() {
        let lz = pz as isize - 1;
        for px in 0..padded {
            let lx = px as isize - 1;
            out[pz * padded + px] = sample_across_chunks(chunk, tile_size, chunks, lx, lz);
        }
    }
    for pz in 1..=tile_size {
        let lz = pz as isize - 1;
        for px in border() {
            let lx = px as isize - 1;
            out[pz * padded + px] = sample_across_chunks(chunk, tile_size, chunks, lx, lz);
        }
    }
    out
}

/// Samples a texel from `chunk` or a neighbor, clamping to the edge if that chunk is missing or has no data yet.
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
    if let Some(preview) = chunks.get(&neighbor)
        && preview.core_data.len() == tile_size * tile_size
    {
        return preview.core_data[sz as usize * tile_size + sx as usize];
    }
    // Grid edge, no data yet, or the neighbor's data doesn't match `tile_size` (its background
    // job hasn't caught up with a just-changed tile size yet).
    let Some(own) = chunks.get(&chunk) else {
        return 0.0;
    };
    if own.core_data.len() != tile_size * tile_size {
        return 0.0;
    }
    let csx = lx.clamp(0, size - 1) as usize;
    let csz = lz.clamp(0, size - 1) as usize;
    own.core_data[csz * tile_size + csx]
}
/// Splits a tile-relative coordinate into (neighbor chunk offset, texel within it).
fn locate(l: isize, size: isize) -> (i32, isize) {
    (l.div_euclid(size) as i32, l.rem_euclid(size))
}
