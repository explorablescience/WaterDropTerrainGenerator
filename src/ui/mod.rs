use wde::prelude::*;
use wde::prelude::ui::egui::Ui as EguiUi;
use bevy::prelude::*;

use crate::TerrainGraph;
use crate::core::graph::NodeId;
use crate::core::node_parameters::{NParamConstraints, NParamValue};

mod dock;
mod snarl_demo;
use dock::EditorDockPlugin;

pub struct UIPlugin;
impl Plugin for UIPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EditorDockPlugin)
            .add_systems(Update, show_ui);
    }
}

/// Owned snapshot of a [`NParamConstraints`], since the original may hold a non-`Clone`
/// custom validator and can't outlive the node borrow it's read from.
enum ParamRange {
    FloatRange(f32, f32),
    IntRange(i32, i32),
    StringMaxLength(usize),
    EnumOneOf(Vec<&'static str>),
    None,
}

fn show_ui(
    ctx: Res<UIContext>,
    mut terrain_graph: ResMut<TerrainGraph>,
    mut selected_node: Local<Option<NodeId>>,
    mut tile_size: Local<usize>,
) {
    if *tile_size == 0 {
        *tile_size = 128;
    }

    let node_labels = terrain_graph.graph().get_nodes_labels();
    if selected_node.is_none() {
        *selected_node = node_labels.last().map(|(id, _)| *id);
    }

    UIWindow::new("WaterDrop Terrain Editor")
        .default_size((400.0, 300.0))
        .show(&ctx.0, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                for (node_id, label) in &node_labels {
                    CollapsingHeader::new(label.as_str())
                        .default_open(true)
                        .show(ui, |ui| {
                            show_node_params(ui, &mut terrain_graph, *node_id);
                        });
                }
            });

            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Output node:");
                let selected_label = selected_node
                    .and_then(|id| node_labels.iter().find(|(nid, _)| *nid == id))
                    .map(|(_, label)| label.as_str())
                    .unwrap_or("<none>");
                ComboBox::from_label("")
                    .selected_text(selected_label)
                    .show_ui(ui, |ui| {
                        for (node_id, label) in &node_labels {
                            ui.selectable_value(&mut *selected_node, Some(*node_id), label);
                        }
                    });
            });

            ui.horizontal(|ui| {
                ui.label("Tile size:");
                ui.add(DragValue::new(&mut *tile_size).range(1..=4096));
            });

            ui.add_enabled_ui(selected_node.is_some(), |ui| {
                if ui.button("Reprocess").clicked() {
                    if let Some(node_id) = *selected_node {
                        if let Err(err) = terrain_graph.process(node_id, *tile_size) {
                            error!("Failed to process terrain graph: {}", err);
                        }
                    }
                }
            });
        });
}

/// Draws an editable row for every parameter of `node_id`, writing changes straight back
/// into the graph so the next "Reprocess" click picks them up.
fn show_node_params(ui: &mut EguiUi, terrain_graph: &mut TerrainGraph, node_id: NodeId) {
    let param_count = match terrain_graph.graph().node(node_id) {
        Ok(node) => node.desc_params().len(),
        Err(_) => return,
    };

    for i in 0..param_count {
        let (key, label, current, range) = {
            let node = terrain_graph.graph().node(node_id).expect("checked above");
            let desc = &node.desc_params()[i];
            let current = node
                .get_param(desc.key)
                .unwrap_or_else(|| desc.default.clone());
            let range = match &desc.constraints {
                Some(NParamConstraints::FloatRange { min, max }) => ParamRange::FloatRange(*min, *max),
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
                            if ui.selectable_value(&mut v, option.to_string(), option).clicked() {
                                changed = true;
                            }
                        }
                    });
                changed.then_some(NParamValue::Enum(v))
            }
        };

        if let Some(value) = new_value {
            if let Ok(node) = terrain_graph.graph_mut().node_mut(node_id) {
                if let Err(err) = node.set_param(key, value) {
                    error!("Failed to set parameter '{}': {}", key, err);
                }
            }
        }
    }
}
