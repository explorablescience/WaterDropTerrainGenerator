use std::collections::HashMap;

use bevy::prelude::*;
use wde::prelude::*;

use crate::{
    TerrainSessionHolder,
    core::{
        graph::GraphNodeId,
        node::NodeLocality,
        parallelism::{ChunkJobs, GlobalPassJobs},
        tiling::ChunkCoord
    },
    render::{
        chunk_array::{ChunkInstance, TerrainPreviewSync},
        generate_chunks_global::update_render_chunks_global,
        generate_chunks_local::update_render_chunks_local,
        utils::{build_shared_chunk_mesh, padded_heightmap}
    }
};

pub(super) struct ChunkPreview {
    pub(super) core_data: Vec<f32>,
    /// Whether `core_data` is the flat fallback shown when this chunk's node can't be evaluated.
    pub(super) is_flat: bool
}

#[derive(Resource, Default)]
pub struct TerrainPreview {
    pub(super) chunks: HashMap<ChunkCoord, ChunkPreview>,
    material_handle: Option<Handle<PbrMaterial>>,

    mesh: Option<Handle<Mesh>>,
    mesh_size: Option<usize>,
    heightmap_array: Option<Handle<Texture>>,
    /// `(padded texel size, layer count)` the current `heightmap_array` was built with.
    array_dims: Option<(u32, u32)>,
    pending_layer_writes: Vec<(u32, Vec<f32>)>
}

/// Creates the material used for rendering the terrain preview meshes.
pub(crate) fn create_material(
    asset_server: Res<AssetServer>,
    mut terrain_preview: ResMut<TerrainPreview>
) {
    terrain_preview.material_handle = Some(asset_server.add(PbrMaterial {
        label: "terrain-white".to_string(),
        albedo: (1.0, 1.0, 1.0, 0.0),
        ..default()
    }));
}

/// Updates the terrain preview meshes based on the currently selected node in the terrain graph.
#[allow(clippy::too_many_arguments)]
pub(crate) fn update_render_chunks(
    asset_server: Res<AssetServer>,
    mut terrain_preview: ResMut<TerrainPreview>,
    mut terrain_preview_sync: ResMut<TerrainPreviewSync>,
    mut chunk_jobs: ResMut<ChunkJobs>,
    mut global_jobs: ResMut<GlobalPassJobs>,
    terrain_graph: Res<TerrainSessionHolder>,
    mut old_selected_node: Local<Option<GraphNodeId>>,
    mut old_selected_node_last_dirty: Local<Option<std::time::Instant>>
) {
    let selected_node = match terrain_graph.read().selected_node {
        Some(node_id) => node_id,
        None => return
    };
    if old_selected_node_last_dirty.is_none() {
        *old_selected_node_last_dirty = Some(std::time::Instant::now());
    }

    // Clear all previous chunk jobs if the selected node has changed or is dirty
    let reprocess = {
        let is_new_node = old_selected_node.is_some_and(|id| id != selected_node);
        let is_dirty = terrain_graph
            .read()
            .graph()
            .is_or_ancestor_dirty(selected_node);
        if is_new_node {
            *old_selected_node_last_dirty = Some(std::time::Instant::now());
        }
        let time_elapsed = old_selected_node_last_dirty
            .as_ref()
            .map(|t| t.elapsed())
            .unwrap_or_default();
        if is_dirty {
            old_selected_node_last_dirty.replace(std::time::Instant::now());
        }
        old_selected_node.replace(selected_node);
        is_new_node || (is_dirty && time_elapsed.as_millis() > 20)
    };
    if reprocess {
        chunk_jobs.clear();
        global_jobs.clear();
    }

    // Don't reprocess the graph if the cooldown hasn't elapsed yet
    if !reprocess && terrain_graph.read().graph().should_reprocess_cooldown() {
        return;
    }

    // Call the appropriate update function based on the selected node's locality
    let material_handle = match &terrain_preview.material_handle {
        Some(handle) => handle.clone(),
        None => return // Material not created yet
    };
    let node_locality = match terrain_graph.read().graph().node(selected_node) {
        Ok(node) => node.locality(),
        Err(_) => return // Selected node no longer exists
    };
    match node_locality {
        NodeLocality::Global { native_resolution } => update_render_chunks_global(
            &asset_server,
            &mut terrain_preview,
            &mut terrain_preview_sync,
            &terrain_graph,
            material_handle,
            selected_node,
            native_resolution
        ),
        NodeLocality::Local => update_render_chunks_local(
            &asset_server,
            &mut terrain_preview,
            &mut terrain_preview_sync,
            &mut chunk_jobs,
            &mut global_jobs,
            &terrain_graph,
            material_handle,
            selected_node
        )
    }
}

