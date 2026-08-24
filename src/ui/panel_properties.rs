use wde::prelude::{ui::egui, *};

use crate::{
    TerrainGraphHolder,
    core::{
        graph::GraphNodeId,
        node_parameters::{NParamConstraints, NParamValue},
    },
};

pub fn draw_properties(
    ui: &mut egui::Ui,
    terrain_graph: &TerrainGraphHolder,
    selected_node: Option<GraphNodeId>,
) {
    if let Some(graph_id) = selected_node {
        let label = terrain_graph
            .read()
            .graph()
            .node(graph_id)
            .expect("selected node should exist in the graph")
            .label()
            .to_string();
        ui.label(format!("{} - Properties", label));

        show_node_params(ui, terrain_graph, graph_id);
    } else {
        ui.label("No node selected.");
    }
}

/// Represents the range of values that a parameter can take, used for rendering appropriate UI controls.
enum ParamRange {
    FloatRange(f32, f32),
    IntRange(i32, i32),
    StringMaxLength(usize),
    EnumOneOf(Vec<String>),
    None,
}

/// Draws the UI for editing the parameters of a node.
fn show_node_params(ui: &mut egui::Ui, terrain_graph: &TerrainGraphHolder, graph_id: GraphNodeId) {
    let param_specs = {
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
                    _ => ParamRange::None,
                };
                (desc.key, desc.label, desc.default.clone(), range)
            })
            .collect::<Vec<_>>()
    };
    if param_specs.is_empty() {
        ui.label("No parameters available.");
        return;
    }

    for (key, label, default, range) in param_specs {
        let current = terrain_graph
            .read()
            .graph()
            .node(graph_id)
            .unwrap()
            .get_param(key)
            .unwrap_or(default);

        let new_value = match current {
            NParamValue::Float(mut v) => {
                let range = match range {
                    ParamRange::FloatRange(min, max) => min..=max,
                    _ => f32::MIN..=f32::MAX,
                };
                let changed = ui.add(Slider::new(&mut v, range).text(label)).changed();
                changed.then_some(NParamValue::Float(v))
            }
            NParamValue::Int(mut v) => {
                let range = match range {
                    ParamRange::IntRange(min, max) => min..=max,
                    _ => i32::MIN..=i32::MAX,
                };
                let changed = ui.add(Slider::new(&mut v, range).text(label)).changed();
                changed.then_some(NParamValue::Int(v))
            }
            NParamValue::Bool(mut v) => {
                let changed = ui.add(Checkbox::new(&mut v, label)).changed();
                changed.then_some(NParamValue::Bool(v))
            }
            NParamValue::String(mut v) => {
                ui.label(label);
                let mut edit = TextEdit::singleline(&mut v);
                if let ParamRange::StringMaxLength(max_length) = range {
                    edit = edit.char_limit(max_length);
                }
                let changed = ui.add(edit).changed();
                changed.then_some(NParamValue::String(v))
            }
            NParamValue::Enum(mut v) => {
                let options = match range {
                    ParamRange::EnumOneOf(options) => options,
                    _ => Vec::new(),
                };
                let mut changed = false;
                ComboBox::from_label(label)
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
                changed.then_some(NParamValue::Enum(v))
            }
        };

        if let Some(value) = new_value {
            let mut terrain_graph = terrain_graph.write();
            let mut node = terrain_graph.graph_mut().node_mut(graph_id).unwrap();
            node.set_param(key, value).unwrap_or_else(|err| {
                error!(
                    "Failed to set parameter {} of node {}: {}",
                    key,
                    node.label(),
                    err
                );
            });
        }
    }
}
