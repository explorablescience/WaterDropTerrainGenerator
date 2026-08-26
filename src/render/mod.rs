use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use wde::prelude::*;

use crate::{
    TerrainGraphHolder,
    core::{
        chunk_grid::{ChunkCoord, ChunkGrid},
        node_error::NodeError::InputNotConnected,
        tile_allocator::crop_center
    },
    render::{
        mesh_generation::heightmap_to_mesh, terrain_preview_pipeline::TerrainPreviewRenderPipeline,
        terrain_preview_subpass::{SubRenderPassTerrainPreview, TerrainPreviewMeshes}
    }
};

mod mesh_generation;
mod terrain_preview_pipeline;
mod terrain_preview_subpass;

/// World-space distance between adjacent heightmap samples.
const CELL_SIZE: f32 = 0.1;
/// World-space height gained per unit of heightmap value.
const HEIGHT_SCALE: f32 = 1.0;

pub struct RenderPlugin;
impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TerrainPreview>()
            .add_systems(Startup, create_material)
            .add_systems(Update, update_terrain_preview);

        // Terrain-preview chunk meshes subpass
        app.add_plugins(RenderPipelineRegisterPlugin::<TerrainPreviewRenderPipeline>::default());

        // Extract the terrain-preview meshes from the main world into the render world every frame
        app.init_resource::<TerrainPreviewMeshes>();
        app.get_sub_app_mut(RenderApp)
            .unwrap()
            .init_resource::<TerrainPreviewMeshes>()
            .add_systems(Extract, SubRenderPassTerrainPreview::extract);

        app.get_sub_app_mut(RenderApp)
            .unwrap()
            .world_mut()
            .get_resource_mut::<RenderGraph>()
            .unwrap()
            .add_sub_pass::<SubRenderPassTerrainPreview, RenderPassDeferredGBuffer>();
    }
}

/// Render state for one chunk
struct ChunkPreview {
    core_data: Vec<f32>,
    /// Whether `core_data` is the flat fallback shown when this chunk's node can't be evaluated
    is_flat: bool,
    mesh_handle: Option<Handle<Mesh>>
}

#[derive(Resource, Default)]
pub struct TerrainPreview {
    chunks: HashMap<ChunkCoord, ChunkPreview>,
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
    terrain_graph: Res<TerrainGraphHolder>
) {
    // Get selected node from terrain graph
    let selected_node = match terrain_graph.read().selected_node {
        Some(node_id) => node_id,
        None => return // No node selected
    };

    let chunk_grid = *terrain_graph.read().graph().chunk_grid();
    let tile_size = chunk_grid.tile_size();
    let force = terrain_graph.write().note_selection(selected_node);

    let material_handle = match &terrain_preview.material_handle {
        Some(handle) => handle.clone(),
        None => return // Material not created yet
    };

    // Pass 1
    let mut live_chunks = HashSet::new();
    let mut changed_chunks = HashSet::new();
    for chunk in chunk_grid.coords() {
        live_chunks.insert(chunk);

        match terrain_graph.write().process_chunk(selected_node, chunk, force) {
            Ok(Some((_, tiles))) => {
                let Some(heightmap) = tiles.first() else { continue };
                let internal_size = heightmap.size();
                let data = crop_center(heightmap, internal_size, tile_size);
                set_chunk_data(&mut terrain_preview, chunk, data, false);
                changed_chunks.insert(chunk);
            }
            Ok(None) => {} // No new output tiles for this chunk
            Err(e) => {
                match e {
                    InputNotConnected { node, socket, .. } => {
                        trace!(
                            "Cannot generate terrain preview for chunk {:?}: Input not connected for node '{}' at socket {}",
                            chunk, node, socket
                        );
                    }
                    _ => {
                        error!("Error while processing terrain graph for chunk {:?}: {:?}", chunk, e);
                    }
                }
                // The selected node can't be evaluated: show a flat chunk instead of leaving
                // whatever was last rendered on screen.
                let already_flat = terrain_preview.chunks.get(&chunk).is_some_and(|c| c.is_flat);
                if !already_flat {
                    set_chunk_data(&mut terrain_preview, chunk, vec![0.0; tile_size * tile_size], true);
                    changed_chunks.insert(chunk);
                }
            }
        }
    }

    // Pass 2
    for chunk in &changed_chunks {
        let padded = padded_heightmap(*chunk, tile_size, &terrain_preview.chunks);
        let world_offset = chunk_origin(*chunk, &chunk_grid);
        let mesh = heightmap_to_mesh(&format!("terrain-preview-{}-{}", chunk.0, chunk.1), &padded, tile_size, world_offset);
        upsert_chunk_mesh(&mut meshes, &mut terrain_preview, *chunk, mesh);
    }

    // Drop any chunks that are no longer in the grid
    terrain_preview.chunks.retain(|chunk, _| live_chunks.contains(chunk));

    // Publish this frame's chunk meshes to our terrain-preview render subpass.
    terrain_preview_meshes.meshes = terrain_preview
        .chunks
        .values()
        .filter_map(|c| c.mesh_handle.clone())
        .collect();
    terrain_preview_meshes.material = Some(material_handle);
}

