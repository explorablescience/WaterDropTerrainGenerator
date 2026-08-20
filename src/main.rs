use wde::{CustomBevyPlugins, prelude::{*, Color as WdeColor}};
use bevy::prelude::*;

pub(crate) mod core;
pub(crate) mod nodes;

/// Custom WaterDropEngine plugins.
#[derive(Default)]
struct CustomWdePlugins;
impl Plugin for CustomWdePlugins {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            wde::wde_logger::LogPlugin::default().auto_level(),
            wde::wde_renderer::RenderPlugin,
            wde::wde_pbr::PbrPlugin,
            wde::wde_camera::CameraPlugin,
            wde::wde_camera_controller::CameraControllerPlugin,
            wde::wde_gizmos::GizmosPlugin,
            wde::wde_editor::EditorPlugin
        ));

        app.add_systems(Startup, init_scene)
            .add_systems(Update, gizmo_debug);
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

fn init_scene(mut commands: Commands) {
    // Main camera
    commands.spawn((
        Name::new("Main Camera"),
        Transform::from_xyz(2.0, 2.0, 2.0).looking_at(Vec3::ZERO, Vec3::Y),
        ActiveCamera,
        ThirdPersonController::default() // FreeCameraController::default()
    ));
}

fn gizmo_debug(mut gizmos: ResMut<Gizmos>) {
    // Gizmos tests
    gizmos.cube(
        Transform::from_xyz(0.0, 1.5, 0.0).with_scale(Vec3::splat(2.0)),
        WdeColor::from_srgba(1.0, 0.0, 0.0, 1.0)
    );
    gizmos.line(
        Vec3::new(-5.0, 0.1, 0.0),
        Vec3::new(5.0, 0.1, 0.0),
        WdeColor::from_srgba(0.0, 1.0, 0.0, 1.0)
    );
    gizmos.line(
        Vec3::new(0.0, 0.1, -5.0),
        Vec3::new(0.0, 0.1, 5.0),
        WdeColor::from_srgba(0.0, 0.5, 1.0, 1.0)
    );
    gizmos.quad(
        [
            Vec3::new(-2.0, 0.05, -2.0),
            Vec3::new(2.0, 0.05, -2.0),
            Vec3::new(2.0, 0.05, 2.0),
            Vec3::new(-2.0, 0.05, 2.0)
        ],
        WdeColor::from_srgba(1.0, 1.0, 0.0, 0.35)
    );
}
