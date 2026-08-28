use std::collections::HashSet;

use bevy::prelude::*;
use wde::prelude::*;

use crate::{
    TerrainSessionHolder,
    core::{
        graph::GraphNodeId, node::NodeError::InputNotConnected, parallelism::ChunkJobs,
        tiling::crop_padding,
    },
    render::{
        generate_chunks::{TerrainPreview, set_chunk_data, set_preview_meshes, upsert_chunk_mesh},
        render_subpass::TerrainPreviewMeshes,
        utils::{chunk_origin, heightmap_to_mesh, padded_heightmap},
    },
};

/// Renders `selected_node`'s output tiled across every chunk of the terrain's [`ChunkGrid`](crate::core::tiling::ChunkGrid).
/// Each chunk is evaluated on the compute task pool (see [`ChunkJobs`]) rather than inline, so a
/// frame only picks up whichever chunks have finished since the last one.
#[allow(clippy::too_many_arguments)]
pub(super) fn update_render_chunks_local(
    meshes: &mut Assets<Mesh>,
    terrain_preview: &mut TerrainPreview,
    terrain_preview_meshes: &mut TerrainPreviewMeshes,
    chunk_jobs: &mut ChunkJobs,
    terrain_graph: &TerrainSessionHolder,
    material_handle: Handle<PbrMaterial>,
    selected_node: GraphNodeId,
    force: bool,
) {
    let _span = debug_span!("update_render_chunks_local", selected_node = ?selected_node).entered();
    let chunk_grid = *terrain_graph.read().graph().chunk_grid();
    let tile_size = chunk_grid.tile_size();

    // Prepare the graph for parallel evaluation if needed
    {
        let _span =
            debug_span!("update_render_chunks_local_prepare", selected_node = ?selected_node)
                .entered();
        match terrain_graph
            .read()
            .graph()
            .needs_parallel_prepare(selected_node)
        {
            Ok(true) => {
                if let Err(e) = terrain_graph
                    .write()
                    .graph_mut()
                    .prepare_for_parallel_eval(selected_node)
                {
                    error!("Failed to prepare terrain graph for evaluation: {:?}", e);
                    return;
                }
            }
            Ok(false) => {}
            Err(e) => {
                error!("Failed to check terrain graph preparation: {:?}", e);
                return;
            }
        }
    }

    // Process every chunk in the grid, using parallel evaluation
    let (all_chunks, changed_chunks) = {
        let _span =
            debug_span!("update_render_chunks_local_process", selected_node = ?selected_node)
                .entered();
        let mut all_chunks = HashSet::new();
        let mut changed_chunks = HashSet::new();

        for chunk in chunk_grid.coords() {
            all_chunks.insert(chunk);

            // Poll or spawn a background job for this chunk (if terrain changed, they were cleared in `generate_chunks`)
            let Some(result) = chunk_jobs.poll_or_spawn(terrain_graph, selected_node, chunk) else {
                continue; // job still running, or was just spawned
            };

            // Apply the result to the graph and update cache
            match terrain_graph
                .write()
                .apply_chunk_result(selected_node, chunk, force, result)
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
                Ok(None) => {}
                Err(e) => {
                    match e {
                        InputNotConnected { .. } => {} // Fine, just means the user hasn't connected the selected node's input yet
                        _ => error!(
                            "Error while processing terrain graph for chunk {:?}: {:?}",
                            chunk, e
                        ),
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
                            true,
                        );
                        changed_chunks.insert(chunk);
                    }
                }
            }
        }
        (all_chunks, changed_chunks)
    };

    // Regenerate the meshes data for every chunk that changed
    {
        let _span = debug_span!("update_render_chunks_local_mesh", selected_node = ?selected_node)
            .entered();
        for chunk in &changed_chunks {
            let padded = padded_heightmap(*chunk, tile_size, &terrain_preview.chunks);
            let world_offset = chunk_origin(*chunk, &chunk_grid);
            let mesh = heightmap_to_mesh(
                &format!("terrain-preview-{}-{}", chunk.0, chunk.1),
                &padded,
                tile_size,
                chunk_grid.world_scale(),
                world_offset,
            );
            upsert_chunk_mesh(meshes, terrain_preview, *chunk, mesh);
        }
    }

    // Drop any chunks (and their in-flight jobs) that are no longer in the grid
    terrain_preview
        .chunks
        .retain(|chunk, _| all_chunks.contains(chunk));
    chunk_jobs.retain_live(&all_chunks);

    // Update the meshes from the newly generated chunk data (or the existing one if it didn't change)
    set_preview_meshes(terrain_preview, terrain_preview_meshes, material_handle);
}
