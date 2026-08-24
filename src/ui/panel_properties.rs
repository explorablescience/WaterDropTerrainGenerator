use wde::prelude::{ui::egui, *};

use crate::{
    core::{
        node::Node,
        node_parameters::{NParamConstraints, NParamValue},
    },
    ui::panel_graph::{GraphInstance, GraphNode},
};
use egui_snarl::NodeId;

pub fn draw_properties(
    ui: &mut egui::Ui,
    graph_instance: &mut GraphInstance,
    selected_node: Option<NodeId>,
) {
    if let Some(node_id) = selected_node {
        let node = &mut graph_instance[node_id];
        let GraphNode::Main(node) = node;
        ui.label(format!("{} - Properties", node.label()));

        show_node_params(ui, node);
    } else {
        ui.label("No node selected.");
    }
}

/// Represents the range of values that a parameter can take, used for rendering appropriate UI controls.
enum ParamRange {
    FloatRange(f32, f32),
    IntRange(i32, i32),
    StringMaxLength(usize),
    EnumOneOf(Vec<&'static str>),
    None,
}

/// Draws the UI for editing the parameters of a node.
fn show_node_params(ui: &mut egui::Ui, node: &mut Box<dyn Node + 'static>) {
    if node.desc_params().is_empty() {
        ui.label("No parameters available.");
        return;
    }

    for i in 0..node.desc_params().len() {
        let (key, label, current, range) = {
            let desc = &node.desc_params()[i];
            let current = node
                .get_param(desc.key)
                .unwrap_or_else(|| desc.default.clone());
            let range = match &desc.constraints {
                Some(NParamConstraints::FloatRange { min, max }) => {
                    ParamRange::FloatRange(*min, *max)
                }
                Some(NParamConstraints::IntRange { min, max }) => ParamRange::IntRange(*min, *max),
                Some(NParamConstraints::StringMaxLength { max_length }) => {
                    ParamRange::StringMaxLength(*max_length)
                }
                Some(NParamConstraints::EnumOneOf { options }) => {
                    ParamRange::EnumOneOf(options.clone())
                }
                _ => ParamRange::None,
            };
            (desc.key, desc.label, current, range)
        };

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
                                .selectable_value(&mut v, option.to_string(), option)
                                .clicked()
                            {
                                changed = true;
                            }
                        }
                    });
                changed.then_some(NParamValue::Enum(v))
            }
        };

        if let Some(value) = new_value
            && let Err(err) = node.set_param(key, value)
        {
            error!("Failed to set parameter {} of node {}: {}", key, node.label(), err);
        }
    }
}
