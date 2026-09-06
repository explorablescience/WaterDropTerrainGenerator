use wde::prelude::{ui::egui, *};

use crate::{
    TerrainInstanceHolder,
    core::{
        CacheEntry, TileHandle,
        graph::GraphNodeId,
        node::{NParamConstraints, NParamValue, NodeError, NodeMessage}
    },
    ui::{theme, widgets}
};

pub fn draw_properties(
    ui: &mut egui::Ui,
    terrain_graph: &TerrainInstanceHolder,
    selected_node: Option<GraphNodeId>
) {
    egui::Frame::NONE
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            if selected_node.is_none() {
                return;
            }
            let graph_id = selected_node.unwrap();

            egui::Frame::new()
                .fill(theme::palette::BG_PANEL)
                .inner_margin(egui::Margin::symmetric(12, 10))
                .corner_radius(egui::CornerRadius::same(theme::layout::CARD_ROUNDING))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());

                    let (label, category, icon) = {
                        let terrain_graph_read = terrain_graph.read();
                        let node = terrain_graph_read
                            .graph()
                            .node(graph_id)
                            .expect("Selected node should exist in the graph");
                        (node.label().to_string(), node.category(), node.icon())
                    };
                    let color = theme::category_color(category);

                    egui::Sides::new().show(
                        ui,
                        |ui| {
                            let (rect, _) = ui.allocate_exact_size(
                                egui::Vec2::splat(theme::fonts::FONT_SIZE_NODE_TITLE),
                                egui::Sense::hover()
                            );
                            widgets::paint_node_icon(ui, rect, icon, color);
                            ui.label(
                                egui::RichText::new(label)
                                    .font(theme::heading_font(theme::fonts::FONT_SIZE_TITLE))
                                    .color(theme::palette::TEXT)
                            );
                        },
                        |ui| {
                            egui::Frame::new()
                                .fill(color.gamma_multiply(0.18))
                                .corner_radius(egui::CornerRadius::same(
                                    theme::layout::CHIP_ROUNDING
                                ))
                                .inner_margin(egui::Margin::symmetric(8, 3))
                                .show(ui, |ui| {
                                    ui.label(
                                        egui::RichText::new(category.display_name())
                                            .font(theme::heading_font(
                                                theme::fonts::FONT_SIZE_SMALL
                                            ))
                                            .color(color)
                                    );
                                });
                        }
                    );
                });

            let messages = collect_node_messages(ui, terrain_graph, graph_id);
            widgets::node_messages(ui, &messages);
            show_node_params(ui, terrain_graph, graph_id);
        });
}

fn collect_node_messages(
    ui: &egui::Ui,
    terrain_graph: &TerrainInstanceHolder,
    graph_id: GraphNodeId
) -> Vec<NodeMessage> {
    terrain_graph.write().drop_expired_messages();

    let terrain_graph = terrain_graph.read();
    let mut messages = Vec::new();
    if let Err(err) = terrain_graph.graph().has_valid_connections(graph_id) {
        let text = match &err {
            NodeError::InputNotConnected {
                node_id,
                node,
                socket
            } => {
                if *node_id == graph_id {
                    format!("Input \"{}\" is not connected", socket)
                } else {
                    format!(
                        "Upstream node \"{}\" has a disconnected input \"{}\"",
                        node, socket
                    )
                }
            }
            _ => err.to_string()
        };
        messages.push(NodeMessage {
            severity: err.severity(),
            text
        });
    }
    if let Some(message) = terrain_graph.action_message(graph_id) {
        messages.push(message.clone());
    }

    if let Some(remaining) = terrain_graph.action_message_remaining(graph_id) {
        ui.ctx().request_repaint_after(remaining);
    }

    messages
}

/// Float/Int parameters read their range straight from `NParamDesc::constraints` inside [`widgets::slider`] instead, since that widget takes the descriptor directly.
enum ParamRange {
    StringMaxLength(usize),
    EnumOneOf(Vec<String>),
    IntList(Vec<i32>),
    None
}

struct ParamSpec {
    key: &'static str,
    label: &'static str,
    category: &'static str,
    default: NParamValue,
    range: ParamRange
}

