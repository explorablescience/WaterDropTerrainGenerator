use bevy::prelude::*;
use wde::prelude::*;

use crate::{
    TerrainSessionHolder,
    core::{
        graph::GraphNodeId,
        node::NodeError::InputNotConnected,
        tiling::{ChunkCoord, crop_padding}
    },
    render::{
        chunk_array::{ChunkInstance, TerrainPreviewSync},
        generate_chunks::{TerrainPreview, queue_layer_write, set_chunk_data, sync_preview_state},
        utils::padded_heightmap
    }
};

/// Renders `selected_node`'s own whole-terrain result (see `TileContext::for_global`) as one mesh
/// covering the same real world extent the chunked terrain does.
#[allow(clippy::too_many_arguments)]
pub(super) fn update_render_chunks_global(
    asset_server: &AssetServer,
    terrain_preview: &mut TerrainPreview,
    terrain_preview_sync: &mut TerrainPreviewSync,
    terrain_graph: &TerrainSessionHolder,
    material_handle: Handle<PbrMaterial>,
    selected_node: GraphNodeId,
    native_resolution: usize,
    force: bool
) {
    let (world_extent_x, world_extent_y) = terrain_graph.read().graph().chunk_grid().world_extent();
    let chunk = ChunkCoord(0, 0);

    // Drop every other per-chunk entry *before* touching the padding/stitching machinery below
    terrain_preview.chunks.retain(|c, _| *c == chunk);

    // Compute the new chunk data (or use the existing one). Sync as global nodes are not parallelizable.
    let changed = {
        let _span = debug_span!("update_render_chunks_global_process", selected_node = ?selected_node, native_resolution = native_resolution).entered();
        let mut changed = false;
        match terrain_graph
            .write()
            .process_sync(selected_node, chunk, force)
        {
            Ok(Some((_, tiles))) => {
                if let Some(heightmap) = tiles.first() {
                    // Set the chunk's data to the cropped heightmap
                    let internal_size = heightmap.size();
                    let data = crop_padding(heightmap, internal_size, native_resolution);
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

                // If the chunk was already flat, don't overwrite it with a new flat chunk (to avoid unnecessary mesh regeneration)
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
        changed
    };

    // If the chunk's data changed, queue its (padded) heightmap for upload into layer 0.
    if changed {
        let _span = debug_span!("update_render_chunks_global_upload", chunk = ?chunk).entered();
        let padded = padded_heightmap(chunk, native_resolution, &terrain_preview.chunks);
        queue_layer_write(terrain_preview, 0, padded);
    }

    // The global preview is always exactly one instance, covering the terrain's real world extent
    let cell_size = world_extent_x / native_resolution as f32;
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
        native_resolution,
        instances,
        |_| 0
    );
}
