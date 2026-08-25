use bevy::prelude::*;
use wde::prelude::*;

use crate::{TerrainGraphHolder, core::node_error::NodeError::InputNotConnected, render::mesh_generation::heightmap_to_mesh};

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
        None => return, // No node selected
    };

    // Check if the terrain graph has new output tiles
    let (generation, tiles) = match terrain_graph.write().process(selected_node) {
        Ok(state) => match state {
            Some((generation, tiles)) => (generation, tiles),
            None => return, // No new output tiles available
        },
        Err(e) => {
            match e {
                InputNotConnected { node, socket } => {
                    trace!("Cannot generate terrain preview: Input not connected for node '{}' at socket {}", node, socket);
                }
                _ => {
                    error!("Error while processing terrain graph: {:?}", e);
                }
            }
            return;
        }
    };

    // Get the output tiles from the terrain graph state
    let heightmap = &tiles[0]; // For now, just use the first tile for preview (should be heightmap)
    let size = heightmap.size();
    let data: Vec<f32> = heightmap.iter().copied().collect();
    let mesh = heightmap_to_mesh(
        &format!("terrain-preview-{}", generation),
        &data,
        size
    );

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
