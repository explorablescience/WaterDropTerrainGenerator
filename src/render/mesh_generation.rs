use bevy::prelude::*;
use wde::prelude::*;

/// World-space height gained per unit of heightmap value.
const HEIGHT_SCALE: f32 = 1.0;

/// Builds a grid [`Mesh`] of `size x size` quads - `(size + 1) x (size + 1)` vertices, spaced
/// `cell_size` world units apart so a tile's world-space footprint is `size * cell_size`
/// regardless of how many texels (`size`) it's subdivided into.
pub fn heightmap_to_mesh(
    label: &str,
    heightmap: &[f32],
    size: usize,
    cell_size: f32,
    world_offset: Vec3
) -> Mesh {
    let padded = size + 3;
    let sample = |x: isize, z: isize| -> f32 {
        let x = (x + 1).clamp(0, padded as isize - 1) as usize;
        let z = (z + 1).clamp(0, padded as isize - 1) as usize;
        heightmap[z * padded + x]
    };

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
        use_ssbo: false
    }
}
