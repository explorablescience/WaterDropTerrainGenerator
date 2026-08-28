use std::collections::HashMap;

use bevy::prelude::*;
use wde::prelude::*;

use crate::{
    TerrainSessionHolder,
    core::{node::NodeLocality, session::ChunkJobs, tiling::ChunkCoord},
    render::{
        terrain_preview_global::update_global_preview,
        terrain_preview_local::update_local_preview,
        terrain_preview_subpass::TerrainPreviewMeshes
    }
};

pub(super) struct ChunkPreview {
    pub(super) core_data: Vec<f32>,
    /// Whether `core_data` is the flat fallback shown when this chunk's node can't be evaluated.
    pub(super) is_flat: bool,
    pub(super) mesh_handle: Option<Handle<Mesh>>
}

#[derive(Resource, Default)]
pub struct TerrainPreview {
    pub(super) chunks: HashMap<ChunkCoord, ChunkPreview>,
    material_handle: Option<Handle<PbrMaterial>>
}

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

pub(crate) fn update_terrain_preview(
    mut meshes: ResMut<Assets<Mesh>>,
    mut terrain_preview: ResMut<TerrainPreview>,
    mut terrain_preview_meshes: ResMut<TerrainPreviewMeshes>,
    mut chunk_jobs: ResMut<ChunkJobs>,
    terrain_graph: Res<TerrainSessionHolder>
) {
    let selected_node = match terrain_graph.read().selected_node {
        Some(node_id) => node_id,
        None => return
    };
    let locality = match terrain_graph.read().graph().node(selected_node) {
        Ok(node) => node.locality(),
        Err(_) => return // Selected node no longer exists (e.g. it was just removed)
    };

    if terrain_graph.read().graph().is_cooling_down() {
        // Debounce: a param still changing every frame (e.g. a slider being dragged) keeps
        // re-arming this, so reprocessing only fires once edits stop landing for a moment.
        return;
    }

    let force = terrain_graph.write().note_selection(selected_node);
    if force {
        // A previous selection's jobs are no longer relevant - drop them so a late result
        // computed for that node is never mistaken for this one's (they'd collide on the same
        // `ChunkCoord` keys).
        chunk_jobs.clear();
    }

    let material_handle = match &terrain_preview.material_handle {
        Some(handle) => handle.clone(),
        None => return // Material not created yet
    };

    match locality {
        NodeLocality::Global { native_resolution } => update_global_preview(
            &mut meshes,
            &mut terrain_preview,
            &mut terrain_preview_meshes,
            &terrain_graph,
            material_handle,
            selected_node,
            native_resolution,
            force
        ),
        NodeLocality::Local => update_local_preview(
            &mut meshes,
            &mut terrain_preview,
            &mut terrain_preview_meshes,
            &mut chunk_jobs,
            &terrain_graph,
            material_handle,
            selected_node,
            force
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
                    is_flat,
                    mesh_handle: None
                }
            );
        }
    }
}

/// Reuses the existing mesh asset for `chunk` if present, otherwise creates a new one.
pub(super) fn upsert_chunk_mesh(
    meshes: &mut Assets<Mesh>,
    terrain_preview: &mut TerrainPreview,
    chunk: ChunkCoord,
    mesh: Mesh
) {
    let existing = terrain_preview
        .chunks
        .get(&chunk)
        .and_then(|c| c.mesh_handle.clone());
    match existing {
        Some(handle) => {
            if let Err(e) = meshes.insert(handle.id(), mesh) {
                error!(
                    "Failed to update terrain preview mesh for chunk {:?}: {:?}",
                    chunk, e
                );
            }
        }
        None => {
            let handle = meshes.add(mesh);
            if let Some(preview) = terrain_preview.chunks.get_mut(&chunk) {
                preview.mesh_handle = Some(handle);
            }
        }
    }
}

/// Publishes every chunk's current mesh into the render-world-facing resource so [`SubRenderPassTerrainPreview`](super::terrain_preview_subpass::SubRenderPassTerrainPreview) draws them next extract.
pub(super) fn publish_preview_meshes(
    terrain_preview: &TerrainPreview,
    terrain_preview_meshes: &mut TerrainPreviewMeshes,
    material_handle: Handle<PbrMaterial>
) {
    terrain_preview_meshes.meshes = terrain_preview
        .chunks
        .values()
        .filter_map(|c| c.mesh_handle.clone())
        .collect();
    terrain_preview_meshes.material = Some(material_handle);
}
