use std::collections::HashSet;

use bevy::prelude::*;
use wde::prelude::*;

use crate::{
    TerrainSessionHolder,
    core::{graph::GraphNodeId, node::NodeError::InputNotConnected, session::ChunkJobs, tiling::crop_center},
    render::{
        chunk_stitching::{chunk_origin, padded_heightmap},
        mesh_generation::heightmap_to_mesh,
        terrain_preview::{TerrainPreview, publish_preview_meshes, set_chunk_data, upsert_chunk_mesh},
        terrain_preview_subpass::TerrainPreviewMeshes
    }
};

/// Renders `selected_node`'s output tiled across every chunk of the terrain's [`ChunkGrid`](crate::core::tiling::ChunkGrid).
/// Each chunk is evaluated on the compute task pool (see [`ChunkJobs`]) rather than inline, so a
/// frame only picks up whichever chunks have finished since the last one.
#[allow(clippy::too_many_arguments)]
pub(super) fn update_local_preview(
    meshes: &mut Assets<Mesh>,
    terrain_preview: &mut TerrainPreview,
    terrain_preview_meshes: &mut TerrainPreviewMeshes,
    chunk_jobs: &mut ChunkJobs,
    terrain_graph: &TerrainSessionHolder,
    material_handle: Handle<PbrMaterial>,
    selected_node: GraphNodeId,
    force: bool
) {
    let chunk_grid = *terrain_graph.read().graph().chunk_grid();
    let tile_size = chunk_grid.tile_size();

    // Only take a write lock on the rare frame something actually needs preparing.
    match terrain_graph.read().graph().needs_parallel_prepare(selected_node) {
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

    let mut live_chunks = HashSet::new();
    let mut changed_chunks = HashSet::new();
    for chunk in chunk_grid.coords() {
        live_chunks.insert(chunk);

        let Some(result) = chunk_jobs.poll_or_spawn(terrain_graph, selected_node, chunk) else {
            continue; // job still running, or was just spawned
        };

        match terrain_graph
            .write()
            .apply_chunk_result(selected_node, chunk, force, result)
        {
            Ok(Some((_, tiles))) => {
                let Some(heightmap) = tiles.first() else {
                    continue;
                };
                let internal_size = heightmap.size();
                let data = crop_center(heightmap, internal_size, tile_size);
                set_chunk_data(terrain_preview, chunk, data, false);
                changed_chunks.insert(chunk);
            }
            Ok(None) => {}
            Err(e) => {
                match e {
                    InputNotConnected { node, socket, .. } => {
                        trace!(
                            "Cannot generate terrain preview for chunk {:?}: Input not connected for node '{}' at socket {}",
                            chunk, node, socket
                        );
                    }
                    _ => {
                        error!(
                            "Error while processing terrain graph for chunk {:?}: {:?}",
                            chunk, e
                        );
                    }
                }
                // Show a flat chunk instead of leaving whatever was last rendered on screen.
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

    for chunk in &changed_chunks {
        let padded = padded_heightmap(*chunk, tile_size, &terrain_preview.chunks);
        let world_offset = chunk_origin(*chunk, &chunk_grid);
        let mesh = heightmap_to_mesh(
            &format!("terrain-preview-{}-{}", chunk.0, chunk.1),
            &padded,
            tile_size,
            chunk_grid.world_scale(),
            world_offset
        );
        upsert_chunk_mesh(meshes, terrain_preview, *chunk, mesh);
    }

    // Drop any chunks (and their in-flight jobs) that are no longer in the grid
    terrain_preview
        .chunks
        .retain(|chunk, _| live_chunks.contains(chunk));
    chunk_jobs.retain_live(&live_chunks);

    publish_preview_meshes(terrain_preview, terrain_preview_meshes, material_handle);
}
