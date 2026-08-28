//! "Terrain / Settings" window: chunk-grid parameters, scoped to the whole project so they live
//! here instead of `panel_properties`.

use bevy::prelude::*;
use wde::prelude::{ui::egui, *};

use crate::{
    TerrainSessionHolder, core::{
        node::{NParamConstraints, NParamDesc, NParamValue}, parallelism::TILE_RESOLUTIONS, tiling::ChunkGrid
    }, ui::{theme, widgets}
};


/// Values are (re)synced from the graph's actual chunk grid each time the window transitions from closed to open, so editing here doesn't fight with e.g. a project load changing the grid while it's sitting open.
/// The default values are set by the `ChunkGrid` constructor.
#[derive(Default)]
pub(super) struct TerrainSettingsState {
    was_open: bool,
    chunks_x: f32,
    chunks_y: f32,
    tile_size: String,
    world_scale: f32
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

fn tile_resolution_options() -> Vec<String> {
    TILE_RESOLUTIONS.iter().map(usize::to_string).collect()
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
        state.tile_size = grid.tile_size().to_string();
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
                &int_field("chunks_x", "Chunks X", 1, 12),
                theme::palette::ACCENT,
                &mut state.chunks_x
            );
            widgets::slider(
                ui,
                &int_field("chunks_y", "Chunks Y", 1, 12),
                theme::palette::ACCENT,
                &mut state.chunks_y
            );
            let prev_tile_size = state.tile_size.parse::<usize>().unwrap_or(TILE_RESOLUTIONS[0]);
            let tile_size_changed = egui::Grid::new("terrain-settings-tile-size")
                .num_columns(2)
                .spacing([10.0, 8.0])
                .striped(false)
                .show(ui, |ui| {
                    let response = widgets::enum_selector(
                        ui,
                        "tile_size",
                        "Tile Size (texels)",
                        theme::palette::ACCENT,
                        &tile_resolution_options(),
                        &mut state.tile_size
                    );
                    ui.end_row();
                    response.changed()
                })
                .inner;
            if tile_size_changed
                && let Ok(new_tile_size) = state.tile_size.parse::<usize>()
                && new_tile_size > 0
            {
                state.world_scale *= prev_tile_size as f32 / new_tile_size as f32;
            }
            widgets::slider(
                ui,
                &NParamDesc {
                    key: "world_scale",
                    label: "World Scale (units/texel)",
                    category: "Chunk Grid",
                    default: NParamValue::Float(state.world_scale),
                    constraints: Some(NParamConstraints::FloatRange {
                        min: 0.001,
                        max: 1.0
                    })
                },
                theme::palette::ACCENT,
                &mut state.world_scale
            );
            ui.add_space(4.0);

            let (chunks_x, chunks_y, tile_size, world_scale) = (
                state.chunks_x,
                state.chunks_y,
                state.tile_size.parse::<usize>().unwrap_or(TILE_RESOLUTIONS[0]),
                state.world_scale
            );
            widgets::button(ui, "Apply", theme::palette::ACCENT, || {
                let grid = ChunkGrid::new(
                    chunks_x.round() as u32,
                    chunks_y.round() as u32,
                    tile_size,
                    world_scale
                );
                terrain_graph.write().graph_mut().set_chunk_grid(grid);
            });
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