/// Creates `chunk`'s preview entry if this is the first time it's been seen.
pub(super) fn set_chunk_data(
    terrain_preview: &mut TerrainPreview,
    chunk: ChunkCoord,
    data: Vec<f32>,
    is_flat: bool
) {
    match terrain_preview.chunks.get_mut(&chunk) {
        Some(preview) => {
            preview.core_data = data;
            preview.is_flat = is_flat;
        }
        None => {
            terrain_preview.chunks.insert(
                chunk,
                ChunkPreview {
                    core_data: data,
                    is_flat
                }
            );
        }
    }
}

/// Queues `chunk`'s (already padded) heightmap for upload into `layer` of the heightmap texture
/// array. Actually uploaded next render frame by [`crate::render::chunk_array::sync_terrain_preview_gpu`].
pub(super) fn queue_layer_write(terrain_preview: &mut TerrainPreview, layer: u32, data: Vec<f32>) {
    terrain_preview.pending_layer_writes.push((layer, data));
}

/// Ensures the shared chunk mesh and heightmap texture array match `size`/`instances.len()`,
/// recreating them (and re-queuing every known chunk's data) when they don't, then publishes the
/// current state into `terrain_preview_sync` for the render world to pick up.
pub(super) fn sync_preview_state(
    asset_server: &AssetServer,
    terrain_preview: &mut TerrainPreview,
    terrain_preview_sync: &mut TerrainPreviewSync,
    material_handle: Handle<PbrMaterial>,
    size: usize,
    instances: Vec<ChunkInstance>,
    layer_of: impl Fn(ChunkCoord) -> u32
) {
    let _span = debug_span!(
        "sync_preview_state",
        size = size,
        instances = instances.len()
    )
    .entered();

    // Rebuild the shared mesh when the vertex density (tile_size / native_resolution) changes.
    if terrain_preview.mesh_size != Some(size) {
        terrain_preview.mesh = Some(asset_server.add(build_shared_chunk_mesh(size)));
        terrain_preview.mesh_size = Some(size);
    }

    // Recreate the heightmap texture array when its per-layer size or layer count changes. A
    // fresh texture starts uninitialized, so every currently-known chunk must be re-uploaded.
    // Layer count is clamped to at least 2: the engine creates a plain D2 (non-array) view for a
    // 1-layer texture, which wouldn't match this pipeline's D2Array-typed bind group layout (the
    // global-locality preview only ever has one chunk/instance).
    let padded_size = (size + 3) as u32;
    let layer_count = instances.len().max(2) as u32;
    if terrain_preview.array_dims != Some((padded_size, layer_count)) {
        terrain_preview.heightmap_array = Some(asset_server.add(Texture {
            label: "terrain-preview-heightmap-array".to_string(),
            size: (padded_size, padded_size),
            format: TextureFormat::R32Float,
            usages: TextureUsages::TEXTURE_BINDING
                | TextureUsages::STORAGE_BINDING
                | TextureUsages::COPY_DST
                | TextureUsages::COPY_SRC,
            layer_count,
            filterable: false,
            ..Default::default()
        }));
        terrain_preview.array_dims = Some((padded_size, layer_count));

        for &chunk in terrain_preview.chunks.keys() {
            let padded = padded_heightmap(chunk, size, &terrain_preview.chunks);
            terrain_preview
                .pending_layer_writes
                .push((layer_of(chunk), padded));
        }
    }

    terrain_preview_sync.heightmap_array = terrain_preview.heightmap_array.clone();
    terrain_preview_sync.mesh = terrain_preview.mesh.clone();
    terrain_preview_sync.material = Some(material_handle);
    terrain_preview_sync.instances = instances;
    terrain_preview_sync.pending_writes = std::mem::take(&mut terrain_preview.pending_layer_writes);
}
