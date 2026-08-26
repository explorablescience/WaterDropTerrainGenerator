use std::collections::HashMap;
use std::path::PathBuf;

use bevy::{
    input::mouse::{MouseButtonInput, MouseMotion, MouseWheel},
    prelude::*,
    window::WindowResized
};
use egui_tiles::{Linear, LinearDir, TileId, Tiles, Tree};
use rfd::FileDialog;
use wde::prelude::{ui::egui, *};

use crate::{
    TerrainGraphHolder,
    ui::{
        editor_behavior, editor_behavior::EditorBehavior, footer::EditorFooterSet,
        panel_graph::GraphInstance, project_io
    }
};

/// Extension (without the leading dot) used for saved terrain graph project files.
const PROJECT_FILE_EXTENSION: &str = "wdtg";
const PROJECT_FILE_FILTER_NAME: &str = "WaterDrop Terrain Graph";

pub struct EditorPanelsPlugin;
impl Plugin for EditorPanelsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EngineViewportRect>()
            .add_systems(
                PreUpdate,
                block_camera_input_outside_engine.after(ui::EguiInputSet)
            )
            .add_systems(
                Update,
                draw_editor.after(EditorMenuBarSet).after(EditorFooterSet)
            );
    }
}

/// List of the editor's panels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EditorPanels {
    Engine,
    Graph,
    Properties
}

/// Stores the layout of the different panels
struct EditorLayout {
    tree: Tree<EditorPanels>,
    panel_to_id: HashMap<EditorPanels, TileId>
}
impl Default for EditorLayout {
    fn default() -> Self {
        let mut tiles = Tiles::default();
        let mut panel_to_id = HashMap::new();
        let engine = tiles.insert_pane(EditorPanels::Engine);
        panel_to_id.insert(EditorPanels::Engine, engine);
        let graph_editor = tiles.insert_pane(EditorPanels::Graph);
        panel_to_id.insert(EditorPanels::Graph, graph_editor);
        let node_info = tiles.insert_pane(EditorPanels::Properties);
        panel_to_id.insert(EditorPanels::Properties, node_info);

        // Split the layout into the different panels
        let main_column = tiles.insert_container(Linear::new_binary(
            LinearDir::Vertical,
            [engine, graph_editor],
            0.6
        ));
        let root = tiles.insert_container(Linear::new_binary(
            LinearDir::Horizontal,
            [main_column, node_info],
            0.75
        ));

        EditorLayout {
            tree: Tree::new("editor", root, tiles),
            panel_to_id
        }
    }
}

#[derive(Resource, Default)]
struct EngineViewportRect(Option<egui::Rect>);

// Utility function to block camera input outside the engine viewport
fn block_camera_input_outside_engine(
    engine_rect: Res<EngineViewportRect>,
    windows: Query<&Window>,
    mut mouse_input: ResMut<ButtonInput<MouseButton>>,
    mut mouse_wheel_messages: ResMut<Messages<MouseWheel>>,
    mut mouse_button_input_messages: ResMut<Messages<MouseButtonInput>>,
    mut mouse_motion_messages: ResMut<Messages<MouseMotion>>
) {
    let cursor_pos = match windows
        .iter()
        .next()
        .and_then(|window| window.cursor_position())
    {
        Some(pos) => pos,
        None => return
    };
    let pointer_pos = egui::pos2(cursor_pos.x, cursor_pos.y);

    let over_engine = engine_rect.0.is_some_and(|rect| rect.contains(pointer_pos));
    if over_engine {
        return;
    }

    mouse_input.reset_all();
    mouse_wheel_messages.clear();
    mouse_button_input_messages.clear();
    mouse_motion_messages.clear();
}

fn draw_editor(
    ctx: Res<UIContext>,
    mut layout: Local<Option<EditorLayout>>,
    mut window_resized: MessageReader<WindowResized>,
    mut generation_id: Local<u64>,
    mut engine_rect: ResMut<EngineViewportRect>,
    mut graph_instance: Local<Option<GraphInstance>>,
    terrain_graph: Res<TerrainGraphHolder>,
    mut ui_menu: ResMut<UIMenu>,
    mut last_project_path: Local<Option<PathBuf>>
) {
    // Avoid weird resizing issues, so reset the graph generation on window resize
    if window_resized.read().count() > 0 {
        *generation_id += 1;
    }

    handle_file_menu(
        &mut ui_menu,
        graph_instance.get_or_insert_default(),
        &terrain_graph,
        &mut last_project_path
    );

    // Create the central panel with the editor layout
    let frame = egui::Frame::central_panel(&ctx.0.style())
        .inner_margin(0.0)
        .fill(egui::Color32::TRANSPARENT);
    let layout = layout.get_or_insert_default();
    let mut outer_rect = egui::Rect::NOTHING;
    egui::CentralPanel::default()
        .frame(frame)
        .show(&ctx.0, |ui| {
            outer_rect = ui.max_rect();
            let mut behavior = EditorBehavior::new(
                &generation_id,
                graph_instance.get_or_insert_default(),
                terrain_graph.clone(),
                outer_rect
            );
            layout.tree.ui(&mut behavior, ui);
        });

    // Update the engine viewport rectangle to the same inset border rect the engine pane draws
    // (see `editor_behavior::panel_border_rect`).
    engine_rect.0 = layout
        .tree
        .tiles
        .rect(layout.panel_to_id[&EditorPanels::Engine])
        .map(|rect| editor_behavior::panel_border_rect(rect, outer_rect));
}

/// Handles clicks on the "File/Save Project" and "File/Load Project" menu entries, prompting for
/// a file via a native dialog and running the matching save/load logic in `project_io`.
fn handle_file_menu(
    ui_menu: &mut UIMenu,
    graph_instance: &mut GraphInstance,
    terrain_graph: &TerrainGraphHolder,
    last_project_path: &mut Option<PathBuf>
) {
    if *ui_menu.clicked_mut("File/Save Project") {
        *ui_menu.clicked_mut("File/Save Project") = false;

        let mut dialog = FileDialog::new().add_filter(PROJECT_FILE_FILTER_NAME, &[PROJECT_FILE_EXTENSION]);
        if let Some(path) = last_project_path.as_deref() {
            if let Some(dir) = path.parent() {
                dialog = dialog.set_directory(dir);
            }
            if let Some(name) = path.file_name() {
                dialog = dialog.set_file_name(name.to_string_lossy());
            }
        }

        if let Some(mut path) = dialog.save_file() {
            path.set_extension(PROJECT_FILE_EXTENSION);
            match project_io::save_project(&path, graph_instance, terrain_graph) {
                Ok(()) => {
                    info!("Saved terrain graph to '{}'", path.display());
                    *last_project_path = Some(path);
                }
                Err(e) => error!("Failed to save terrain graph to '{}': {}", path.display(), e)
            }
        }
    }

    if *ui_menu.clicked_mut("File/Load Project") {
        *ui_menu.clicked_mut("File/Load Project") = false;

        let mut dialog = FileDialog::new().add_filter(PROJECT_FILE_FILTER_NAME, &[PROJECT_FILE_EXTENSION]);
        if let Some(dir) = last_project_path.as_deref().and_then(|p| p.parent()) {
            dialog = dialog.set_directory(dir);
        }

        if let Some(path) = dialog.pick_file() {
            match project_io::load_project(&path, graph_instance, terrain_graph) {
                Ok(()) => {
                    info!("Loaded terrain graph from '{}'", path.display());
                    *last_project_path = Some(path);
                }
                Err(e) => error!("Failed to load terrain graph from '{}': {}", path.display(), e)
            }
        }
    }
}
