use std::collections::HashMap;

use bevy::prelude::*;
use wde::wde_renderer::{assets::{Mesh, MeshBbox}, passes::Vertex};

use crate::{
    core::tiling::{ChunkCoord, ChunkGrid},
    render::generate_chunks::ChunkPreview
};

/// World-space height gained per unit of heightmap value.
const HEIGHT_SCALE: f32 = 1.0;

/// Builds a grid [`Mesh`] from a heightmap, with the given world-space cell size and world-space offset for the mesh's origin.
pub fn heightmap_to_mesh(
    label: &str,
    heightmap: &[f32],
    size: usize,
    cell_size: f32,
    world_offset: Vec3
) -> Mesh {
    // Remove padding
    let padded = size + 3;
    let sample = |x: isize, z: isize| -> f32 {
        let x = (x + 1).clamp(0, padded as isize - 1) as usize;
        let z = (z + 1).clamp(0, padded as isize - 1) as usize;
        heightmap[z * padded + x]
    };

    // Build vertices and normals
    let verts = size + 1;
    let mut vertices = Vec::with_capacity(verts * verts);
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    for z in 0..verts {
        for x in 0..verts {
            let height = sample(x as isize, z as isize) * HEIGHT_SCALE;
            let position =
                world_offset + Vec3::new(x as f32 * cell_size, height, z as f32 * cell_size);

            // Central-difference slope estimate, used as a smooth per-vertex normal.
            let dx = (sample(x as isize + 1, z as isize) - sample(x as isize - 1, z as isize))
                * HEIGHT_SCALE;
            let dz = (sample(x as isize, z as isize + 1) - sample(x as isize, z as isize - 1))
                * HEIGHT_SCALE;
            let normal = Vec3::new(-dx, 2.0 * cell_size, -dz).normalize();

            min = min.min(position);
            max = max.max(position);
            vertices.push(Vertex {
                position: [position.x, position.y, position.z],
                uv: [x as f32 / size as f32, z as f32 / size as f32],
                normal: [normal.x, normal.y, normal.z],
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
        label: label.to_string(),
        vertices,
        indices,
        bbox: MeshBbox { min, max },
        use_ssbo: false // Using custom buffers to avoid overflow
    }
}


/// World-space position of `chunk`'s local `(0, 0)` texel.
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

/// Builds a padded heightmap for `chunk` by sampling neighboring chunks as needed. The returned heightmap is `(tile_size + 3) x (tile_size + 3)` in size, with the extra padding used for smooth normal calculation at the edges of the chunk.
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

/// Samples a texel from the given chunk or one of its neighbors, clamping to the edge of the chunk if the neighbor is missing or has no data yet.
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
    // No such chunk (grid edge) or no data yet.
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
