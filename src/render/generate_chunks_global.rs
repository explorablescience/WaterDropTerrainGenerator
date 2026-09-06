use bevy::prelude::*;
use wde::prelude::*;

use crate::{
    TerrainInstanceHolder,
    core::{
        CacheEntry, graph::GraphNodeId, node::NodeError::InputNotConnected, tiling::ChunkCoord
    },
    render::{
        chunk_array::{ChunkInstance, TerrainPreviewSync},
        generate_chunks::{TerrainPreview, queue_layer_write, set_chunk_data, sync_preview_state},
        utils::{MAX_PREVIEW_MESH_SIZE, padded_heightmap, resample_for_preview}
    }
};

/// Renders `selected_node`'s own whole-terrain result (see `TileContext::for_global`) as one mesh
/// covering the same real world extent the chunked terrain does.
pub(super) fn update_render_chunks_global(
    asset_server: &AssetServer,
    terrain_preview: &mut TerrainPreview,
    terrain_preview_sync: &mut TerrainPreviewSync,
    terrain_graph: &TerrainInstanceHolder,
    material_handle: Handle<PbrMaterial>,
    selected_node: GraphNodeId,
    native_resolution: usize
) {
    let chunk_grid = *terrain_graph.read().graph().chunk_grid();
    let (world_extent_x, world_extent_y) = chunk_grid.world_extent();
    let chunk = ChunkCoord(0, 0);
    let preview_size = chunk_grid.tile_size().min(MAX_PREVIEW_MESH_SIZE);

    // Drop every other per-chunk entry *before* touching the padding/stitching machinery below
    terrain_preview.chunks.retain(|c, _| *c == chunk);

    let mut changed = false;
    match terrain_graph.write().get(selected_node) {
        Ok(Some(CacheEntry::Global(uuid, tiles))) => {
            let _span = debug_span!("update_render_chunks_global_apply", selected_node = ?selected_node, native_resolution = native_resolution).entered();
            if terrain_preview.global_uuid != Some(uuid) {
                terrain_preview.global_uuid = Some(uuid);
                if let Some(heightmap) = tiles.first() {
                    let data = resample_for_preview(heightmap, native_resolution, preview_size);
                    set_chunk_data(terrain_preview, chunk, data, false);
                    changed = true;
                }
            }
        }
        Ok(Some(CacheEntry::Local(_))) => {
            warn!(
                "Global-locality node {:?} evaluated to a Local result",
                selected_node
            );
            return;
        }
        Ok(None) => return, // still processing
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

            // If the chunk was already flat, don't overwrite it with a new flat chunk (to avoid unnecessary mesh regeneration)
            let already_flat = terrain_preview
                .chunks
                .get(&chunk)
                .is_some_and(|c| c.is_flat);
            if !already_flat {
                set_chunk_data(
                    terrain_preview,
                    chunk,
                    vec![0.0; preview_size * preview_size],
                    true
                );
                changed = true;
            }
            terrain_preview.global_uuid = None;
        }
    }

    // If the chunk's data changed, queue its (padded) heightmap for upload into layer 0.
    if changed {
        let _span = debug_span!("update_render_chunks_global_upload", chunk = ?chunk).entered();
        let padded = padded_heightmap(chunk, preview_size, &terrain_preview.chunks);
        queue_layer_write(terrain_preview, 0, padded);
    }

    // Nothing changed and the mesh/array already exist: leave the GPU-facing state untouched.
    if !changed && terrain_preview.has_mesh() {
        return;
    }

    // The global preview is always exactly one instance, covering the terrain's real world extent
    let cell_size = world_extent_x / preview_size as f32;
    let instances = vec![ChunkInstance {
        world_offset: [-world_extent_x * 0.5, -world_extent_y * 0.5],
        cell_size,
        layer: 0
    }];

    // Publish the current mesh/array/instance state for the render world to pick up.
    sync_preview_state(
        asset_server,
        terrain_preview,
        terrain_preview_sync,
        material_handle,
        preview_size,
        instances,
        |_| 0
    );
}
