use std::collections::HashSet;

use bevy::prelude::*;
use rayon::prelude::*;
use wde::prelude::*;

use crate::{
    TerrainInstanceHolder,
    core::{
        CacheEntry, graph::GraphNodeId, node::NodeError::InputNotConnected, tiling::ChunkCoord
    },
    render::{
        chunk_array::{ChunkInstance, TerrainPreviewSync},
        generate_chunks::{TerrainPreview, queue_layer_write, set_chunk_data, sync_preview_state},
        utils::{chunk_origin, padded_heightmap}
    }
};

/// Renders `selected_node`'s output tiled across every chunk of the terrain's [`ChunkGrid`](crate::core::tiling::ChunkGrid).
/// `NodeGraph::get` returns every chunk together, once the whole barrier is done - only the chunks
/// whose uuid actually changed (plus their neighbors, whose padding rings sample them) are
/// re-stitched and re-queued for GPU upload.
pub(super) fn update_render_chunks_local(
    asset_server: &AssetServer,
    terrain_preview: &mut TerrainPreview,
    terrain_preview_sync: &mut TerrainPreviewSync,
    terrain_graph: &TerrainInstanceHolder,
    material_handle: Handle<PbrMaterial>,
    selected_node: GraphNodeId
) {
    let _span = debug_span!("update_render_chunks_local", selected_node = ?selected_node).entered();
    let chunk_grid = *terrain_graph.read().graph().chunk_grid();
    let tile_size = chunk_grid.tile_size();
    let chunks_x = chunk_grid.chunks_x();
    let all_chunks: HashSet<ChunkCoord> = chunk_grid.coords().collect();

    let mut changed_chunks = HashSet::new();
    match terrain_graph.write().get(selected_node) {
        Ok(Some(CacheEntry::Local(map))) => {
            let _span =
                debug_span!("update_render_chunks_local_apply", selected_node = ?selected_node)
                    .entered();
            for (chunk, (uuid, tiles)) in map {
                if terrain_preview.chunk_uuids.get(&chunk) == Some(&uuid) {
                    continue; // unchanged since the last frame we uploaded it
                }
                terrain_preview.chunk_uuids.insert(chunk, uuid);
                let Some(heightmap) = tiles.first() else {
                    continue;
                };
                set_chunk_data(terrain_preview, chunk, heightmap.to_vec(), false);
                changed_chunks.insert(chunk);
            }
        }
        Ok(Some(CacheEntry::Global(..))) => {
            warn!(
                "Local-locality node {:?} evaluated to a Global result",
                selected_node
            );
            return;
        }
        Ok(None) => return, // still processing
        Err(e) => {
            match e {
                InputNotConnected { .. } => {} // Fine, just means the user hasn't connected the selected node's input yet
                _ => error!(
                    "Error while processing terrain graph for node {:?}: {:?}",
                    selected_node, e
                )
            }

            // Show a flat chunk instead of whatever was last known, unless it already is one.
            for &chunk in &all_chunks {
                let already_flat = terrain_preview
                    .chunks
                    .get(&chunk)
                    .is_some_and(|c| c.is_flat);
                if !already_flat {
                    set_chunk_data(
                        terrain_preview,
                        chunk,
                        vec![0.0; tile_size * tile_size],
                        true
                    );
                    changed_chunks.insert(chunk);
                }
            }
            terrain_preview.chunk_uuids.clear();
        }
    }

    // A changed chunk's neighbors also need re-stitching/re-upload - their border texels sample it.
    let chunks_to_upload: HashSet<_> = changed_chunks
        .iter()
        .flat_map(|chunk| {
            (-1..=1)
                .flat_map(move |dz| (-1..=1).map(move |dx| ChunkCoord(chunk.0 + dx, chunk.1 + dz)))
        })
        .filter(|chunk| all_chunks.contains(chunk))
        .collect();

    // Queue every affected chunk's (padded) heightmap for upload into its texture array layer.
    if !chunks_to_upload.is_empty() {
        let _span =
            debug_span!("update_render_chunks_local_upload", selected_node = ?selected_node)
                .entered();
        let writes: Vec<(u32, Vec<f32>)> = chunks_to_upload
            .par_iter()
            .map(|chunk| {
                let padded = padded_heightmap(*chunk, tile_size, &terrain_preview.chunks);
                (chunk.1 as u32 * chunks_x + chunk.0 as u32, padded)
            })
            .collect();
        for (layer, padded) in writes {
            queue_layer_write(terrain_preview, layer, padded);
        }
    }

    // Drop any chunks that are no longer in the grid
    terrain_preview
        .chunks
        .retain(|chunk, _| all_chunks.contains(chunk));
    terrain_preview
        .chunk_uuids
        .retain(|chunk, _| all_chunks.contains(chunk));

    // Nothing changed and the mesh/array already exist: leave the GPU-facing state untouched.
    if changed_chunks.is_empty() && terrain_preview.has_mesh() {
        return;
    }

    // Build every chunk's instance descriptor (deterministic: layer = grid-row-major index).
    let instances: Vec<ChunkInstance> = chunk_grid
        .coords()
        .map(|chunk| {
            let offset = chunk_origin(chunk, &chunk_grid);
            ChunkInstance {
                world_offset: [offset.x, offset.z],
                cell_size: chunk_grid.world_scale(),
                layer: chunk.1 as u32 * chunks_x + chunk.0 as u32
            }
        })
        .collect();

    // Publish the current mesh/array/instance state for the render world to pick up.
    sync_preview_state(
        asset_server,
        terrain_preview,
        terrain_preview_sync,
        material_handle,
        tile_size,
        instances,
        |chunk| chunk.1 as u32 * chunks_x + chunk.0 as u32
    );
}
