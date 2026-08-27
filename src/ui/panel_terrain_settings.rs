//! "Terrain / Settings" window: chunk-grid parameters and a one-shot whole-terrain export - both scoped to the whole project, so they live here instead of `panel_properties`.

use bevy::prelude::*;
use rfd::FileDialog;
use wde::prelude::{ui::egui, *};

use crate::{
    TerrainSessionHolder,
    core::{
        node::{NParamConstraints, NParamDesc, NParamValue},
        tiling::ChunkGrid
    },
    ui::{theme, widgets}
};

/// Values are (re)synced from the graph's actual chunk grid each time the window transitions from closed to open, so editing here doesn't fight with e.g. a project load changing the grid while it's sitting open.
pub(super) struct TerrainSettingsState {
    was_open: bool,
    chunks_x: f32,
    chunks_y: f32,
    tile_size: f32,
    world_scale: f32,
    export_path: String
}
impl Default for TerrainSettingsState {
    fn default() -> Self {
        Self {
            was_open: false,
            chunks_x: 5.0,
            chunks_y: 5.0,
            tile_size: 128.0,
            world_scale: 1.0 / 128.0 * 2.0 * 5.0,
            export_path: String::new()
        }
    }
}

/// Built fresh each frame just to drive [`widgets::slider`]'s display - not a real node parameter.
fn int_field(key: &'static str, label: &'static str, min: i32, max: i32) -> NParamDesc {
    NParamDesc {
        key,
        label,
        category: "Chunk Grid",
        default: NParamValue::Int(min),
        constraints: Some(NParamConstraints::IntRange { min, max })
    }
}

pub fn draw_terrain_settings(
    ctx: Res<UIContext>,
    mut ui_menu: ResMut<UIMenu>,
    terrain_graph: Res<TerrainSessionHolder>,
    mut state: Local<TerrainSettingsState>
) {
    let open = ui_menu.clicked_mut("Terrain/Settings");
    if !*open {
        state.was_open = false;
        return;
    }
    if !state.was_open {
        let grid = *terrain_graph.read().graph().chunk_grid();
        state.chunks_x = grid.chunks_x() as f32;
        state.chunks_y = grid.chunks_y() as f32;
        state.tile_size = grid.tile_size() as f32;
        state.world_scale = grid.world_scale();
        state.was_open = true;
    }

    egui::Window::new("Terrain Settings")
        .open(open)
        .resizable(false)
        .default_width(260.0)
        .show(&ctx.0, |ui| {
            section_label(ui, "Chunk Grid");

            widgets::slider(
                ui,
                &int_field("chunks_x", "Chunks X", 1, 64),
                theme::palette::ACCENT,
                &mut state.chunks_x
            );
            widgets::slider(
                ui,
                &int_field("chunks_y", "Chunks Y", 1, 64),
                theme::palette::ACCENT,
                &mut state.chunks_y
            );
            widgets::slider(
                ui,
                &int_field("tile_size", "Tile Size (texels)", 4, 1024),
                theme::palette::ACCENT,
                &mut state.tile_size
            );
            widgets::slider(
                ui,
                &NParamDesc {
                    key: "world_scale",
                    label: "World Scale (units/texel)",
                    category: "Chunk Grid",
                    default: NParamValue::Float(state.world_scale),
                    constraints: Some(NParamConstraints::FloatRange {
                        min: 0.001,
                        max: 100.0
                    })
                },
                theme::palette::ACCENT,
                &mut state.world_scale
            );
            ui.add_space(4.0);

            let (chunks_x, chunks_y, tile_size, world_scale) = (
                state.chunks_x,
                state.chunks_y,
                state.tile_size,
                state.world_scale
            );
            widgets::button(ui, "Apply", theme::palette::ACCENT, || {
                let grid = ChunkGrid::new(
                    chunks_x.round() as u32,
                    chunks_y.round() as u32,
                    tile_size.round() as usize,
                    world_scale
                );
                terrain_graph.write().graph_mut().set_chunk_grid(grid);
            });

            ui.add_space(10.0);
            section_label(ui, "Export Whole Terrain");

            widgets::text_field(
                ui,
                "File Path",
                theme::palette::ACCENT,
                &mut state.export_path,
                None
            );
            widgets::button(ui, "Browse...", theme::palette::ACCENT, || {
                let mut dialog = FileDialog::new().add_filter("PNG heightmap", &["png"]);
                if let Some(dir) = std::path::Path::new(&state.export_path).parent()
                    && !dir.as_os_str().is_empty()
                {
                    dialog = dialog.set_directory(dir);
                }
                if let Some(file) = dialog.save_file() {
                    state.export_path = file.display().to_string();
                }
            });

            let selected = terrain_graph.read().selected_node;
            match selected {
                Some(node_id) if !state.export_path.trim().is_empty() => {
                    let export_path = state.export_path.clone();
                    widgets::button(ui, "Export", theme::palette::ACCENT, || {
                        let mut path = std::path::PathBuf::from(&export_path);
                        path.set_extension("png");
                        match terrain_graph.write().export_stitched_png(node_id, &path) {
                            Ok(()) => info!("Exported whole terrain to '{}'", path.display()),
                            Err(e) => error!(
                                "Failed to export whole terrain to '{}': {}",
                                path.display(),
                                e
                            )
                        }
                    });
                }
                _ => {
                    ui.label(
                        egui::RichText::new(
                            "Select a node in the graph and set a file path to export."
                        )
                        .font(theme::body_font(theme::fonts::FONT_SIZE_SMALL))
                        .color(theme::palette::TEXT_DISABLED)
                    );
                }
            }
        });
}

fn section_label(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .font(theme::heading_font(theme::fonts::FONT_SIZE_HEADING))
            .color(theme::palette::TEXT_DISABLED)
    );
    ui.add_space(4.0);
}
