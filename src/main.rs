use bevy::{
    app::{ScheduleRunnerPlugin, TaskPoolThreadAssignmentPolicy},
    diagnostic::FrameCountPlugin,
    input::InputPlugin,
    prelude::*,
    time::TimePlugin
};
use waterdrop_terrain_generator::{TerrainSessionHolder, render, ui};
use wde::prelude::*;

#[derive(Default)]
struct CustomWdePlugins;
impl Plugin for CustomWdePlugins {
    fn build(&self, app: &mut App) {
        // Show or not the built-in UI menu items from WDE.
        app.insert_resource(EngineUiConfig { enabled: false });

        app.add_plugins((
            wde::wde_logger::LogPlugin {
                level: wde::wde_logger::LogLevel::INFO,
                log_file: std::env::temp_dir()
                    .join("waterdrop-terrain-generator")
                    .join("log.txt"),
                ..default()
            }
            .with_crate_level(
                "waterdrop_terrain_generator",
                wde::wde_logger::LogLevel::DEBUG
            ),
            wde::wde_renderer::RenderPlugin {
                window_title: "WaterDrop Terrain Generator".into(),
                window_resolution: (1600, 900),
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

        app.init_resource::<TerrainSessionHolder>()
            .add_plugins((render::RenderPlugin, ui::UIPlugin))
            .add_systems(Startup, default_scene);
    }
}

fn main() {
    // Split the available CPU cores between Bevy's task pool and Rayon. `async_compute_threads` is
    // sized directly (min == max) for concurrent per-chunk dispatch rather than left as leftover
    // budget - `TaskPoolOptions` allocates IO, then async compute, then compute, each forced to at
    // least its own `min_threads`, so a small shared `max_total_threads` used to let IO's forced
    // thread eat into async compute's share and starve it down to a single thread on machines with
    // few cores. Rayon gets whatever's left, for per-node internal parallelism (e.g. erosion, tile
    // resampling).
    let cores = std::thread::available_parallelism().map_or(4, |n| n.get());
    let async_compute_threads = (cores / 4).clamp(2, 8);
    let bevy_threads = async_compute_threads + 2; // + IO(1) + Compute(1)
    let rayon_threads = cores.saturating_sub(bevy_threads).max(1);

    let _ = rayon::ThreadPoolBuilder::new()
        .num_threads(rayon_threads)
        // Otherwise these threads inherit the main thread's OS name, and profilers like Tracy
        // (which falls back to /proc/self/task/<tid>/comm) can't tell them apart from it.
        .thread_name(|i| format!("Rayon {i}"))
        .build_global();
    let bevy_task_pool = TaskPoolPlugin {
        task_pool_options: TaskPoolOptions {
            min_total_threads: bevy_threads,
            max_total_threads: bevy_threads,
            io: TaskPoolThreadAssignmentPolicy {
                min_threads: 1,
                max_threads: 1,
                percent: 0.25,
                on_thread_spawn: None,
                on_thread_destroy: None
            },
            compute: TaskPoolThreadAssignmentPolicy {
                min_threads: 1,
                max_threads: 1,
                percent: 0.25,
                on_thread_spawn: None,
                on_thread_destroy: None
            },
            async_compute: TaskPoolThreadAssignmentPolicy {
                min_threads: async_compute_threads,
                max_threads: async_compute_threads,
                percent: 1.0,
                on_thread_spawn: None,
                on_thread_destroy: None
            }
        }
    };

    // Create the Bevy app and add the custom plugins.
    let mut app = App::new();
    app.add_plugins((
        FrameCountPlugin,
        TimePlugin,
        ScheduleRunnerPlugin::default(),
        bevy::prelude::AssetPlugin {
            mode: AssetMode::Unprocessed,
            file_path: "res".to_string(),
            ..Default::default()
        },
        InputPlugin,
        TransformPlugin,
        CustomWdePlugins,
        bevy_task_pool
    ));
    app.run();
}

fn default_scene(mut commands: Commands) {
    commands.spawn((
        Name::new("Main Camera"),
        Transform::from_xyz(-6.0, 13.0, -4.0),
        ActiveCamera,
        ThirdPersonController::default()
    ));

    commands.spawn((
        Name::new("Sun Light"),
        DirectionalLight {
            direction: Vec3::new(-1.0, -2.0, -1.0).normalize(),
            intensity: 0.5,
            ..Default::default()
        }
    ));
}
