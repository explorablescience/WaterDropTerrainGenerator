//! "Terrain / Export" window: batch-exports every node marked via the graph's right-click "Mark
//! for Export" menu item (the only way to mark a node - there's no header icon for it) as PNGs -
//! one file per chunk per output socket, named `{node_name}_{output_name}_{cx}_{cy}.png`. Height
//! and Mask outputs are written as 16-bit grayscale, remapped from the panel's value range; Color
//! outputs (textures) are written as 8-bit RGB, already in `[0, 1]`. Replaces the old per-graph
//! `Export`/`Load File` nodes: export is a whole-terrain, cross-node action, not something that
//! belongs wired into the graph itself.

use std::collections::HashSet;
use std::path::Path;

use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, block_on, poll_once};
use rfd::FileDialog;
use wde::prelude::{ui::egui, *};

use crate::{
    TerrainInstanceHolder,
    core::{
        EXPORT_RESOLUTIONS, TileHandle,
        graph::GraphNodeId,
        node::{NParamConstraints, NParamDesc, NParamValue, NodeMessage, NodePortType, ParamUnit}
    },
    ui::{theme, widgets}
};

/// Values are only (re)synced from defaults when the window transitions from closed to open - see
/// `panel_terrain_settings`'s identical rationale.
#[derive(Default)]
pub(super) struct ExportPanelState {
    was_open: bool,
    folder_path: String,
    chunks: f32,
    resolution: String,
    min_value: f32,
    max_value: f32,
    status: Option<Result<String, String>>,
    /// The in-flight export, if any - runs on `AsyncComputeTaskPool` (evaluating a node and
    /// writing its PNGs is blocking work) so the editor UI doesn't freeze while it runs.
    task: Option<Task<Result<String, String>>>
}

fn chunks_desc() -> NParamDesc {
    NParamDesc {
        key: "export_chunks",
        label: "Chunks",
        category: "Export",
        default: NParamValue::Int(1),
        constraints: Some(NParamConstraints::IntRange { min: 1, max: 8 }),
        unit: ParamUnit::None
    }
}

fn value_desc(key: &'static str, label: &'static str) -> NParamDesc {
    NParamDesc {
        key,
        label,
        category: "Export",
        default: NParamValue::Float(0.0),
        constraints: Some(NParamConstraints::FloatRange {
            min: -50.0,
            max: 50.0
        }),
        unit: ParamUnit::None
    }
}

fn resolution_options() -> Vec<String> {
    EXPORT_RESOLUTIONS.iter().map(u32::to_string).collect()
}

