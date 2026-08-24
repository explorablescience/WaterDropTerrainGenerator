use bevy::prelude::*;
use waterdrop_terrain_editor::{
    TerrainGraphHolder, render, ui
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

        app.init_resource::<TerrainGraphHolder>()
            .add_plugins((render::RenderPlugin, ui::UIPlugin))
            .add_systems(Startup, default_scene);
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
