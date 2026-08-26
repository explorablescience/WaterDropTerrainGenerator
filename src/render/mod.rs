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
    render::mesh_generation::heightmap_to_mesh
};

mod mesh_generation;

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
    }
}

/// The mesh entity backing one chunk's preview, once it's been spawned.
#[derive(Clone)]
struct ChunkRender {
    entity: Entity,
    mesh_handle: Handle<Mesh>
}

/// Render state for one chunk: its latest core (non-halo) heightmap data, kept around so a
/// neighboring chunk can borrow it to compute seamless edge normals even on a frame where this
/// chunk itself didn't change, plus the mesh entity once one exists.
struct ChunkPreview {
    core_data: Vec<f32>,
    /// Whether `core_data` is the flat fallback shown when this chunk's node can't be evaluated,
    /// rather than real terrain data.
    is_flat: bool,
    render: Option<ChunkRender>
}

#[derive(Resource, Default)]
pub struct TerrainPreview {
    chunks: HashMap<ChunkCoord, ChunkPreview>,
    material_handle: Option<Handle<PbrMaterial>>
}

pub fn create_material(
    asset_server: Res<AssetServer>,
    mut terrain_preview: ResMut<TerrainPreview>
) {
    terrain_preview.material_handle = Some(asset_server.add(PbrMaterial {
        label: "terrain-white".to_string(),
        albedo: (1.0, 1.0, 1.0, 0.0),
        ..default()
    }));
}

pub fn update_terrain_preview(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut terrain_preview: ResMut<TerrainPreview>,
    terrain_graph: Res<TerrainGraphHolder>
) {
    // Get selected node from terrain graph
    let selected_node = match terrain_graph.read().selected_node {
        Some(node_id) => node_id,
        None => return // No node selected
    };

    let chunk_grid = *terrain_graph.read().graph().chunk_grid();
    let tile_size = chunk_grid.tile_size();
    // A newly selected node forces every one of its chunks to redraw once, even a chunk whose
    // cached generation hasn't advanced (e.g. reselecting a node computed before the selection
    // moved away from it).
    let force = terrain_graph.write().note_selection(selected_node);

    let material_handle = match &terrain_preview.material_handle {
        Some(handle) => handle.clone(),
        None => return // Material not created yet
    };

    // Pass 1: fetch each chunk's core heightmap data. Kept separate from meshing (pass 2) so a
    // chunk whose data just changed can read its neighbors' *current* data for normal estimation,
    // regardless of which order chunks are visited in.
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

    // Pass 2: (re)build the mesh for every chunk whose data changed this frame.
    for chunk in &changed_chunks {
        let padded = padded_heightmap(*chunk, tile_size, &terrain_preview.chunks);
        let mesh = heightmap_to_mesh(&format!("terrain-preview-{}-{}", chunk.0, chunk.1), &padded, tile_size);
        let translation = chunk_translation(*chunk, &chunk_grid);
        upsert_chunk_mesh(&mut commands, &mut meshes, &mut terrain_preview, &material_handle, *chunk, mesh, translation);
    }

    // Chunks that used to exist (e.g. before the chunk grid shrank) but aren't part of the grid
    // anymore: their preview entities would otherwise be left floating in the scene forever.
    terrain_preview.chunks.retain(|chunk, preview| {
        let keep = live_chunks.contains(chunk);
        if !keep && let Some(render) = &preview.render {
            commands.entity(render.entity).despawn();
        }
        keep
    });
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
                .insert(chunk, ChunkPreview { core_data: data, is_flat, render: None });
        }
    }
}

/// World-space position of `chunk`'s preview mesh, keeping the whole grid roughly centered on the
/// origin regardless of how many chunks it has - the degenerate single-chunk grid sits exactly at
/// the origin, matching the pre-chunking behavior.
fn chunk_translation(chunk: ChunkCoord, grid: &ChunkGrid) -> Vec3 {
    let step = grid.tile_size() as f32 * CELL_SIZE;
    let half_x = (grid.chunks_x() as f32 - 1.0) * step * 0.5;
    let half_y = (grid.chunks_y() as f32 - 1.0) * step * 0.5;
    Vec3::new(chunk.0 as f32 * step - half_x, 0.0, chunk.1 as f32 * step - half_y)
}

/// Builds `chunk`'s `(tile_size + 2) x (tile_size + 2)` heightmap for [`heightmap_to_mesh`]: its
/// own `tile_size x tile_size` core data, surrounded by a 1-texel halo sampled from the
/// corresponding edge of each neighboring chunk. Where a neighbor doesn't exist (the edge of the
/// whole grid) or hasn't been seen yet, the halo instead clamps to `chunk`'s own edge - the same
/// behavior a lone, unchunked tile always had.
fn padded_heightmap(chunk: ChunkCoord, tile_size: usize, chunks: &HashMap<ChunkCoord, ChunkPreview>) -> Vec<f32> {
    let padded = tile_size + 2;
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

/// Samples core-tile texel `(lx, lz)` relative to `chunk`'s own origin - `lx`/`lz` may run one
/// texel past `chunk`'s own `[0, tile_size)` range, in which case the corresponding neighboring
/// chunk is sampled instead (see [`padded_heightmap`]).
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
    let csx = sx.clamp(0, size - 1) as usize;
    let csz = sz.clamp(0, size - 1) as usize;
    own.core_data[csz * tile_size + csx]
}

/// Splits a core-tile-relative coordinate into which neighboring chunk it falls in (`-1`, `0`, or
/// `1` chunks away) and the corresponding local coordinate within that chunk's own tile.
fn locate(l: isize, size: isize) -> (i32, isize) {
    if l < 0 {
        (-1, size - 1)
    } else if l >= size {
        (1, 0)
    } else {
        (0, l)
    }
}

/// Reuses the existing mesh asset and entity for `chunk` if present, otherwise creates a new one.
fn upsert_chunk_mesh(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    terrain_preview: &mut TerrainPreview,
    material_handle: &Handle<PbrMaterial>,
    chunk: ChunkCoord,
    mesh: Mesh,
    translation: Vec3
) {
    let existing = terrain_preview.chunks.get(&chunk).and_then(|c| c.render.clone());
    match existing {
        Some(render) => {
            if let Err(e) = meshes.insert(render.mesh_handle.id(), mesh) {
                error!("Failed to update terrain preview mesh for chunk {:?}: {:?}", chunk, e);
            }
            commands.entity(render.entity).insert(Transform::from_translation(translation));
        }
        None => {
            let handle = meshes.add(mesh);
            let entity = commands
                .spawn((
                    Name::new(format!("Terrain Preview Chunk ({}, {})", chunk.0, chunk.1)),
                    Transform::from_translation(translation),
                    Mesh3d(handle.clone()),
                    PbrMaterial3d(material_handle.clone()),
                    CastShadow
                ))
                .id();
            if let Some(preview) = terrain_preview.chunks.get_mut(&chunk) {
                preview.render = Some(ChunkRender { entity, mesh_handle: handle });
            }
        }
    }
}
