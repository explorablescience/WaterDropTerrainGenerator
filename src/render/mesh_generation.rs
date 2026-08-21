use wde::prelude::*;
use bevy::prelude::*;

use crate::render::{CELL_SIZE, HEIGHT_SCALE};

/// Builds a grid [`Mesh`] from a square, row-major heightmap, centered on the origin in the XZ plane.
/// Per-vertex normals are estimated from the heightmap slope so the PBR shading (and shadows) read correctly.
pub fn heightmap_to_mesh(label: &str, heightmap: &[f32], size: usize) -> Mesh {
    let sample = |x: isize, z: isize| -> f32 {
        let x = x.clamp(0, size as isize - 1) as usize;
        let z = z.clamp(0, size as isize - 1) as usize;
        heightmap[z * size + x]
    };

    let offset = (size - 1) as f32 * 0.5;
    let mut vertices = Vec::with_capacity(size * size);
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    for z in 0..size {
        for x in 0..size {
            let height = sample(x as isize, z as isize) * HEIGHT_SCALE;
            let position = Vec3::new(
                (x as f32 - offset) * CELL_SIZE,
                height,
                (z as f32 - offset) * CELL_SIZE,
            );

            // Central-difference slope estimate, used as a smooth per-vertex normal.
            let dx = (sample(x as isize + 1, z as isize) - sample(x as isize - 1, z as isize)) * HEIGHT_SCALE;
            let dz = (sample(x as isize, z as isize + 1) - sample(x as isize, z as isize - 1)) * HEIGHT_SCALE;
            let normal = Vec3::new(-dx, 2.0 * CELL_SIZE, -dz).normalize();

            min = min.min(position);
            max = max.max(position);
            vertices.push(Vertex {
                position: [position.x, position.y, position.z],
                uv: [
                    x as f32 / (size - 1) as f32,
                    z as f32 / (size - 1) as f32,
                ],
                normal: [normal.x, normal.y, normal.z],
                tangent: [1.0, 0.0, 0.0, 1.0],
            });
        }
    }

    let mut indices = Vec::with_capacity((size - 1) * (size - 1) * 6);
    for z in 0..size - 1 {
        for x in 0..size - 1 {
            let row = size as u32;
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
        use_ssbo: true,
    }
}