pub fn draw_export_panel(
    ctx: Res<UIContext>,
    mut ui_menu: ResMut<UIMenu>,
    terrain_graph: Res<TerrainInstanceHolder>,
    mut state: Local<ExportPanelState>
) {
    let open = ui_menu.clicked_mut("Terrain/Export");
    if !*open {
        state.was_open = false;
        return;
    }
    if !state.was_open {
        state.chunks = 1.0;
        state.resolution = EXPORT_RESOLUTIONS[7].to_string();
        state.min_value = 0.0;
        state.max_value = terrain_graph.read().graph().chunk_grid().max_height();
        state.was_open = true;
    }

    if let Some(task) = &mut state.task
        && let Some(result) = block_on(poll_once(task))
    {
        state.status = Some(result);
        state.task = None;
    }

    let marked: Vec<(GraphNodeId, String)> = {
        let session = terrain_graph.read();
        let graph = session.graph();
        session
            .export_marked_nodes()
            .filter_map(|id| graph.display_name(id).ok().map(|name| (id, name)))
            .collect()
    };

    egui::Window::new("Export Terrain")
        .open(open)
        .resizable(false)
        .default_width(300.0)
        .show(&ctx.0, |ui| {
            section_label(ui, "Tiling");
            widgets::slider(
                ui,
                &chunks_desc(),
                theme::palette::EXPORT_ACCENT,
                &mut state.chunks
            );
            widgets::enum_selector(
                ui,
                "export_resolution",
                "Resolution",
                theme::palette::EXPORT_ACCENT,
                &resolution_options(),
                &mut state.resolution
            );
            ui.add_space(6.0);

            section_label(ui, "Value Range (Height / Mask)");
            widgets::slider(
                ui,
                &value_desc("export_min_value", "Min Value"),
                theme::palette::EXPORT_ACCENT,
                &mut state.min_value
            );
            widgets::slider(
                ui,
                &value_desc("export_max_value", "Max Value"),
                theme::palette::EXPORT_ACCENT,
                &mut state.max_value
            );
            ui.label(
                egui::RichText::new("Color (texture) outputs export as-is - this range doesn't apply to them.")
                    .font(theme::body_font(theme::fonts::FONT_SIZE_SMALL))
                    .color(theme::palette::TEXT_DISABLED)
            );
            ui.add_space(6.0);

            section_label(ui, "Folder");
            ui.label(
                egui::RichText::new(if state.folder_path.is_empty() {
                    "No folder selected"
                } else {
                    &state.folder_path
                })
                .font(theme::body_font(theme::fonts::FONT_SIZE_BODY))
                .color(theme::palette::TEXT_MUTED)
            );
            widgets::button(
                ui,
                "Browse Folder...",
                theme::palette::EXPORT_ACCENT,
                || {
                    let mut dialog = FileDialog::new();
                    if !state.folder_path.is_empty() {
                        dialog = dialog.set_directory(&state.folder_path);
                    }
                    if let Some(dir) = dialog.pick_folder() {
                        state.folder_path = dir.display().to_string();
                    }
                }
            );
            ui.add_space(6.0);

            section_label(ui, "Nodes to Export");
            if marked.is_empty() {
                ui.label(
                    egui::RichText::new(
                        "No nodes marked - right-click a node in the graph and choose \"Mark for Export\"."
                    )
                    .font(theme::body_font(theme::fonts::FONT_SIZE_SMALL))
                    .color(theme::palette::TEXT_DISABLED)
                );
            } else {
                egui::ScrollArea::vertical()
                    .max_height(120.0)
                    .show(ui, |ui| {
                        for (_, name) in &marked {
                            ui.label(
                                egui::RichText::new(name)
                                    .font(theme::body_font(theme::fonts::FONT_SIZE_BODY))
                                    .color(theme::palette::TEXT_MUTED)
                            );
                        }
                    });
            }
            ui.add_space(6.0);

            if state.task.is_none()
                && let Some(status) = &state.status
            {
                let message = match status {
                    Ok(text) => NodeMessage::info(text.clone()),
                    Err(text) => NodeMessage::error(text.clone())
                };
                widgets::node_messages(ui, std::slice::from_ref(&message));
                ui.add_space(4.0);
            }

            if state.task.is_some() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(
                        egui::RichText::new("Exporting...")
                            .font(theme::body_font(theme::fonts::FONT_SIZE_BODY))
                            .color(theme::palette::EXPORT_ACCENT)
                    );
                });
                return;
            }

            let chunks = state.chunks.round().max(1.0) as u32;
            let resolution: u32 = state
                .resolution
                .parse()
                .unwrap_or(EXPORT_RESOLUTIONS[EXPORT_RESOLUTIONS.len() / 2]);
            let (min_value, max_value) = (state.min_value, state.max_value);
            let folder_path = state.folder_path.clone();
            let terrain_graph = terrain_graph.clone();
            widgets::button(ui, "Export All", theme::palette::EXPORT_ACCENT, || {
                state.task = Some(AsyncComputeTaskPool::get().spawn(async move {
                    export_marked_nodes(
                        &terrain_graph,
                        &marked,
                        &folder_path,
                        chunks,
                        resolution,
                        min_value,
                        max_value
                    )
                }));
            });
        });
}

fn export_marked_nodes(
    terrain_graph: &TerrainInstanceHolder,
    marked: &[(GraphNodeId, String)],
    folder_path: &str,
    chunks: u32,
    resolution: u32,
    min_value: f32,
    max_value: f32
) -> Result<String, String> {
    if folder_path.is_empty() {
        return Err("No folder selected".into());
    }
    if marked.is_empty() {
        return Err("No nodes marked for export".into());
    }

    let folder = Path::new(folder_path);
    std::fs::create_dir_all(folder)
        .map_err(|e| format!("Failed to create folder {folder_path}: {e}"))?;

    let side = chunks as usize * resolution as usize;
    let range = (max_value - min_value).max(f32::EPSILON);

    let mut used_stems: HashSet<String> = HashSet::new();
    let mut file_count = 0usize;

    for (node_id, name) in marked {
        // Each output socket's name and resolved dtype (a `Generic` socket, e.g. `Combine`'s,
        // resolves from its actual connections) - read up front, separately from evaluating the
        // node itself, since `evaluate_once` only needs the read lock for its own duration.
        let output_sockets: Vec<(String, Option<NodePortType>)> = {
            let session = terrain_graph.read();
            let graph = session.graph();
            let node = graph.node(*node_id).map_err(|e| format!("'{name}': {e}"))?;
            node.outputs()
                .iter()
                .enumerate()
                .map(|(i, socket)| (socket.name.to_string(), graph.output_dtype(*node_id, i)))
                .collect()
        };

        let tiles = terrain_graph
            .read()
            .graph()
            .evaluate_once(*node_id, side)
            .map_err(|e| format!("Failed to evaluate '{name}': {e}"))?;

        for (socket_idx, tile) in tiles.iter().enumerate() {
            let Some((socket_name, port_type)) = output_sockets.get(socket_idx) else {
                continue;
            };
            let Some(port_type) = port_type else {
                return Err(format!(
                    "'{name}' → '{socket_name}': output type couldn't be resolved (connect it to something once, then try again)"
                ));
            };

            if tile.size() != side {
                return Err(format!(
                    "'{name}' → '{socket_name}' produced a {0}x{0} tile but expected {1}x{1} - try exporting again",
                    tile.size(),
                    side
                ));
            }

            let stem = dedupe_stem(
                &mut used_stems,
                format!(
                    "{}_{}",
                    sanitize_file_name(name),
                    sanitize_file_name(socket_name)
                )
            );
            write_output_chunks(
                tile, *port_type, &stem, folder, chunks, resolution, side, min_value, range
            )?;
            file_count += (chunks * chunks) as usize;
        }
    }

    Ok(format!(
        "Exported {file_count} file{} to {folder_path}",
        if file_count == 1 { "" } else { "s" }
    ))
}

