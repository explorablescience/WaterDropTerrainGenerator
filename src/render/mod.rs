use bevy::prelude::*;
use wde::prelude::*;

use crate::{
    TerrainGraphHolder, core::node_error::NodeError::InputNotConnected,
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

#[derive(Resource, Default)]
pub struct TerrainPreview {
    current_go: Option<Entity>,
    mesh_handle: Option<Handle<Mesh>>,
    material_handle: Option<Handle<PbrMaterial>>,
    /// Show flat fallback terrain instead of the last rendered terrain when the selected node cannot be evaluated. 
    showing_flat_fallback: bool
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

/// Extracts the centered `target_size × target_size` interior out of the raw heightmap data
fn crop_center(data: &[f32], full_size: usize, target_size: usize) -> Vec<f32> {
    if full_size == target_size {
        return data.to_vec();
    }
    let padding = (full_size - target_size) / 2;
    let mut cropped = Vec::with_capacity(target_size * target_size);
    for z in 0..target_size {
        let row_start = (z + padding) * full_size + padding;
        cropped.extend_from_slice(&data[row_start..row_start + target_size]);
    }
    cropped
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

    let size = terrain_graph.read().graph().tile_size();

    // Check if the terrain graph has new output tiles
    let mesh = match terrain_graph.write().process(selected_node) {
        Ok(Some((generation, tiles))) => {
            if tiles.is_empty() {
                return; // No output tiles available
            }

            terrain_preview.showing_flat_fallback = false;
            let heightmap = &tiles[0]; // For now, just use the first tile for preview (should be heightmap)
            let internal_size = heightmap.size();
            let data = crop_center(heightmap, internal_size, size);
            heightmap_to_mesh(&format!("terrain-preview-{}", generation), &data, size)
        }
        Ok(None) => return, // No new output tiles available
        Err(e) => {
            match e {
                InputNotConnected { node, socket, .. } => {
                    trace!(
                        "Cannot generate terrain preview: Input not connected for node '{}' at socket {}",
                        node, socket
                    );
                }
                _ => {
                    error!("Error while processing terrain graph: {:?}", e);
                }
            }
            // The selected node can't be evaluated: show a flat terrain instead of leaving
            // whatever was last rendered on screen.
            if terrain_preview.showing_flat_fallback {
                return;
            }
            terrain_preview.showing_flat_fallback = true;
            heightmap_to_mesh("terrain-preview-flat", &vec![0.0; size * size], size)
        }
    };

    // Reuse the existing mesh asset and entity if present, otherwise create a new one
    match &terrain_preview.mesh_handle {
        Some(handle) => {
            if let Err(e) = meshes.insert(handle.id(), mesh) {
                error!("Failed to update terrain preview mesh: {:?}", e);
            }
        }
        None => {
            let handle = meshes.add(mesh);
            terrain_preview.current_go = Some(
                commands
                    .spawn((
                        Name::new("Terrain Preview"),
                        Transform::default(),
                        Mesh3d(handle.clone()),
                        PbrMaterial3d(terrain_preview.material_handle.clone().unwrap()),
                        CastShadow
                    ))
                    .id()
            );
            terrain_preview.mesh_handle = Some(handle);
        }
    }
}
