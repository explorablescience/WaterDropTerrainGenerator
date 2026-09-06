use std::collections::HashSet;

use bevy::prelude::*;
use rayon::prelude::*;
use wde::prelude::*;

use crate::{
    TerrainSessionHolder,
    core::{
        graph::GraphNodeId,
        node::NodeError::InputNotConnected,
        parallelism::{ChunkJobs, GlobalPassJobs},
        tiling::{ChunkCoord, crop_padding}
    },
    render::{
        chunk_array::{ChunkInstance, TerrainPreviewSync},
        generate_chunks::{TerrainPreview, queue_layer_write, set_chunk_data, sync_preview_state},
        utils::{chunk_origin, padded_heightmap}
    }
};

/// Renders `selected_node`'s output tiled across every chunk of the terrain's [`ChunkGrid`](crate::core::tiling::ChunkGrid).
/// Each chunk is evaluated on the compute task pool (see [`ChunkJobs`]), polled a frame at a time,
/// but a dirtied batch is only swapped into the preview once every chunk in it is ready.
#[allow(clippy::too_many_arguments)]
pub(super) fn update_render_chunks_local(
    asset_server: &AssetServer,
    terrain_preview: &mut TerrainPreview,
    terrain_preview_sync: &mut TerrainPreviewSync,
    chunk_jobs: &mut ChunkJobs,
    global_jobs: &mut GlobalPassJobs,
    terrain_graph: &TerrainSessionHolder,
    material_handle: Handle<PbrMaterial>,
    selected_node: GraphNodeId
) {
    let _span = debug_span!("update_render_chunks_local", selected_node = ?selected_node).entered();
    let chunk_grid = *terrain_graph.read().graph().chunk_grid();
    let tile_size = chunk_grid.tile_size();
    let chunks_x = chunk_grid.chunks_x();

    // Resize the pool if needed - cheap bookkeeping, safe to do inline
    {
        let needs_resize = terrain_graph
            .read()
            .graph()
            .needs_pool_resize(selected_node);
        match needs_resize {
            Ok(true) => {
                if let Err(e) = terrain_graph
                    .write()
                    .graph_mut()
                    .resize_pool_for(selected_node)
                {
                    match e {
                        InputNotConnected { .. } => {} // Fine
                        _ => error!("Failed to resize tile pool for evaluation: {:?}", e)
                    }
                    return;
                }
            }
            Ok(false) => {}
            Err(e) => {
                error!("Failed to check tile pool size: {:?}", e);
                return;
            }
        }
    }

    // Barrier on any required `Global` ancestor's whole-terrain pass, computed as its own
    // background task rather than blocking this frame. Chunk fan-out below only proceeds once
    // every such ancestor is cached.
    {
        let _span =
            debug_span!("update_render_chunks_local_global_ancestors", selected_node = ?selected_node)
                .entered();
        let pending_ancestors = terrain_graph
            .read()
            .graph()
            .pending_global_ancestors(selected_node);
        match pending_ancestors {
            Ok(ancestors) => {
                for ancestor in ancestors {
                    let Some(result) = global_jobs.poll_or_spawn(terrain_graph, ancestor) else {
                        return; // still computing, or was just spawned
                    };
                    if let Err(e) = result {
                        match e {
                            InputNotConnected { .. } => {} // Fine
                            _ => error!("Failed to compute global ancestor pass: {:?}", e)
                        }
                        return;
                    }
                }
            }
            Err(e) => {
                error!("Failed to check pending global ancestors: {:?}", e);
                return;
            }
        }
    }

    // Evaluate every chunk, buffering finished ones in `chunk_jobs` until the whole batch is ready.
    let mut all_chunks = HashSet::new();
    let mut batch_ready = true;
    {
        let _span =
            debug_span!("update_render_chunks_local_process", selected_node = ?selected_node)
                .entered();

        for chunk in chunk_grid.coords() {
            all_chunks.insert(chunk);

            // Poll or spawn a background job for this chunk (if terrain changed, they were cleared in `generate_chunks`)
            let Some(result) = chunk_jobs.poll_or_spawn(terrain_graph, selected_node, chunk) else {
                batch_ready = false;
                continue; // job still running, or was just spawned
            };
            chunk_jobs.stage_result(chunk, result);
        }
    }

    // Apply the batch only once it's fully landed.
    let mut changed_chunks = HashSet::new();
    if batch_ready {
        let _span =
            debug_span!("update_render_chunks_local_apply", selected_node = ?selected_node)
                .entered();

        for (chunk, result) in chunk_jobs.take_staged() {
            match terrain_graph
                .write()
                .apply_chunk_result(selected_node, chunk, result)
            {
                Ok(Some((_, tiles))) => {
                    // If the chunk was recomputed, update its data and mark it as changed
                    let Some(heightmap) = tiles.first() else {
                        continue;
                    };
                    let internal_size = heightmap.size();
                    let data = crop_padding(heightmap, internal_size, tile_size);
                    set_chunk_data(terrain_preview, chunk, data, false);
                    changed_chunks.insert(chunk);
                }
                Ok(None) => {} // unchanged
                Err(e) => {
                    match e {
                        InputNotConnected { .. } => {} // Fine, just means the user hasn't connected the selected node's input yet
                        _ => error!(
                            "Error while processing terrain graph for chunk {:?}: {:?}",
                            chunk, e
                        )
                    }

                    // Show a flat chunk instead
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
            }
        }
    }

    // A changed chunk's neighbors also need re-stitching/re-upload - their border texels sample it.
    let chunks_to_upload: HashSet<_> = changed_chunks
        .iter()
        .flat_map(|chunk| {
            (-1..=1).flat_map(move |dz| {
                (-1..=1).map(move |dx| ChunkCoord(chunk.0 + dx, chunk.1 + dz))
            })
        })
        .filter(|chunk| all_chunks.contains(chunk))
        .collect();

    // Queue every affected chunk's (padded) heightmap for upload into its texture array layer.
    {
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

    // Drop any chunks (and their in-flight jobs) that are no longer in the grid
    terrain_preview
        .chunks
        .retain(|chunk, _| all_chunks.contains(chunk));
    chunk_jobs.retain_live(&all_chunks);

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
