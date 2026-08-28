use egui_tiles::{Behavior, TabState, Tiles, UiResponse};
use wde::prelude::ui::egui;

use crate::{
    TerrainSessionHolder,
    ui::{
        editor::EditorPanels,
        panel_graph::{self, GraphInstance},
        panel_properties, theme
    }
};

pub struct EditorBehavior<'a> {
    pub graph_id: egui::Id,
    pub graph_instance: &'a mut GraphInstance,

    pub terrain_graph: TerrainSessionHolder,

    /// Used to tell a tile's outer edges (which get the full border inset) apart from edges shared with a neighboring tile (which get half of it).
    pub outer_rect: egui::Rect
}
impl<'a> Behavior<EditorPanels> for EditorBehavior<'a> {
    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        _tile_id: egui_tiles::TileId,
        pane: &mut EditorPanels
    ) -> egui_tiles::UiResponse {
        let tile_rect = ui.max_rect();
        let border_rect = panel_border_rect(tile_rect, self.outer_rect);
        if matches!(pane, EditorPanels::Engine) {
            paint_frame_gap(
                ui.painter(),
                tile_rect,
                border_rect,
                theme::palette::BG_EXTREME
            );
        } else {
            ui.painter()
                .rect_filled(tile_rect, 0, theme::palette::BG_EXTREME);
        }

        let content_rect = if matches!(pane, EditorPanels::Engine) {
            border_rect
        } else {
            let card_rect = border_rect.shrink(theme::layout::CARD_PADDING);
            ui.painter().rect_filled(
                card_rect,
                egui::CornerRadius::same(theme::layout::CARD_ROUNDING),
                theme::palette::BG_CARD
            );
            card_rect
        };

        let content_clip = ui.clip_rect().intersect(content_rect);
        ui.scope_builder(egui::UiBuilder::new().max_rect(content_rect), |ui| {
            ui.set_clip_rect(content_clip);
            match pane {
                EditorPanels::Engine => {}
                EditorPanels::Graph => {
                    let (selected_node, pinned_node) = panel_graph::show_graph(
                        self.graph_id,
                        ui,
                        self.graph_instance,
                        self.terrain_graph.clone()
                    );
                    let old_selected_node = self.terrain_graph.read().selected_node;
                        let render_node = pinned_node.or(selected_node);
                        if old_selected_node != render_node.map(|node| node.graph_id) {
                        self.terrain_graph.write().selected_node =
                            render_node.map(|node| node.graph_id);
                    }
                }
                EditorPanels::Properties => {
                    let selected_node = panel_graph::selected_node(ui.ctx());
                    panel_properties::draw_properties(
                        ui,
                        &self.terrain_graph,
                        selected_node.map(|node| node.graph_id)
                    );
                }
            }
        });

        UiResponse::None
    }

    fn tab_title_for_pane(&mut self, pane: &EditorPanels) -> egui::WidgetText {
        Self::get_pane_title(pane).into()
    }

    /// Thin accent-colored outline instead of the default, so the focused panel is unambiguous at a glance.
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
        terrain_graph: TerrainSessionHolder,
        outer_rect: egui::Rect
    ) -> Self {
        EditorBehavior {
            graph_id: egui::Id::new("editor-graph-id").with(*generation_id),
            graph_instance,
            terrain_graph,
            outer_rect
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

/// Half inset on edges shared with a neighboring tile, so the gap between two adjacent panels matches the gap to the outer edge instead of doubling.
pub fn panel_border_rect(tile_rect: egui::Rect, outer_rect: egui::Rect) -> egui::Rect {
    const EPS: f32 = 0.5;
    let full = theme::layout::PANEL_BORDER_INSET;
    let half = full * 0.5;

    let left = if (tile_rect.left() - outer_rect.left()).abs() < EPS {
        full
    } else {
        half
    };
    let top = if (tile_rect.top() - outer_rect.top()).abs() < EPS {
        theme::layout::PANEL_TOP_INSET
    } else {
        half
    };
    let right = if (tile_rect.right() - outer_rect.right()).abs() < EPS {
        full
    } else {
        half
    };
    let bottom = if (tile_rect.bottom() - outer_rect.bottom()).abs() < EPS {
        full
    } else {
        half
    };

    egui::Rect::from_min_max(
        egui::pos2(tile_rect.left() + left, tile_rect.top() + top),
        egui::pos2(tile_rect.right() - right, tile_rect.bottom() - bottom)
    )
}

/// Backdrops the margin around the engine pane's live viewport without ever painting over it.
fn paint_frame_gap(
    painter: &egui::Painter,
    outer: egui::Rect,
    inner: egui::Rect,
    color: egui::Color32
) {
    painter.rect_filled(
        egui::Rect::from_min_max(outer.left_top(), egui::pos2(outer.right(), inner.top())),
        0,
        color
    );
    painter.rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(outer.left(), inner.bottom()),
            outer.right_bottom()
        ),
        0,
        color
    );
    painter.rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(outer.left(), inner.top()),
            egui::pos2(inner.left(), inner.bottom())
        ),
        0,
        color
    );
    painter.rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(inner.right(), inner.top()),
            egui::pos2(outer.right(), inner.bottom())
        ),
        0,
        color
    );
}
