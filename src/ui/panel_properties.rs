use wde::prelude::{ui::egui, *};

use crate::{
    TerrainGraphHolder,
    core::{
        graph::GraphNodeId,
        node_parameters::{NParamConstraints, NParamValue}
    },
    ui::theme
};

pub fn draw_properties(
    ui: &mut egui::Ui,
    terrain_graph: &TerrainGraphHolder,
    selected_node: Option<GraphNodeId>
) {
    egui::Frame::NONE
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            if let Some(graph_id) = selected_node {
                let label = terrain_graph
                    .read()
                    .graph()
                    .node(graph_id)
                    .expect("selected node should exist in the graph")
                    .label()
                    .to_string();
                ui.label(
                    egui::RichText::new(label)
                        .font(theme::heading_font(13.5))
                        .color(theme::palette::TEXT)
                );
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(10.0);

                show_node_params(ui, terrain_graph, graph_id);
            } else {
                ui.label(
                    egui::RichText::new("No node selected.").color(theme::palette::TEXT_MUTED)
                );
            }
        });
}

/// Represents the range of values that a parameter can take, used for rendering appropriate UI controls.
enum ParamRange {
    FloatRange(f32, f32),
    IntRange(i32, i32),
    StringMaxLength(usize),
    EnumOneOf(Vec<String>),
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
fn show_node_params(ui: &mut egui::Ui, terrain_graph: &TerrainGraphHolder, graph_id: GraphNodeId) {
    let param_specs: Vec<ParamSpec> = {
        let terrain_graph_read = terrain_graph.read();
        let node = terrain_graph_read.graph().node(graph_id).unwrap();
        node.desc_params()
            .iter()
            .map(|desc| {
                let range = match &desc.constraints {
                    Some(NParamConstraints::FloatRange { min, max }) => {
                        ParamRange::FloatRange(*min, *max)
                    }
                    Some(NParamConstraints::IntRange { min, max }) => {
                        ParamRange::IntRange(*min, *max)
                    }
                    Some(NParamConstraints::StringMaxLength { max_length }) => {
                        ParamRange::StringMaxLength(*max_length)
                    }
                    Some(NParamConstraints::EnumOneOf { options }) => {
                        ParamRange::EnumOneOf(options.iter().map(ToString::to_string).collect())
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
        ui.label(egui::RichText::new("No parameters available.").color(theme::palette::TEXT_MUTED));
        return;
    }

    // Group parameters by category, preserving the order in which each category first appears.
    let mut categories: Vec<(&'static str, Vec<usize>)> = Vec::new();
    for (i, spec) in param_specs.iter().enumerate() {
        match categories.iter_mut().find(|(c, _)| *c == spec.category) {
            Some((_, indices)) => indices.push(i),
            None => categories.push((spec.category, vec![i]))
        }
    }

    for (card_index, (category, indices)) in categories.iter().enumerate() {
        if card_index > 0 {
            ui.add_space(10.0);
        }

        egui::CollapsingHeader::new(
            egui::RichText::new(*category)
                .font(theme::heading_font(13.0))
                .color(theme::palette::TEXT)
        )
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
    }
}

/// Draws one parameter as a `label | control` grid row and, if the control was edited, writes
/// the new value back into the graph.
fn show_param_row(
    ui: &mut egui::Ui,
    terrain_graph: &TerrainGraphHolder,
    graph_id: GraphNodeId,
    spec: &ParamSpec
) {
    let current = terrain_graph
        .read()
        .graph()
        .node(graph_id)
        .unwrap()
        .get_param(spec.key)
        .unwrap_or_else(|| spec.default.clone());

    // Values edited away from their default are called out in amber, mirroring the convention
    // that a parameter's label color tells you at a glance whether it still holds its default.
    let is_modified = current != spec.default;
    let label_color = if is_modified {
        theme::palette::MODIFIED
    } else {
        theme::palette::TEXT_MUTED
    };
    ui.label(egui::RichText::new(spec.label).color(label_color));

    let new_value = match current {
        NParamValue::Float(mut v) => {
            let range = match spec.range {
                ParamRange::FloatRange(min, max) => min..=max,
                _ => f32::MIN..=f32::MAX
            };
            let changed = ui.add(Slider::new(&mut v, range)).changed();
            changed.then_some(NParamValue::Float(v))
        }
        NParamValue::Int(mut v) => {
            let range = match spec.range {
                ParamRange::IntRange(min, max) => min..=max,
                _ => i32::MIN..=i32::MAX
            };
            let changed = ui.add(Slider::new(&mut v, range)).changed();
            changed.then_some(NParamValue::Int(v))
        }
        NParamValue::Bool(mut v) => {
            let changed = theme::widgets::toggle_switch(ui, &mut v).changed();
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
                _ => Vec::new()
            };
            let mut changed = false;
            if !options.is_empty() && options.len() <= 3 {
                // A small, fixed option set reads better as a segmented row of buttons than as
                // a dropdown that hides the choices behind a click.
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
