use std::collections::HashSet;

use bevy::prelude::*;
use rayon::prelude::*;
use wde::prelude::*;

use crate::{
    TerrainInstanceHolder,
    core::{CacheEntry, node::NodeError::InputNotConnected, tiling::ChunkCoord},
    render::{
        chunk_array::{ChunkInstance, TerrainPreviewSync},
        generate_chunks::{
            RenderTarget, TerrainPreview, queue_color_write, queue_layer_write,
            set_chunk_color_data, set_chunk_data, sync_preview_state
        },
        utils::{
            MAX_PREVIEW_MESH_SIZE, chunk_origin, padded_heightmap, resample_color_for_preview,
            resample_for_preview
        }
    }
};

/// Renders `render_node`'s output tiled across every chunk of the terrain's [`ChunkGrid`](crate::core::tiling::ChunkGrid).
/// A Height output feeds the mesh directly; a Color/Mask output feeds the colormap overlay instead,
/// in which case `mesh_node` (if any) separately supplies the mesh shape.
/// `NodeGraph::get` returns every chunk together, once the whole barrier is done - only the chunks
/// whose uuid actually changed (plus their neighbors, whose padding rings sample them) are
/// re-stitched and re-queued for GPU upload.
pub(super) fn update_render_chunks_local(
    asset_server: &AssetServer,
    terrain_preview: &mut TerrainPreview,
    terrain_preview_sync: &mut TerrainPreviewSync,
    terrain_graph: &TerrainInstanceHolder,
    material_handle: Handle<PbrMaterial>,
    target: &RenderTarget
) {
    let render_node = target.render_node;
    let mesh_node = target.mesh_node;
    let renders_color = target.renders_color();

    let _span = debug_span!("update_render_chunks_local", render_node = ?render_node).entered();
    let chunk_grid = *terrain_graph.read().graph().chunk_grid();
    let tile_size = chunk_grid.tile_size();
    let preview_size = tile_size.min(MAX_PREVIEW_MESH_SIZE);
    let chunks_x = chunk_grid.chunks_x();
    let all_chunks: HashSet<ChunkCoord> = chunk_grid.coords().collect();

    let mut changed_height_chunks = HashSet::new();
    let mut changed_color_chunks = HashSet::new();

    // The rendered node's own data: Height feeds the mesh directly; Color/Mask feeds the overlay.
    match terrain_graph.write().get(render_node) {
        Ok(Some(CacheEntry::Local(map))) => {
            let _span = debug_span!("update_render_chunks_local_apply", render_node = ?render_node)
                .entered();
            for (chunk, (uuid, tiles)) in map {
                if terrain_preview.chunk_uuids.get(&chunk) == Some(&uuid) {
                    continue; // unchanged since the last frame we uploaded it
                }
                terrain_preview.chunk_uuids.insert(chunk, uuid);
                let Some(tile) = tiles.first() else {
                    continue;
                };
                if renders_color {
                    let color = resample_color_for_preview(tile, tile_size, preview_size);
                    set_chunk_color_data(terrain_preview, chunk, Some(color));
                    changed_color_chunks.insert(chunk);
                } else {
                    let data = resample_for_preview(tile, tile_size, preview_size);
                    set_chunk_data(terrain_preview, chunk, data, false);
                    changed_height_chunks.insert(chunk);
                }
            }
        }
        Ok(Some(CacheEntry::Global(..))) => {
            warn!(
                "Local-locality node {:?} evaluated to a Global result",
                render_node
            );
            return;
        }
        Ok(None) => return, // still processing
        Err(e) => {
            match e {
                InputNotConnected { .. } => {} // Fine, just means the user hasn't connected the selected node's input yet
                _ => error!(
                    "Error while processing terrain graph for node {:?}: {:?}",
                    render_node, e
                )
            }

            // Show a flat/blank chunk instead of whatever was last known, unless it already is one.
            for &chunk in &all_chunks {
                if renders_color {
                    let already_blank = terrain_preview
                        .chunks
                        .get(&chunk)
                        .is_some_and(|c| c.color_data.is_none());
                    if !already_blank {
                        set_chunk_color_data(terrain_preview, chunk, None);
                        changed_color_chunks.insert(chunk);
                    }
                } else {
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
                        changed_height_chunks.insert(chunk);
                    }
                }
            }
            terrain_preview.chunk_uuids.clear();
        }
    }

    // Height-output rendering: clear any stale color overlay left from a previous Color/Mask
    // selection, so switching back to a Height node reverts to the plain flat-white material.
    if !renders_color {
        for &chunk in &all_chunks {
            let already_blank = terrain_preview
                .chunks
                .get(&chunk)
                .is_some_and(|c| c.color_data.is_none());
            if !already_blank {
                set_chunk_color_data(terrain_preview, chunk, None);
                changed_color_chunks.insert(chunk);
            }
        }
    }

    // Only relevant when the rendered node itself isn't Height: either a separately pinned mesh
    // supplies real height data, or (no pin) every chunk stays a flat placeholder.
    if renders_color {
        if let Some(mesh_node) = mesh_node {
            let _span =
                debug_span!("update_render_chunks_local_mesh", mesh_node = ?mesh_node).entered();
            match terrain_graph.write().get(mesh_node) {
                Ok(Some(CacheEntry::Local(map))) => {
                    for (chunk, (_, tiles)) in map {
                        let Some(tile) = tiles.first() else {
                            continue;
                        };
                        let data = resample_for_preview(tile, tile_size, preview_size);
                        set_chunk_data(terrain_preview, chunk, data, false);
                        changed_height_chunks.insert(chunk);
                    }
                }
                Ok(Some(CacheEntry::Global(..))) => {
                    warn!(
                        "Pinned mesh node {:?} evaluated to a Global result",
                        mesh_node
                    );
                }
                Ok(None) => {} // still processing, keep whatever height was last shown
                Err(e) => match e {
                    InputNotConnected { .. } => {}
                    _ => error!(
                        "Error while processing pinned mesh node {:?}: {:?}",
                        mesh_node, e
                    )
                }
            }
        } else {
            for &chunk in &all_chunks {
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
                    changed_height_chunks.insert(chunk);
                }
            }
        }
    }

    // A changed chunk's neighbors also need re-stitching/re-upload - their border texels sample it.
    let chunks_to_upload: HashSet<_> = changed_height_chunks
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
            debug_span!("update_render_chunks_local_upload", render_node = ?render_node).entered();
        let writes: Vec<(u32, Vec<f32>)> = chunks_to_upload
            .par_iter()
            .map(|chunk| {
                let padded = padded_heightmap(*chunk, preview_size, &terrain_preview.chunks);
                (chunk.1 as u32 * chunks_x + chunk.0 as u32, padded)
            })
            .collect();
        for (layer, padded) in writes {
            queue_layer_write(terrain_preview, layer, padded);
        }
    }

    // Colormap texels need no cross-chunk stitching, so only the exactly-changed chunks matter.
    for &chunk in &changed_color_chunks {
        let data = terrain_preview
            .chunks
            .get(&chunk)
            .and_then(|c| c.color_data.clone())
            .unwrap_or_else(|| vec![1.0; preview_size * preview_size * 4]);
        queue_color_write(
            terrain_preview,
            chunk.1 as u32 * chunks_x + chunk.0 as u32,
            data
        );
    }

    // Drop any chunks that are no longer in the grid
    terrain_preview
        .chunks
        .retain(|chunk, _| all_chunks.contains(chunk));
    terrain_preview
        .chunk_uuids
        .retain(|chunk, _| all_chunks.contains(chunk));

    // Nothing changed and the mesh/array already exist: leave the GPU-facing state untouched.
    if changed_height_chunks.is_empty()
        && changed_color_chunks.is_empty()
        && terrain_preview.has_mesh()
    {
        return;
    }

    // Build every chunk's instance descriptor (deterministic: layer = grid-row-major index).
    let instances: Vec<ChunkInstance> = chunk_grid
        .coords()
        .map(|chunk| {
            let offset = chunk_origin(chunk, &chunk_grid);
            ChunkInstance {
                world_offset: [offset.x, offset.z],
                cell_size: chunk_grid.world_scale() * (tile_size as f32 / preview_size as f32),
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
        preview_size,
        instances,
        |chunk| chunk.1 as u32 * chunks_x + chunk.0 as u32
    );
}