/// Draws the UI for editing the parameters of a node, grouped into cards by [`NParamDesc::category`].
fn show_node_params(
    ui: &mut egui::Ui,
    terrain_graph: &TerrainInstanceHolder,
    graph_id: GraphNodeId
) {
    let param_specs: Vec<ParamSpec> = {
        let terrain_graph_read = terrain_graph.read();
        let node = terrain_graph_read.graph().node(graph_id).unwrap();
        node.desc_params()
            .iter()
            .map(|desc| {
                let range = match &desc.constraints {
                    Some(NParamConstraints::StringMaxLength { max_length }) => {
                        ParamRange::StringMaxLength(*max_length)
                    }
                    Some(NParamConstraints::EnumOneOf { options }) => {
                        ParamRange::EnumOneOf(options.iter().map(ToString::to_string).collect())
                    }
                    Some(NParamConstraints::IntList { values }) => {
                        ParamRange::IntList(values.clone())
                    }
                    _ => ParamRange::None
                };
                ParamSpec {
                    key: desc.key,
                    label: desc.label,
                    category: desc.category,
                    default: desc.default.clone(),
                    range
                }
            })
            .collect()
    };
    if param_specs.is_empty() {
        return;
    }

    let action_output: Vec<TileHandle> = match terrain_graph.write().get(graph_id) {
        Ok(Some(CacheEntry::Global(_, tiles))) => tiles,
        Ok(Some(CacheEntry::Local(_))) | Ok(None) => Vec::new(),
        Err(NodeError::InputNotConnected { .. }) => Vec::new(), // already shown via has_valid_connections
        Err(err) => {
            terrain_graph.write().set_action_result(graph_id, Err(err));
            Vec::new()
        }
    };

    // Group parameters by category, preserving the order in which each category first appears.
    let mut categories: Vec<(&'static str, Vec<usize>)> = Vec::new();
    for (i, spec) in param_specs.iter().enumerate() {
        match categories.iter_mut().find(|(c, _)| *c == spec.category) {
            Some((_, indices)) => indices.push(i),
            None => categories.push((spec.category, vec![i]))
        }
    }

    for (category, indices) in categories.iter() {
        ui.add_space(6.0);

        egui::Frame::new()
            .fill(theme::palette::BG_WIDGET)
            .inner_margin(egui::Margin {
                left: 0,
                right: 20,
                top: 6,
                bottom: 6
            })
            .corner_radius(egui::CornerRadius::same(theme::layout::CARD_ROUNDING))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());

                egui::CollapsingHeader::new(
                    egui::RichText::new(format!("  {}", category))
                        .font(theme::heading_font(theme::fonts::FONT_SIZE_HEADING))
                        .color(theme::palette::TEXT_DISABLED)
                )
                .icon(widgets::menu_icon)
                .default_open(true)
                .show(ui, |ui| {
                    ui.add_space(2.0);

                    let mut grid_run = 0usize;
                    let mut prev_row_drawn = false;
                    let mut i = 0;
                    while i < indices.len() {
                        // Float/Int/String/Vector2/Action paint their own full-width pill, so they get their own row instead of sharing the label|control grid with Bool/Enum. An `IntList`-constrained Int is picked from a fixed set instead, so it shares Enum's grid row.
                        let is_full_width = |idx: usize| {
                            if matches!(param_specs[idx].range, ParamRange::IntList(_)) {
                                return false;
                            }
                            matches!(
                                param_specs[idx].default,
                                NParamValue::Float(_)
                                    | NParamValue::Int(_)
                                    | NParamValue::String(_)
                                    | NParamValue::Vector2(_, _)
                                    | NParamValue::Vector2Int(_, _)
                                    | NParamValue::Action { .. }
                            )
                        };
                        if prev_row_drawn {
                            ui.add_space(8.0);
                        }
                        if is_full_width(indices[i]) {
                            show_param_row(
                                ui,
                                terrain_graph,
                                graph_id,
                                &param_specs[indices[i]],
                                &action_output
                            );
                            i += 1;
                        } else {
                            let start = i;
                            while i < indices.len() && !is_full_width(indices[i]) {
                                i += 1;
                            }
                            egui::Grid::new(("param-grid", graph_id, *category, grid_run))
                                .num_columns(2)
                                .spacing([10.0, 8.0])
                                .striped(false)
                                .show(ui, |ui| {
                                    for &j in &indices[start..i] {
                                        show_param_row(
                                            ui,
                                            terrain_graph,
                                            graph_id,
                                            &param_specs[j],
                                            &action_output
                                        );
                                    }
                                });
                            grid_run += 1;
                        }
                        prev_row_drawn = true;
                    }
                });
            });
    }
}

