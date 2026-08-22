use bevy::input::mouse::{MouseButtonInput, MouseMotion, MouseWheel};
use bevy::prelude::*;
use bevy::window::WindowResized;
use egui_snarl::{
    Snarl,
    ui::{SnarlStyle, get_selected_nodes}
};
use egui_tiles::{Behavior, Linear, LinearDir, TileId, Tiles, Tree, UiResponse};
use wde::prelude::{EditorMenuBarSet, UIContext};
use wde::prelude::ui::egui;
use wde::prelude::ui::EguiInputSet;

use super::snarl_demo::{DemoNode, GRAPH_ID_SALT, new_demo_graph, show_demo_graph};

/// Screen rect of the engine pane, in logical window pixels, updated every frame by
/// [`show_tiles`]. Used by [`block_camera_input_outside_engine`] to keep the game's camera
/// controller from reacting to mouse input meant for one of the other panes (e.g. wheel-zooming
/// the graph editor).
///
/// egui's own `Context::wants_pointer_input` can't tell panes within a single `CentralPanel`
/// apart (the whole panel counts as "used" as soon as it's shown), so it can't be used for this.
#[derive(Resource, Default)]
struct EngineViewportRect(Option<egui::Rect>);

/// The panes of the editor's tiled layout.
enum EditorPane {
    /// Top pane: the live engine viewport (rendered behind the UI).
    Engine,
    /// Bottom pane: the node-graph editor.
    GraphEditor,
    /// Right pane: infos about the currently selected graph node.
    NodeInfo
}

pub struct EditorDockPlugin;
impl Plugin for EditorDockPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EngineViewportRect>()
            // Must run after `EguiInputSet` (`wde_egui`'s `handle_input`) so egui itself still
            // gets to see e.g. a scroll event over the graph editor pane before we clear it out
            // from under the game's camera controller.
            .add_systems(PreUpdate, block_camera_input_outside_engine.after(EguiInputSet))
            // `show_tiles` draws into a screen-filling `egui::CentralPanel`, which egui requires
            // to be added after every other panel (here, the editor's top menu bar) in the same frame.
            .add_systems(Update, show_tiles.after(EditorMenuBarSet));
    }
}

/// The tiled layout plus the id of its engine pane, so its on-screen rect can be read back
/// after showing the tree (see [`show_tiles`]).
struct EditorLayout {
    tree: Tree<EditorPane>,
    engine_id: TileId
}

/// Builds the initial layout: engine on top, graph editor below it, node info on the right.
fn new_layout() -> EditorLayout {
    let mut tiles = Tiles::default();
    let engine = tiles.insert_pane(EditorPane::Engine);
    let graph_editor = tiles.insert_pane(EditorPane::GraphEditor);
    let node_info = tiles.insert_pane(EditorPane::NodeInfo);

    let main_column =
        tiles.insert_container(Linear::new_binary(LinearDir::Vertical, [engine, graph_editor], 0.6));
    let root =
        tiles.insert_container(Linear::new_binary(LinearDir::Horizontal, [main_column, node_info], 0.75));

    EditorLayout { tree: Tree::new("editor-layout", root, tiles), engine_id: engine }
}

