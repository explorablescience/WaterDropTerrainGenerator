use egui_snarl::{InPin, NodeId, OutPin, Snarl, ui::{PinInfo, SnarlPin, SnarlStyle, SnarlViewer, SnarlWidget}};
use wde::prelude::{ui::egui};

use crate::{core::node::Node, nodes::{NodeErosion, NodeGeneratorFlat, NodeGeneratorPerlin}};

pub type GraphInstance = Snarl<GraphNode>;
pub enum GraphNode {
    Main(Box<dyn Node>)
}

struct GraphViewer;
impl SnarlViewer<GraphNode> for GraphViewer {
    fn title(&mut self, node: &GraphNode) -> String {
        match node {
            GraphNode::Main(node) => node.label().to_string()
        }
    }
    
    fn show_header(
        &mut self,
        node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut egui::Ui,
        instance: &mut GraphInstance
    ) {
        let label = self.title(&instance[node]);
        ui.label(label);
    }

    fn inputs(&mut self, node: &GraphNode) -> usize {
        match node {
            GraphNode::Main(node) => node.inputs().len()
        }
    }

    fn outputs(&mut self, node: &GraphNode) -> usize {
        match node {
            GraphNode::Main(node) => node.outputs().len()
        }
    }

    fn show_input(&mut self, pin: &InPin, ui: &mut egui::Ui, instance: &mut GraphInstance) -> impl SnarlPin + 'static {
        match &instance[pin.id.node] {
            GraphNode::Main(node) => {
                let pin = &node.inputs()[pin.id.input];
                ui.label(pin.name);
                PinInfo::circle()
            }
        }
    }

    fn show_output(&mut self, pin: &OutPin, ui: &mut egui::Ui, instance: &mut GraphInstance) -> impl SnarlPin + 'static {
        match &instance[pin.id.node] {
            GraphNode::Main(node) => {
                let pin = &node.outputs()[pin.id.output];
                ui.label(pin.name);
                PinInfo::circle()
            }
        }
    }

    fn has_graph_menu(&mut self, _pos: egui::Pos2, _snarl: &mut Snarl<GraphNode>) -> bool {
        true
    }
    fn show_graph_menu(&mut self, pos: egui::Pos2, ui: &mut egui::Ui, snarl: &mut Snarl<GraphNode>) {
        ui.label("Add Node");

        ui.menu_button("Generator", |ui| {
            if ui.button("Flat Generator").clicked() {
                snarl.insert_node(pos, GraphNode::Main(Box::new(NodeGeneratorFlat)));
                ui.close();
            }
            if ui.button("Perlin Generator").clicked() {
                snarl.insert_node(pos, GraphNode::Main(Box::new(NodeGeneratorPerlin::default())));
                ui.close();
            }
        });

        ui.menu_button("Simulation", |ui| {
            if ui.button("Erosion").clicked() {
                snarl.insert_node(pos, GraphNode::Main(Box::new(NodeErosion::default())));
                ui.close();
            }
        });
    }
}

/// Displays the graph editor using egui-snarl.
pub fn show_graph(id: egui::Id, ui: &mut egui::Ui, graph_instance: &mut GraphInstance) {
    let style = SnarlStyle::default();
    SnarlWidget::new()
        .id(id)
        .style(style)
        .show(graph_instance, &mut GraphViewer {}, ui);
}