/// If the control was edited, writes the new value back into the graph.
fn show_param_row(
    ui: &mut egui::Ui,
    terrain_graph: &TerrainInstanceHolder,
    graph_id: GraphNodeId,
    spec: &ParamSpec,
    action_output: &[TileHandle]
) {
    let current = terrain_graph
        .read()
        .graph()
        .node(graph_id)
        .unwrap()
        .get_param(spec.key)
        .unwrap_or_else(|| spec.default.clone());

    let new_value = match current {
        NParamValue::Float(mut v) => {
            let terrain_graph_read = terrain_graph.read();
            let node = terrain_graph_read.graph().node(graph_id).unwrap();
            let desc = node
                .desc_params()
                .iter()
                .find(|d| d.key == spec.key)
                .unwrap();
            let color = theme::category_color(node.category());
            let changed = widgets::slider(ui, desc, color, &mut v).changed();
            changed.then_some(NParamValue::Float(v))
        }
        NParamValue::Int(v) if matches!(spec.range, ParamRange::IntList(_)) => {
            let ParamRange::IntList(values) = &spec.range else {
                unreachable!("guarded by the match arm above")
            };
            let color = {
                let terrain_graph_read = terrain_graph.read();
                let node = terrain_graph_read.graph().node(graph_id).unwrap();
                theme::category_color(node.category())
            };
            let options: Vec<String> = values.iter().map(ToString::to_string).collect();
            let mut current = v.to_string();
            let changed = widgets::enum_selector(
                ui,
                (graph_id, spec.key),
                spec.label,
                color,
                &options,
                &mut current
            )
            .changed();
            changed
                .then(|| current.parse().ok())
                .flatten()
                .map(NParamValue::Int)
        }
        NParamValue::Int(v) => {
            let terrain_graph_read = terrain_graph.read();
            let node = terrain_graph_read.graph().node(graph_id).unwrap();
            let desc = node
                .desc_params()
                .iter()
                .find(|d| d.key == spec.key)
                .unwrap();
            let color = theme::category_color(node.category());
            let mut v_float = v as f32;
            let changed = widgets::slider(ui, desc, color, &mut v_float).changed();
            changed.then_some(NParamValue::Int(v_float.round() as i32))
        }
        NParamValue::Vector2(x, y) => {
            let terrain_graph_read = terrain_graph.read();
            let node = terrain_graph_read.graph().node(graph_id).unwrap();
            let desc = node
                .desc_params()
                .iter()
                .find(|d| d.key == spec.key)
                .unwrap();
            let color = theme::category_color(node.category());
            let mut v = (x, y);
            let changed = widgets::vector2(ui, desc, color, &mut v).changed();
            changed.then_some(NParamValue::Vector2(v.0, v.1))
        }
        NParamValue::Vector2Int(x, y) => {
            let terrain_graph_read = terrain_graph.read();
            let node = terrain_graph_read.graph().node(graph_id).unwrap();
            let desc = node
                .desc_params()
                .iter()
                .find(|d| d.key == spec.key)
                .unwrap();
            let color = theme::category_color(node.category());
            let mut v = (x as f32, y as f32);
            let changed = widgets::vector2(ui, desc, color, &mut v).changed();
            changed.then_some(NParamValue::Vector2Int(
                v.0.round() as i32,
                v.1.round() as i32
            ))
        }
        NParamValue::Bool(mut v) => {
            let color = {
                let terrain_graph_read = terrain_graph.read();
                let node = terrain_graph_read.graph().node(graph_id).unwrap();
                theme::category_color(node.category())
            };
            let changed = widgets::toggle_switch(ui, spec.label, color, &mut v).changed();
            changed.then_some(NParamValue::Bool(v))
        }
        NParamValue::String(mut v) => {
            let color = {
                let terrain_graph_read = terrain_graph.read();
                let node = terrain_graph_read.graph().node(graph_id).unwrap();
                theme::category_color(node.category())
            };
            let char_limit = match spec.range {
                ParamRange::StringMaxLength(max_length) => Some(max_length),
                _ => None
            };
            let changed = widgets::text_field(ui, spec.label, color, &mut v, char_limit).changed();
            changed.then_some(NParamValue::String(v))
        }
        NParamValue::Enum(mut v) => {
            let color = {
                let terrain_graph_read = terrain_graph.read();
                let node = terrain_graph_read.graph().node(graph_id).unwrap();
                theme::category_color(node.category())
            };
            let options = match &spec.range {
                ParamRange::EnumOneOf(options) => options.clone(),
                _ => Vec::new()
            };
            let changed = widgets::enum_selector(
                ui,
                (graph_id, spec.key),
                spec.label,
                color,
                &options,
                &mut v
            )
            .changed();
            changed.then_some(NParamValue::Enum(v))
        }
        NParamValue::Action {
            show_success_message
        } => {
            let color = {
                let terrain_graph_read = terrain_graph.read();
                let node = terrain_graph_read.graph().node(graph_id).unwrap();
                theme::category_color(node.category())
            };
            let terrain_graph = terrain_graph.clone();
            let key = spec.key;
            let action_label = spec.label;
            widgets::button(ui, spec.label, color, move || {
                run_node_action(
                    &terrain_graph,
                    graph_id,
                    key,
                    action_label,
                    show_success_message,
                    action_output
                );
            });
            None
        }
    };

    ui.end_row();

    if let Some(value) = new_value {
        let mut terrain_graph = terrain_graph.write();
        let (node_label, result) = {
            let mut node = terrain_graph.graph_mut().node_mut(graph_id).unwrap();
            let node_label = node.label().to_string();
            let result = node.set_param(spec.key, value);
            (node_label, result)
        };
        if let Err(err) = result {
            error!(
                "Failed to set parameter {} of node {}: {}",
                spec.key, node_label, err
            );
            terrain_graph.set_action_result(graph_id, Err(err));
        }
    }
}

