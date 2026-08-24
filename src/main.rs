use bevy::prelude::*;
use waterdrop_terrain_editor::{
    TerrainGraph,
    nodes::{NodeErosion, NodeGeneratorPerlin},
    render, ui
};
use wde::{CustomBevyPlugins, prelude::*};

/// Custom WaterDropEngine plugins.
#[derive(Default)]
struct CustomWdePlugins;
impl Plugin for CustomWdePlugins {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            wde::wde_logger::LogPlugin {
                level: wde::wde_logger::LogLevel::INFO,
                log_file: std::env::temp_dir()
                    .join("waterdrop-terrain-editor")
                    .join("log.txt"),
                ..default()
            }
            .with_crate_level("waterdrop_terrain_editor", wde::wde_logger::LogLevel::DEBUG),
            wde::wde_renderer::RenderPlugin,
            wde::wde_pbr::PbrPlugin,
            wde::wde_camera::CameraPlugin,
            wde::wde_camera_controller::CameraControllerPlugin,
            wde::wde_gizmos::GizmosPlugin,
            wde::wde_editor::EditorPlugin
        ));

        app.init_resource::<TerrainGraph>()
            .add_plugins((render::RenderPlugin, ui::UIPlugin))
            .add_systems(Startup, (default_scene, default_terrain));
    }
}

fn main() {
    // Create the app
    let mut app = App::new();

    // Add default plugins
    app.add_plugins((CustomBevyPlugins, CustomWdePlugins));

    // Run the app
    app.run();
}

fn default_scene(mut commands: Commands) {
    // Main camera
    commands.spawn((
        Name::new("Main Camera"),
        Transform::from_xyz(2.0, 2.0, 2.0).looking_at(Vec3::ZERO, Vec3::Y),
        ActiveCamera,
        ThirdPersonController::default()
    ));

    // Spawn the lights
    commands.spawn((
        Name::new("Sun Light"),
        DirectionalLight {
            direction: Vec3::new(-1.0, -2.0, -1.0).normalize(),
            intensity: 0.5,
            ..Default::default()
        }
    ));
}

fn default_terrain(mut terrain_graph: ResMut<TerrainGraph>) {
    // For now, create a simple graph
    let graph = terrain_graph.graph_mut();
    let (generator, erosion) = (
        graph.add_node(Box::new(NodeGeneratorPerlin {
            frequency: 2.5,
            octaves: 1,
            amplitude: 1.0
        })),
        // graph.add_node(Box::new(NodeGeneratorFlat)),
        graph.add_node(Box::new(NodeErosion::default()))
    );
    graph
        .connect(generator, 0, erosion, 0)
        .expect("Graph connection should succeed");

    // For now, process the graph once
    terrain_graph
        .process(erosion, 128)
        .expect("Graph processing should succeed");
}
