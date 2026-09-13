use bevy::prelude::*;
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
            MAX_PREVIEW_MESH_SIZE, padded_heightmap, resample_color_for_preview,
            resample_for_preview
        }
    }
};

/// Renders `render_node`'s own whole-terrain result (see `TileContext::for_global`) as one mesh
/// covering the same real world extent the chunked terrain does. A Height output feeds the mesh
/// directly; a Color/Mask output feeds the colormap overlay instead, in which case `mesh_node`
/// (if any) separately supplies the mesh shape.
pub(super) fn update_render_chunks_global(
    asset_server: &AssetServer,
    terrain_preview: &mut TerrainPreview,
    terrain_preview_sync: &mut TerrainPreviewSync,
    terrain_graph: &TerrainInstanceHolder,
    material_handle: Handle<PbrMaterial>,
    target: &RenderTarget,
    native_resolution: usize
) {
    let render_node = target.render_node;
    let mesh_node = target.mesh_node;
    let renders_color = target.renders_color();

    let chunk_grid = *terrain_graph.read().graph().chunk_grid();
    let (world_extent_x, world_extent_y) = chunk_grid.world_extent();
    let chunk = ChunkCoord(0, 0);
    let preview_size = chunk_grid.tile_size().min(MAX_PREVIEW_MESH_SIZE);

    // Drop every other per-chunk entry *before* touching the padding/stitching machinery below
    terrain_preview.chunks.retain(|c, _| *c == chunk);

    let mut height_changed = false;
    let mut color_changed = false;
    match terrain_graph.write().get(render_node) {
        Ok(Some(CacheEntry::Global(uuid, tiles))) => {
            let _span = debug_span!("update_render_chunks_global_apply", render_node = ?render_node, native_resolution = native_resolution).entered();
            if terrain_preview.global_uuid != Some(uuid) {
                terrain_preview.global_uuid = Some(uuid);
                if let Some(tile) = tiles.first() {
                    if renders_color {
                        let color =
                            resample_color_for_preview(tile, native_resolution, preview_size);
                        set_chunk_color_data(terrain_preview, chunk, Some(color));
                        color_changed = true;
                    } else {
                        let data = resample_for_preview(tile, native_resolution, preview_size);
                        set_chunk_data(terrain_preview, chunk, data, false);
                        height_changed = true;
                    }
                }
            }
        }
        Ok(Some(CacheEntry::Local(_))) => {
            warn!(
                "Global-locality node {:?} evaluated to a Local result",
                render_node
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

            if renders_color {
                let already_blank = terrain_preview
                    .chunks
                    .get(&chunk)
                    .is_some_and(|c| c.color_data.is_none());
                if !already_blank {
                    set_chunk_color_data(terrain_preview, chunk, None);
                    color_changed = true;
                }
            } else {
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
                    height_changed = true;
                }
            }
            terrain_preview.global_uuid = None;
        }
    }

    // Height-output rendering: clear any stale color overlay left from a previous Color/Mask
    // selection, so switching back to a Height node reverts to the plain flat-white material.
    if !renders_color {
        let already_blank = terrain_preview
            .chunks
            .get(&chunk)
            .is_some_and(|c| c.color_data.is_none());
        if !already_blank {
            set_chunk_color_data(terrain_preview, chunk, None);
            color_changed = true;
        }
    }

    // Only relevant when the rendered node itself isn't Height: either a separately pinned mesh
    // supplies real height data, or (no pin) the chunk stays a flat placeholder.
    if renders_color {
        if let Some(mesh_node) = mesh_node {
            match terrain_graph.write().get(mesh_node) {
                Ok(Some(CacheEntry::Global(_, tiles))) => {
                    if let Some(tile) = tiles.first() {
                        let data = resample_for_preview(tile, native_resolution, preview_size);
                        set_chunk_data(terrain_preview, chunk, data, false);
                        height_changed = true;
                    }
                }
                Ok(Some(CacheEntry::Local(_))) => {
                    warn!(
                        "Pinned mesh node {:?} evaluated to a Local result",
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
                height_changed = true;
            }
        }
    }

    // If the chunk's height changed, queue its (padded) heightmap for upload into layer 0.
    if height_changed {
        let _span = debug_span!("update_render_chunks_global_upload", chunk = ?chunk).entered();
        let padded = padded_heightmap(chunk, preview_size, &terrain_preview.chunks);
        queue_layer_write(terrain_preview, 0, padded);
    }
    if color_changed {
        let data = terrain_preview
            .chunks
            .get(&chunk)
            .and_then(|c| c.color_data.clone())
            .unwrap_or_else(|| vec![1.0; preview_size * preview_size * 4]);
        queue_color_write(terrain_preview, 0, data);
    }

    // Nothing changed and the mesh/array already exist: leave the GPU-facing state untouched.
    if !height_changed && !color_changed && terrain_preview.has_mesh() {
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