/// Stores `data` as `chunk`'s current core heightmap, creating its preview entry if this is the
/// first time `chunk` has been seen.
fn set_chunk_data(terrain_preview: &mut TerrainPreview, chunk: ChunkCoord, data: Vec<f32>, is_flat: bool) {
    match terrain_preview.chunks.get_mut(&chunk) {
        Some(preview) => {
            preview.core_data = data;
            preview.is_flat = is_flat;
        }
        None => {
            terrain_preview
                .chunks
                .insert(chunk, ChunkPreview { core_data: data, is_flat, mesh_handle: None });
        }
    }
}

/// World-space position of `chunk`'s local `(0, 0)` texel
fn chunk_origin(chunk: ChunkCoord, grid: &ChunkGrid) -> Vec3 {
    let step = grid.tile_size() as f32 * CELL_SIZE;
    let extent_x = grid.chunks_x() as f32 * step;
    let extent_y = grid.chunks_y() as f32 * step;
    Vec3::new(chunk.0 as f32 * step - extent_x * 0.5, 0.0, chunk.1 as f32 * step - extent_y * 0.5)
}

/// Builds `chunk`'s `(tile_size + 3) x (tile_size + 3)` heightmap for [`heightmap_to_mesh`]
fn padded_heightmap(chunk: ChunkCoord, tile_size: usize, chunks: &HashMap<ChunkCoord, ChunkPreview>) -> Vec<f32> {
    let padded = tile_size + 3;
    let mut out = vec![0.0; padded * padded];
    for pz in 0..padded {
        for px in 0..padded {
            let lx = px as isize - 1;
            let lz = pz as isize - 1;
            out[pz * padded + px] = sample_across_chunks(chunk, tile_size, chunks, lx, lz);
        }
    }
    out
}

/// Samples core-tile texel `(lx, lz)` relative to `chunk`'s own origin
fn sample_across_chunks(
    chunk: ChunkCoord,
    tile_size: usize,
    chunks: &HashMap<ChunkCoord, ChunkPreview>,
    lx: isize,
    lz: isize
) -> f32 {
    let size = tile_size as isize;
    let (dx, sx) = locate(lx, size);
    let (dz, sz) = locate(lz, size);
    let neighbor = ChunkCoord(chunk.0 + dx, chunk.1 + dz);
    if let Some(preview) = chunks.get(&neighbor) {
        return preview.core_data[sz as usize * tile_size + sx as usize];
    }
    // No such chunk (grid edge) or it hasn't produced data yet: fall back to clamping within
    // this chunk's own tile, same as an unchunked tile always did at its own edge.
    let Some(own) = chunks.get(&chunk) else { return 0.0 };
    let csx = lx.clamp(0, size - 1) as usize;
    let csz = lz.clamp(0, size - 1) as usize;
    own.core_data[csz * tile_size + csx]
}

/// Splits a core-tile-relative coordinate into which neighboring chunk to sample from and which texel within that chunk's tile to read.
fn locate(l: isize, size: isize) -> (i32, isize) {
    (l.div_euclid(size) as i32, l.rem_euclid(size))
}

/// Reuses the existing mesh asset for `chunk` if present, otherwise creates a new one.
fn upsert_chunk_mesh(meshes: &mut Assets<Mesh>, terrain_preview: &mut TerrainPreview, chunk: ChunkCoord, mesh: Mesh) {
    let existing = terrain_preview.chunks.get(&chunk).and_then(|c| c.mesh_handle.clone());
    match existing {
        Some(handle) => {
            if let Err(e) = meshes.insert(handle.id(), mesh) {
                error!("Failed to update terrain preview mesh for chunk {:?}: {:?}", chunk, e);
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
