use egui_tiles::{Behavior, TabState, Tiles, UiResponse};
use wde::prelude::ui::egui;

use crate::{
    TerrainGraphHolder,
    ui::{
        editor::EditorPanels,
        panel_graph::{self, GraphInstance},
        panel_properties, theme
    }
};

/// Define the behavior of the editor's panels, including how they are displayed and interacted with.
pub struct EditorBehavior<'a> {
    /// Unique identifier for the graph editor pane
    pub graph_id: egui::Id,
    /// Instance of the graph editor's underlying data structure
    pub graph_instance: &'a mut GraphInstance,

    /// Reference to the terrain graph holder, which manages the terrain graph data
    pub terrain_graph: TerrainGraphHolder
}
impl<'a> Behavior<EditorPanels> for EditorBehavior<'a> {
    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        _tile_id: egui_tiles::TileId,
        pane: &mut EditorPanels
    ) -> egui_tiles::UiResponse {
        let title = Self::get_pane_title(pane);

        if matches!(pane, EditorPanels::Engine) {
            // Minimal floating HUD label over the transparent viewport (no backdrop).
            egui::Frame::NONE
                .inner_margin(egui::Margin::symmetric(10, 6))
                .show(ui, |_ui| {});
        } else {
            // // Solid panel backdrop with a proper header + separator, like a docked tool panel.
            // ui.painter()
            //     .rect_filled(ui.max_rect(), 0.0, egui::Color32::from_rgb(200, 20, 20)); //ui.visuals().panel_fill

            // egui::Frame::NONE
            //     .inner_margin(egui::Margin::symmetric(12, 8))
            //     .show(ui, |ui| {
            //         ui.label(
            //             egui::RichText::new(title)
            //                 .font(theme::heading_font(13.0))
            //                 .color(theme::palette::TEXT)
            //         );
            //     });
            // ui.separator();
            // ui.add_space(2.0);
        }

        match pane {
            EditorPanels::Engine => {}
            EditorPanels::Graph => {
                // let selected_node = panel_graph::show_graph(
                //     self.graph_id,
                //     ui,
                //     self.graph_instance,
                //     self.terrain_graph.clone()
                // );
                // let old_selected_node = self.terrain_graph.read().selected_node;
                // if old_selected_node != selected_node.map(|node| node.graph_id) {
                //     self.terrain_graph.write().selected_node =
                //         selected_node.map(|node| node.graph_id);
                // }
            }
            EditorPanels::Properties => {
                // let selected_node = self.terrain_graph.read().selected_node; // Watch out for deadlocks
                // panel_properties::draw_properties(ui, &self.terrain_graph, selected_node);
            }
        }
        UiResponse::None
    }

    fn tab_title_for_pane(&mut self, pane: &EditorPanels) -> egui::WidgetText {
        Self::get_pane_title(pane).into()
    }

    /// Give the active tab a thin accent-colored outline instead of the default plain one, so
    /// the currently focused panel is unambiguous at a glance.
    fn tab_outline_stroke(
        &self,
        _visuals: &egui::Visuals,
        _tiles: &Tiles<EditorPanels>,
        _tile_id: egui_tiles::TileId,
        state: &TabState
    ) -> egui::Stroke {
        if state.active {
            egui::Stroke::new(1.5, theme::palette::ACCENT)
        } else {
            egui::Stroke::NONE
        }
    }
}
impl<'a> EditorBehavior<'a> {
    pub fn new(
        generation_id: &u64,
        graph_instance: &'a mut GraphInstance,
        terrain_graph: TerrainGraphHolder
    ) -> Self {
        EditorBehavior {
            graph_id: egui::Id::new("editor-graph-id").with(*generation_id),
            graph_instance,
            terrain_graph
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