fn run_node_action(
    terrain_graph: &TerrainInstanceHolder,
    graph_id: GraphNodeId,
    key: &str,
    action_label: &str,
    show_success_message: bool,
    fallback_output: &[TileHandle]
) {
    let output_size = terrain_graph.read().graph().tile_size();
    let resolution = terrain_graph
        .read()
        .graph()
        .node(graph_id)
        .unwrap()
        .action_resolution(key);

    let gathered;
    let output = match resolution {
        Some(resolution) => {
            gathered = gather_action_input(terrain_graph, graph_id, resolution);
            &gathered
        }
        None => fallback_output
    };

    let mut terrain_graph = terrain_graph.write();
    let (node_label, result) = {
        let mut node = terrain_graph.graph_mut().node_mut(graph_id).unwrap();
        let node_label = node.label().to_string();
        let result = node.on_action(key, output, output_size);
        (node_label, result)
    };
    match result {
        Ok(()) if show_success_message => {
            terrain_graph.set_action_result(graph_id, Ok(format!("{} completed", action_label)));
        }
        Ok(()) => terrain_graph.clear_action_message(graph_id),
        Err(err) => {
            error!("Action '{}' failed on node {}: {}", key, node_label, err);
            terrain_graph.set_action_result(graph_id, Err(err));
        }
    }
}

/// For an action that declares a resolution via [`crate::core::node::Node::action_resolution`]:
/// freshly evaluates whatever feeds the node's socket-0 input at that resolution, independent of
/// the node's own (cheap, preview-resolution) cached output.
fn gather_action_input(
    terrain_graph: &TerrainInstanceHolder,
    graph_id: GraphNodeId,
    resolution: usize
) -> Vec<TileHandle> {
    let terrain_graph_read = terrain_graph.read();
    let graph = terrain_graph_read.graph();
    let Some((from, from_socket)) = graph
        .edges()
        .find_map(|(from, from_socket, to, to_socket)| {
            (to == graph_id && to_socket == 0).then_some((from, from_socket))
        })
    else {
        return Vec::new();
    };
    match graph.evaluate_once(from, resolution) {
        Ok(tiles) => tiles.get(from_socket).cloned().into_iter().collect(),
        Err(err) => {
            trace!(
                "Action input unavailable for node {:?} at resolution {}: {}",
                graph_id, resolution, err
            );
            Vec::new()
        }
    }
}