#[allow(clippy::too_many_arguments)]
fn write_output_chunks(
    tile: &TileHandle,
    port_type: NodePortType,
    stem: &str,
    folder: &Path,
    chunks: u32,
    resolution: u32,
    side: usize,
    min_value: f32,
    range: f32
) -> Result<(), String> {
    let resolution = resolution as usize;
    for cy in 0..chunks as usize {
        for cx in 0..chunks as usize {
            let path = folder.join(format!("{stem}_{cx}_{cy}.png"));
            let row_of = |y: usize| (cy * resolution + y) * side + cx * resolution;
            match port_type {
                NodePortType::Height | NodePortType::Mask => {
                    write_grayscale_chunk(tile, &path, resolution, row_of, min_value, range)
                }
                NodePortType::Color => write_color_chunk(tile, &path, resolution, row_of)
            }?;
        }
    }
    Ok(())
}

fn write_grayscale_chunk(
    tile: &TileHandle,
    path: &Path,
    resolution: usize,
    row_of: impl Fn(usize) -> usize,
    min_value: f32,
    range: f32
) -> Result<(), String> {
    let plane = tile.plane(0);
    let mut pixels = Vec::with_capacity(resolution * resolution);
    for y in 0..resolution {
        let row = row_of(y);
        for x in 0..resolution {
            let t = ((plane[row + x] - min_value) / range).clamp(0.0, 1.0);
            pixels.push((t * u16::MAX as f32).round() as u16);
        }
    }
    let image = image::ImageBuffer::<image::Luma<u16>, _>::from_raw(
        resolution as u32,
        resolution as u32,
        pixels
    )
    .expect("pixel buffer is exactly resolution x resolution");
    image
        .save(path)
        .map_err(|e| format!("Failed to write {}: {}", path.display(), e))
}

fn write_color_chunk(
    tile: &TileHandle,
    path: &Path,
    resolution: usize,
    row_of: impl Fn(usize) -> usize
) -> Result<(), String> {
    let mut pixels = Vec::with_capacity(resolution * resolution * 3);
    for y in 0..resolution {
        let row = row_of(y);
        for x in 0..resolution {
            for c in 0..3 {
                let v = tile.plane(c)[row + x].clamp(0.0, 1.0);
                pixels.push((v * u8::MAX as f32).round() as u8);
            }
        }
    }
    let image = image::ImageBuffer::<image::Rgb<u8>, _>::from_raw(
        resolution as u32,
        resolution as u32,
        pixels
    )
    .expect("pixel buffer is exactly resolution x resolution");
    image
        .save(path)
        .map_err(|e| format!("Failed to write {}: {}", path.display(), e))
}

/// Appends `_2`, `_3`, ... until `base` no longer collides with an already-claimed stem in this
/// export run - two marked nodes can share a display name (e.g. both left at the default "Perlin"),
/// or a `Generic` socket could coincidentally share a name with another node's output.
fn dedupe_stem(used: &mut HashSet<String>, base: String) -> String {
    if used.insert(base.clone()) {
        return base;
    }
    let mut n = 2;
    loop {
        let candidate = format!("{base}_{n}");
        if used.insert(candidate.clone()) {
            return candidate;
        }
        n += 1;
    }
}

/// Node display names and socket names are free text - turned into a safe file name stem by
/// replacing anything that isn't alphanumeric, `-` or `_`.
fn sanitize_file_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.is_empty() {
        "node".to_string()
    } else {
        sanitized
    }
}

fn section_label(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .font(theme::heading_font(theme::fonts::FONT_SIZE_HEADING))
            .color(theme::palette::TEXT_DISABLED)
    );
    ui.add_space(4.0);
}
