//! Small status footer along the bottom of the editor window.
//!
//! Shows the same FPS reading WaterDropEngine's built-in overlay would (refreshed every few
//! seconds rather than every frame, since a number that reflows that fast is just noise), plus
//! terrain-editor-specific state: how much memory the tile pool has allocated, and whether the
//! graph is currently processing. The engine's own overlay is disabled in `install_theme` (see
//! [`super::install_theme`]) so the two don't draw on top of each other.

use std::time::{Duration, Instant};

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use wde::prelude::{ui::egui, *};

use crate::{
    TerrainGraphHolder,
    ui::theme::{self, palette}
};

/// System set containing the footer's draw system. Since the footer reserves a bottom strip of
/// the screen via a [`egui::TopBottomPanel`], anything that draws a screen-filling panel (e.g.
/// the editor's [`egui::CentralPanel`]) must run its own drawing system after this set, otherwise
/// both panels would compute their layout before the other has claimed its space.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct EditorFooterSet;

pub struct FooterPlugin;
impl Plugin for FooterPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            draw_footer.after(EditorMenuBarSet).in_set(EditorFooterSet)
        );
    }
}

const FOOTER_HEIGHT: f32 = 30.0;
const STAT_FONT_SIZE: f32 = 11.0;
/// How often the FPS reading is refreshed. Deliberately coarse - a number reflowing every frame
/// is noise, not signal.
const FPS_UPDATE_INTERVAL: Duration = Duration::from_secs(5);

/// The FPS reading shown in the footer, held between refreshes.
#[derive(Default)]
struct FpsDisplay {
    value: Option<f64>,
    last_update: Option<Instant>
}

fn draw_footer(
    ctx: Res<UIContext>,
    diagnostics: Res<DiagnosticsStore>,
    terrain_graph: Res<TerrainGraphHolder>,
    mut fps_display: Local<FpsDisplay>
) {
    if fps_display
        .last_update
        .is_none_or(|t| t.elapsed() >= FPS_UPDATE_INTERVAL)
    {
        fps_display.value = diagnostics
            .get(&FrameTimeDiagnosticsPlugin::FPS)
            .and_then(|d| d.smoothed());
        fps_display.last_update = Some(Instant::now());
    }
    let fps = fps_display.value;

    let (is_processing, tile_bytes) = {
        let terrain_graph = terrain_graph.read();
        let graph = terrain_graph.graph();
        (graph.is_processing(), graph.pool().allocated_bytes())
    };

    let frame = egui::Frame::NONE.fill(palette::BG_EXTREME).inner_margin(egui::Margin {
        left: 32,
        right: 32,
        top: 0,
        bottom: 6
    });

    egui::TopBottomPanel::bottom("wde_terrain_footer")
        .frame(frame)
        .exact_height(FOOTER_HEIGHT)
        .show_separator_line(false)
        .show(&ctx.0, |ui| {
            ui.horizontal(|ui| {
                ui.set_height(ui.available_height());
                status_indicator(ui, is_processing);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    stat(
                        ui,
                        "FPS",
                        fps.map_or_else(|| "n/a".to_string(), |v| format!("{v:.0}"))
                    );
                    separator(ui);
                    stat(ui, "Tiles", format_bytes(tile_bytes));
                });
            });
        });
}

/// A small colored dot plus an "Idle"/"Processing" label: gray while the graph has nothing new
/// to compute, the accent color for a brief moment right after a node actually recomputes.
fn status_indicator(ui: &mut egui::Ui, processing: bool) {
    let color = if processing {
        palette::ACCENT
    } else {
        palette::TEXT_DISABLED
    };
    let label = if processing { "Processing" } else { "Idle" };

    let dot_diameter = 6.0;
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(dot_diameter, dot_diameter),
        egui::Sense::hover()
    );
    ui.painter()
        .circle_filled(rect.center(), dot_diameter * 0.5, color);
    ui.add_space(2.0);

    ui.label(
        egui::RichText::new(label)
            .font(theme::body_font(STAT_FONT_SIZE))
            .color(color)
    );
}

/// A dim `label` followed by a brighter `value`, e.g. "FPS 60". Drawn as its own left-to-right
/// group so it reads correctly regardless of the enclosing layout's direction.
fn stat(ui: &mut egui::Ui, label: &str, value: String) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.label(
            egui::RichText::new(label)
                .font(theme::body_font(STAT_FONT_SIZE))
                .color(palette::TEXT_DISABLED)
        );
        ui.label(
            egui::RichText::new(value)
                .font(theme::body_font(STAT_FONT_SIZE))
                .color(palette::TEXT_MUTED)
        );
    });
}

/// A thin vertical rule separating two stats, since the theme disables egui's own separator
/// stroke for noninteractive widgets.
fn separator(ui: &mut egui::Ui) {
    let height = 12.0;
    ui.add_space(4.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(1.0, height), egui::Sense::hover());
    ui.painter().line_segment(
        [rect.center_top(), rect.center_bottom()],
        egui::Stroke::new(1.0, palette::BORDER)
    );
    ui.add_space(4.0);
}

fn format_bytes(bytes: usize) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    let bytes = bytes as f64;
    if bytes >= MB {
        format!("{:.1} MB", bytes / MB)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes / KB)
    } else {
        format!("{bytes:.0} B")
    }
}
