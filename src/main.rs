use bevy::prelude::*;
use waterdrop_terrain_generator::{DEBUG_MODE, TerrainGraphHolder, render, ui};
use wde::{CustomBevyPlugins, prelude::*};

/// Custom WaterDropEngine plugins.
#[derive(Default)]
struct CustomWdePlugins;
impl Plugin for CustomWdePlugins {
    fn build(&self, app: &mut App) {
        // Must be inserted before `wde::wde_editor::EditorPlugin` and friends, which only fill
        // in the default (enabled) via `init_resource` if it isn't already set.
        app.insert_resource(EngineUiConfig { enabled: DEBUG_MODE });

        app.add_plugins((
            wde::wde_logger::LogPlugin {
                level: wde::wde_logger::LogLevel::INFO,
                log_file: std::env::temp_dir()
                    .join("waterdrop-terrain-generator")
                    .join("log.txt"),
                ..default()
            }
            .with_crate_level("waterdrop_terrain_generator", wde::wde_logger::LogLevel::DEBUG),
            wde::wde_renderer::RenderPlugin {
                window_title: "WaterDrop Terrain Generator".into(),
                window_resolution: (1600, 900),
                // Drop a PNG at assets/icon.png to have it picked up as the taskbar/title bar icon.
                window_icon: std::fs::read("assets/icon.png")
                    .ok()
                    .and_then(|bytes| wde::wde_renderer::core::WindowIcon::from_bytes(&bytes).ok())
            },
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
        Transform::from_xyz(-6.0, 13.0, -4.0),
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
