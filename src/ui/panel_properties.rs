use wde::prelude::{ui::egui, *};

use crate::{
    TerrainGraphHolder, core::{
        graph::GraphNodeId,
        node_parameters::{NParamConstraints, NParamValue},
    }, ui::{theme, widgets},
};

pub fn draw_properties(
    ui: &mut egui::Ui,
    terrain_graph: &TerrainGraphHolder,
    selected_node: Option<GraphNodeId>,
) {
    egui::Frame::NONE
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            if selected_node.is_none() {
                return;
            }
            let graph_id = selected_node.unwrap();

            // Title bar
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
                                egui::Sense::hover(),
                            );
                            widgets::paint_node_icon(ui.painter(), rect, icon, color);
                            ui.label(
                                egui::RichText::new(label)
                                    .font(theme::heading_font(theme::fonts::FONT_SIZE_TITLE))
                                    .color(theme::palette::TEXT),
                            );
                        },
                        |ui| {
                            egui::Frame::new()
                                .fill(color.gamma_multiply(0.18))
                                .corner_radius(egui::CornerRadius::same(theme::layout::CHIP_ROUNDING))
                                .inner_margin(egui::Margin::symmetric(8, 3))
                                .show(ui, |ui| {
                                    ui.label(
                                        egui::RichText::new(category.display_name())
                                            .font(theme::heading_font(
                                                theme::fonts::FONT_SIZE_SMALL,
                                            ))
                                            .color(color),
                                    );
                                });
                        },
                    );
                });

            show_node_params(ui, terrain_graph, graph_id);
        });
}

/// Represents the range of values that a parameter can take, used for rendering appropriate UI controls.
/// Float/Int parameters read their range straight from `NParamDesc::constraints` inside
/// [`widgets::param_number_field`] instead, since that widget takes the descriptor directly.
enum ParamRange {
    StringMaxLength(usize),
    EnumOneOf(Vec<String>),
    None,
}

struct ParamSpec {
    key: &'static str,
    label: &'static str,
    category: &'static str,
    default: NParamValue,
    range: ParamRange,
}

/// Draws the UI for editing the parameters of a node, grouped into cards by [`NParamDesc::category`].
fn show_node_params(ui: &mut egui::Ui, terrain_graph: &TerrainGraphHolder, graph_id: GraphNodeId) {
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
                    _ => ParamRange::None,
                };
                ParamSpec {
                    key: desc.key,
                    label: desc.label,
                    category: desc.category,
                    default: desc.default.clone(),
                    range,
                }
            })
            .collect()
    };
    if param_specs.is_empty() {
        return;
    }

    // Group parameters by category, preserving the order in which each category first appears.
    let mut categories: Vec<(&'static str, Vec<usize>)> = Vec::new();
    for (i, spec) in param_specs.iter().enumerate() {
        match categories.iter_mut().find(|(c, _)| *c == spec.category) {
            Some((_, indices)) => indices.push(i),
            None => categories.push((spec.category, vec![i])),
        }
    }

    // Draw each category as a collapsible card with a grid of parameter rows.
    for (category, indices) in categories.iter() {
        ui.add_space(6.0);

        egui::Frame::new()
            .fill(theme::palette::BG_WIDGET)
            .inner_margin(egui::Margin::symmetric(12, 10))
            .corner_radius(egui::CornerRadius::same(theme::layout::CARD_ROUNDING))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());

                egui::CollapsingHeader::new(
                    egui::RichText::new(format!("  {}", category))
                        .font(theme::heading_font(theme::fonts::FONT_SIZE_HEADING))
                        .color(theme::palette::TEXT_DISABLED),
                )
                .icon(widgets::menu_icon)
                .default_open(true)
                .show(ui, |ui| {
                    ui.add_space(2.0);
                    egui::Grid::new(("param-grid", graph_id, *category))
                        .num_columns(2)
                        .spacing([10.0, 8.0])
                        .striped(false)
                        .show(ui, |ui| {
                            for &i in indices {
                                show_param_row(ui, terrain_graph, graph_id, &param_specs[i]);
                            }
                        });
                });
            });
    }
}

/// Draws one parameter as a `label | control` grid row and, if the control was edited, writes
/// the new value back into the graph.
fn show_param_row(
    ui: &mut egui::Ui,
    terrain_graph: &TerrainGraphHolder,
    graph_id: GraphNodeId,
    spec: &ParamSpec,
) {
    let current = terrain_graph
        .read()
        .graph()
        .node(graph_id)
        .unwrap()
        .get_param(spec.key)
        .unwrap_or_else(|| spec.default.clone());

    ui.label(egui::RichText::new(spec.label).color(theme::palette::TEXT_MUTED));

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
            let changed = widgets::param_number_field(ui, desc, color, &mut v).changed();
            changed.then_some(NParamValue::Float(v))
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
            let changed = widgets::param_number_field(ui, desc, color, &mut v_float).changed();
            changed.then_some(NParamValue::Int(v_float.round() as i32))
        }
        NParamValue::Bool(mut v) => {
            let changed = widgets::toggle_switch(ui, &mut v).changed();
            changed.then_some(NParamValue::Bool(v))
        }
        NParamValue::String(mut v) => {
            let mut edit = TextEdit::singleline(&mut v);
            if let ParamRange::StringMaxLength(max_length) = spec.range {
                edit = edit.char_limit(max_length);
            }
            let changed = ui.add(edit).changed();
            changed.then_some(NParamValue::String(v))
        }
        NParamValue::Enum(mut v) => {
            let options = match &spec.range {
                ParamRange::EnumOneOf(options) => options.clone(),
                _ => Vec::new(),
            };
            let mut changed = false;
            if !options.is_empty() && options.len() <= 3 {
                ui.horizontal(|ui| {
                    for option in &options {
                        let selected = *option == v;
                        if ui.selectable_label(selected, option).clicked() && !selected {
                            v = option.clone();
                            changed = true;
                        }
                    }
                });
            } else {
                ComboBox::from_id_salt((graph_id, spec.key))
                    .selected_text(v.clone())
                    .show_ui(ui, |ui| {
                        for option in options {
                            if ui
                                .selectable_value(&mut v, option.clone(), option)
                                .clicked()
                            {
                                changed = true;
                            }
                        }
                    });
            }
            changed.then_some(NParamValue::Enum(v))
        }
    };

    ui.end_row();

    if let Some(value) = new_value {
        let mut terrain_graph = terrain_graph.write();
        let mut node = terrain_graph.graph_mut().node_mut(graph_id).unwrap();
        node.set_param(spec.key, value).unwrap_or_else(|err| {
            error!(
                "Failed to set parameter {} of node {}: {}",
                spec.key,
                node.label(),
                err
            );
        });
    }
}
