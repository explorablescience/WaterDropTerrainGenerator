use egui_snarl::NodeId;
use egui_tiles::{Behavior, UiResponse};
use wde::prelude::ui::egui;

use crate::ui::{
    editor::EditorPanels, panel_graph::{self, GraphInstance}, panel_properties
};

/// Define the behavior of the editor's panels, including how they are displayed and interacted with.
pub struct EditorBehavior<'a> {
    /// Unique identifier for the graph editor pane
    pub graph_id: egui::Id,
    /// Instance of the graph editor's underlying data structure
    pub graph_instance: &'a mut GraphInstance,

    /// Selected node in the graph editor, if any
    pub selected_node: Option<NodeId>,
}
impl<'a> Behavior<EditorPanels> for EditorBehavior<'a> {
    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        _tile_id: egui_tiles::TileId,
        pane: &mut EditorPanels
    ) -> egui_tiles::UiResponse {
        // Add solid backdrop except for engine viewport (transparent)
        if !matches!(pane, EditorPanels::Engine) {
            ui.painter()
                .rect_filled(ui.max_rect(), 0.0, ui.visuals().panel_fill);
        }
        ui.label(egui::RichText::new(Self::get_pane_title(pane)).strong());

        match pane {
            EditorPanels::Engine => {}
            EditorPanels::Graph => {
                let selected_node = panel_graph::show_graph(self.graph_id, ui, self.graph_instance);
                self.selected_node = selected_node;
            }
            EditorPanels::Properties => {
                panel_properties::draw_properties(ui, self.graph_instance, self.selected_node);
            }
        }
        UiResponse::None
    }

    fn tab_title_for_pane(&mut self, pane: &EditorPanels) -> egui::WidgetText {
        Self::get_pane_title(pane).into()
    }
}
impl<'a> EditorBehavior<'a> {
    pub fn new(generation_id: &u64, graph_instance: &'a mut GraphInstance) -> Self {
        EditorBehavior {
            graph_id: egui::Id::new("editor-graph-id").with(*generation_id),
            graph_instance,
            selected_node: None
        }
    }

    pub fn get_pane_title(pane: &EditorPanels) -> &'static str {
        match pane {
            EditorPanels::Engine => "Engine",
            EditorPanels::Graph => "Graph Editor",
            EditorPanels::Properties => "Properties"
        }
    }
}
