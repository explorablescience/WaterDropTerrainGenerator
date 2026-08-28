use std::collections::HashMap;
use wde::prelude::*;

use bevy::prelude::*;
use rayon::prelude::*;
use wde::wde_renderer::{assets::{Mesh, MeshBbox}, passes::Vertex};

use crate::{
    core::tiling::{ChunkCoord, ChunkGrid},
    render::generate_chunks::ChunkPreview
};

/// World-space height gained per unit of heightmap value.
const HEIGHT_SCALE: f32 = 1.0;

/// Builds a grid [`Mesh`] from a heightmap.
pub fn heightmap_to_mesh(
    label: &str,
    heightmap: &[f32],
    size: usize,
    cell_size: f32,
    world_offset: Vec3
) -> Mesh {
    let _span = debug_span!("heightmap_to_mesh", label = label, size = size).entered();

    // `heightmap` is padded 1 texel per side, so (x, z) and its normal-sample neighbors always land in-bounds at +1.
    let padded = size + 3;
    let verts = size + 1;

    // One row at a time, in parallel (rows are independent).
    let rows: Vec<(Vec<Vertex>, Vec3, Vec3)> = (0..verts)
        .into_par_iter()
        .map(|z| {
            let row_above = &heightmap[z * padded..(z + 1) * padded];
            let row_center = &heightmap[(z + 1) * padded..(z + 2) * padded];
            let row_below = &heightmap[(z + 2) * padded..(z + 3) * padded];

            let mut min = Vec3::splat(f32::MAX);
            let mut max = Vec3::splat(f32::MIN);
            let mut row = Vec::with_capacity(verts);
            for x in 0..verts {
                let height = row_center[x + 1] * HEIGHT_SCALE;
                let position =
                    world_offset + Vec3::new(x as f32 * cell_size, height, z as f32 * cell_size);

                // Central-difference slope -> smooth per-vertex normal.
                let dx = (row_center[x + 2] - row_center[x]) * HEIGHT_SCALE;
                let dz = (row_below[x + 1] - row_above[x + 1]) * HEIGHT_SCALE;
                let normal = Vec3::new(-dx, 2.0 * cell_size, -dz).normalize();

                min = min.min(position);
                max = max.max(position);
                row.push(Vertex {
                    position: [position.x, position.y, position.z],
                    uv: [x as f32 / size as f32, z as f32 / size as f32],
                    normal: [normal.x, normal.y, normal.z],
                    tangent: [1.0, 0.0, 0.0, 1.0]
                });
            }
            (row, min, max)
        })
        .collect();

    let mut vertices = Vec::with_capacity(verts * verts);
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    for (row, row_min, row_max) in rows {
        vertices.extend(row);
        min = min.min(row_min);
        max = max.max(row_max);
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
        label: label.to_string(),
        vertices,
        indices,
        bbox: MeshBbox { min, max },
        use_ssbo: false // custom buffers, to avoid overflow
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
    // it directly instead of a per-texel HashMap lookup.
    if let Some(own) = chunks.get(&chunk) {
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
    if let Some(preview) = chunks.get(&neighbor) {
        return preview.core_data[sz as usize * tile_size + sx as usize];
    }
    // Grid edge, or no data yet.
    let Some(own) = chunks.get(&chunk) else {
        return 0.0;
    };
    let csx = lx.clamp(0, size - 1) as usize;
    let csz = lz.clamp(0, size - 1) as usize;
    own.core_data[csz * tile_size + csx]
}
/// Splits a tile-relative coordinate into (neighbor chunk offset, texel within it).
fn locate(l: isize, size: isize) -> (i32, isize) {
    (l.div_euclid(size) as i32, l.rem_euclid(size))
}
