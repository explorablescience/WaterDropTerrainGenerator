use bevy::prelude::*;
use wde::prelude::*;

use crate::{
    TerrainSessionHolder,
    core::{
        graph::GraphNodeId,
        node::NodeError::InputNotConnected,
        tiling::{ChunkCoord, crop_center}
    },
    render::{
        chunk_stitching::padded_heightmap,
        mesh_generation::heightmap_to_mesh,
        terrain_preview::{TerrainPreview, publish_preview_meshes, set_chunk_data, upsert_chunk_mesh},
        terrain_preview_subpass::TerrainPreviewMeshes
    }
};

/// World-space distance between adjacent heightmap samples for the global (non-tiled) preview.
const GLOBAL_CELL_SIZE: f32 = 0.1;

/// Renders `selected_node`'s own bare, self-centered result (see `TileContext::for_global`) as one mesh, centered at the world origin.
#[allow(clippy::too_many_arguments)]
pub(super) fn update_global_preview(
    meshes: &mut Assets<Mesh>,
    terrain_preview: &mut TerrainPreview,
    terrain_preview_meshes: &mut TerrainPreviewMeshes,
    terrain_graph: &TerrainSessionHolder,
    material_handle: Handle<PbrMaterial>,
    selected_node: GraphNodeId,
    native_resolution: usize,
    force: bool
) {
    let chunk = ChunkCoord(0, 0);

    // Drop every other per-chunk entry *before* touching the padding/stitching machinery below
    terrain_preview.chunks.retain(|c, _| *c == chunk);

    let mut changed = false;

    match terrain_graph
        .write()
        .process_chunk(selected_node, chunk, force)
    {
        Ok(Some((_, tiles))) => {
            if let Some(heightmap) = tiles.first() {
                let internal_size = heightmap.size();
                let data = crop_center(heightmap, internal_size, native_resolution);
                set_chunk_data(terrain_preview, chunk, data, false);
                changed = true;
            }
        }
        Ok(None) => {}
        Err(e) => {
            match e {
                InputNotConnected { node, socket, .. } => {
                    trace!(
                        "Cannot generate global preview: Input not connected for node '{}' at socket {}",
                        node, socket
                    );
                }
                _ => error!(
                    "Error while processing terrain graph for the global preview: {:?}",
                    e
                )
            }
            let already_flat = terrain_preview
                .chunks
                .get(&chunk)
                .is_some_and(|c| c.is_flat);
            if !already_flat {
                set_chunk_data(
                    terrain_preview,
                    chunk,
                    vec![0.0; native_resolution * native_resolution],
                    true
                );
                changed = true;
            }
        }
    }

    if changed {
        let padded = padded_heightmap(chunk, native_resolution, &terrain_preview.chunks);
        let extent = native_resolution as f32 * GLOBAL_CELL_SIZE;
        let world_offset = Vec3::new(-extent * 0.5, 0.0, -extent * 0.5);
        let mesh = heightmap_to_mesh(
            "terrain-preview-global",
            &padded,
            native_resolution,
            GLOBAL_CELL_SIZE,
            world_offset
        );
        upsert_chunk_mesh(meshes, terrain_preview, chunk, mesh);
    }

    publish_preview_meshes(terrain_preview, terrain_preview_meshes, material_handle);
}
