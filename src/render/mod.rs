use bevy::prelude::*;
use wde::prelude::*;

use crate::{TerrainGraph, render::mesh_generation::heightmap_to_mesh};

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
    generation: u32,
    current_go: Option<Entity>,
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
    asset_server: Res<AssetServer>,
    mut terrain_preview: ResMut<TerrainPreview>,
    terrain_graph: Res<TerrainGraph>
) {
    // Check if the terrain graph has new output tiles
    let state = terrain_graph.state();
    if state.is_none() || state.unwrap().0 == terrain_preview.generation {
        return; // No new output tiles, or already processed this generation
    }

    // Despawn the previous terrain preview entity if it exists
    if let Some(entity) = terrain_preview.current_go.take() {
        commands.entity(entity).despawn();
    }

    // Get the output tiles from the terrain graph state
    let output_tiles = &state.unwrap().1;
    let heightmap = &output_tiles[0]; // For now, just use the first tile for preview (should be heightmap)
    let size = heightmap.size();
    let data: Vec<f32> = heightmap.iter().copied().collect();

    // Spawn the mesh
    let mesh = asset_server.add(heightmap_to_mesh(
        format!("terrain_preview_{}", terrain_preview.generation).as_str(),
        &data,
        size
    ));
    terrain_preview.current_go = Some(
        commands
            .spawn((
                Name::new("Terrain Preview"),
                Transform::default(),
                Mesh3d(mesh),
                PbrMaterial3d(terrain_preview.material_handle.clone().unwrap()),
                CastShadow
            ))
            .id()
    );

    // Update the generation to the latest
    terrain_preview.generation = state.unwrap().0;
}
