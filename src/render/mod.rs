use bevy::prelude::*;
use wde::prelude::*;

use crate::{
    core::graph::NodeGraph, nodes::{NodeErosion, NodeGeneratorPerlin}, render::mesh_generation::heightmap_to_mesh,
};

mod mesh_generation;

/// Number of texels per side of the heightmap tile requested from the graph.
const TILE_SIZE: usize = 128;
/// World-space distance between adjacent heightmap samples.
const CELL_SIZE: f32 = 0.1;
/// World-space height gained per unit of heightmap value.
const HEIGHT_SCALE: f32 = 1.0;


pub struct RenderPlugin;
impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_terrain_preview);
    }
}

/// Runs the node graph, then spawns the resulting heightmap as a plain white,
/// PBR-shaded (and shadow-casting) terrain mesh.
pub fn spawn_terrain_preview(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<PbrMaterial>>,
) {
    let mut graph = NodeGraph::new();
    let (generator, erosion) = (
        graph.add_node(Box::new(NodeGeneratorPerlin {
            frequency: 2.5,
            octaves: 1,
            amplitude: 1.0,
        })),
        // graph.add_node(Box::new(NodeGeneratorFlat)),
        graph.add_node(Box::new(NodeErosion::default())),
    );
    graph
        .connect(generator, 0, erosion, 0)
        .expect("Graph connection should succeed");
    graph.validate(erosion).expect("Graph should be valid");

    let output_tiles = graph
        .process(erosion, TILE_SIZE)
        .expect("Graph processing should succeed");
    let heightmap = &output_tiles[0];
    let size = heightmap.size();
    let data: Vec<f32> = heightmap.iter().copied().collect();

    let mesh_handle = asset_server.add(heightmap_to_mesh("terrain-heightmap", &data, size));
    let white_material = materials.add(PbrMaterial {
        label: "terrain-white".to_string(),
        albedo: (1.0, 1.0, 1.0, 0.0),
        ..default()
    });

    commands.spawn((
        Name::new("Terrain Preview"),
        Transform::default(),
        Mesh3d(mesh_handle),
        PbrMaterial3d(white_material),
        CastShadow,
    ));
}