/// Keeps the game's camera controller from reacting to mouse input over any pane except the
/// engine viewport (e.g. wheel-zooming the graph editor should not also zoom the 3D camera).
///
/// Runs in `PreUpdate`, so it always clears the relevant input resources before any `Update`
/// system (like the camera controller) gets a chance to read them this frame. It uses last
/// frame's engine rect, which is fine in practice since the layout rarely changes frame to frame.
fn block_camera_input_outside_engine(
    engine_rect: Res<EngineViewportRect>,
    windows: Query<&Window>,
    mut mouse_input: ResMut<ButtonInput<MouseButton>>,
    mut mouse_wheel_messages: ResMut<Messages<MouseWheel>>,
    mut mouse_button_input_messages: ResMut<Messages<MouseButtonInput>>,
    mut mouse_motion_messages: ResMut<Messages<MouseMotion>>
) {
    let Some(window) = windows.iter().next() else { return };
    let Some(cursor_pos) = window.cursor_position() else { return };
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

struct EditorBehavior<'a> {
    snarl: &'a mut Snarl<DemoNode>,
    snarl_style: SnarlStyle,
    /// Widget id of the graph editor, bumped on window resize (see [`show_tiles`]) so
    /// egui-snarl re-fits its view to the graph instead of keeping its old, now-stale pan/zoom.
    graph_id: egui::Id
}
impl Behavior<EditorPane> for EditorBehavior<'_> {
    fn tab_title_for_pane(&mut self, pane: &EditorPane) -> egui::WidgetText {
        pane_title(pane).into()
    }

    fn pane_ui(&mut self, ui: &mut egui::Ui, _tile_id: TileId, pane: &mut EditorPane) -> UiResponse {
        if !matches!(pane, EditorPane::Engine) {
            // Give every pane but the engine viewport a solid backdrop, so the 3D scene
            // rendered behind egui only shows through the engine pane.
            ui.painter().rect_filled(ui.max_rect(), 0.0, ui.visuals().panel_fill);
        }

        ui.label(egui::RichText::new(pane_title(pane)).strong());
        ui.separator();

        match pane {
            EditorPane::Engine => {
                ui.label(egui::RichText::new("Engine viewport").weak());
            }
            EditorPane::GraphEditor => show_demo_graph(ui, self.snarl, self.snarl_style, self.graph_id),
            EditorPane::NodeInfo => show_node_info(ui, self.snarl, self.graph_id)
        }

        UiResponse::None
    }
}

fn pane_title(pane: &EditorPane) -> &'static str {
    match pane {
        EditorPane::Engine => "Engine",
        EditorPane::GraphEditor => "Graph Editor",
        EditorPane::NodeInfo => "Node Info"
    }
}

/// Demo placeholder listing the graph nodes currently selected in the graph editor pane.
fn show_node_info(ui: &mut egui::Ui, snarl: &Snarl<DemoNode>, graph_id: egui::Id) {
    let selected = get_selected_nodes(graph_id, ui.ctx());
    if selected.is_empty() {
        ui.weak("No node selected.");
        return;
    }

    for node_id in selected {
        let Some(node) = snarl.get_node(node_id) else { continue };
        ui.label(format!("Node #{}: {}", node_id.0, node.label()));
    }
}

fn show_tiles(
    ctx: Res<UIContext>,
    mut layout: Local<Option<EditorLayout>>,
    mut snarl: Local<Option<Snarl<DemoNode>>>,
    mut window_resized: MessageReader<WindowResized>,
    mut graph_generation: Local<u64>,
    mut engine_rect: ResMut<EngineViewportRect>
) {
    let layout = layout.get_or_insert_with(new_layout);
    let snarl = snarl.get_or_insert_with(new_demo_graph);

    // egui-snarl only auto-fits its view to the graph the first time a given widget id is
    // shown; on later frames it keeps reusing the persisted pan/zoom. Since that transform is
    // in absolute screen space, resizing the window (e.g. going fullscreen) leaves the graph
    // panned to where the pane used to be. Bumping the id on resize forces a fresh fit.
    if window_resized.read().count() > 0 {
        *graph_generation += 1;
    }
    let graph_id = egui::Id::new(GRAPH_ID_SALT).with(*graph_generation);

    let mut behavior = EditorBehavior { snarl, snarl_style: SnarlStyle::default(), graph_id };

    // Leave the panel itself transparent: panes paint their own backdrop (see `pane_ui`),
    // except the engine pane, which is left see-through so the rendered scene shows behind it.
    let frame = egui::Frame::central_panel(&ctx.0.style())
        .inner_margin(0.0)
        .fill(egui::Color32::TRANSPARENT);
    egui::CentralPanel::default().frame(frame).show(&ctx.0, |ui| {
        layout.tree.ui(&mut behavior, ui);
    });

    engine_rect.0 = layout.tree.tiles.rect(layout.engine_id);
}
