use egui_snarl::{
    InPin, NodeId, OutPin, Snarl,
    ui::{PinInfo, SnarlPin, SnarlStyle, SnarlViewer, SnarlWidget},
};
use wde::prelude::ui::egui;

use crate::{
    core::node::Node,
    nodes::{NodeErosion, NodeGeneratorFlat, NodeGeneratorPerlin},
};

pub type GraphInstance = Snarl<GraphNode>;
pub enum GraphNode {
    Main(Box<dyn Node>),
}

/// Tracks which node is currently selected. `egui-snarl`'s own selection
/// only reacts to Shift/Cmd-modified clicks or a drag-rectangle, so we
/// drive selection ourselves from a plain click on the node's header.
struct GraphViewer {
    selected: Option<NodeId>,
}
impl SnarlViewer<GraphNode> for GraphViewer {
    fn title(&mut self, node: &GraphNode) -> String {
        match node {
            GraphNode::Main(node) => node.label().to_string(),
        }
    }
    fn inputs(&mut self, node: &GraphNode) -> usize {
        match node {
            GraphNode::Main(node) => node.inputs().len(),
        }
    }
    fn outputs(&mut self, node: &GraphNode) -> usize {
        match node {
            GraphNode::Main(node) => node.outputs().len(),
        }
    }


    fn show_header(
        &mut self,
        node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut egui::Ui,
        instance: &mut GraphInstance,
    ) {
        let label = self.title(&instance[node]);
        ui.label(label);
    }
    fn show_input(
        &mut self,
        pin: &InPin,
        ui: &mut egui::Ui,
        instance: &mut GraphInstance,
    ) -> impl SnarlPin + 'static {
        match &instance[pin.id.node] {
            GraphNode::Main(node) => {
                let pin = &node.inputs()[pin.id.input];
                ui.label(pin.name);
                PinInfo::circle()
            }
        }
    }
    fn show_output(
        &mut self,
        pin: &OutPin,
        ui: &mut egui::Ui,
        instance: &mut GraphInstance,
    ) -> impl SnarlPin + 'static {
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
    fn show_graph_menu(
        &mut self,
        pos: egui::Pos2,
        ui: &mut egui::Ui,
        snarl: &mut Snarl<GraphNode>,
    ) {
        ui.label("Add Node");

        ui.menu_button("Generator", |ui| {
            if ui.button("Flat Generator").clicked() {
                snarl.insert_node(pos, GraphNode::Main(Box::new(NodeGeneratorFlat)));
                ui.close();
            }
            if ui.button("Perlin Generator").clicked() {
                snarl.insert_node(
                    pos,
                    GraphNode::Main(Box::new(NodeGeneratorPerlin::default())),
                );
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



    fn node_frame(
        &mut self,
        default: egui::Frame,
        node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        _snarl: &Snarl<GraphNode>,
    ) -> egui::Frame {
        if self.selected == Some(node) {
            default.stroke(egui::Stroke::new(2.0, egui::Color32::from_rgb(200, 40, 40)))
        } else {
            default
        }
    }
    
    fn final_node_rect(
        &mut self,
        node: NodeId,
        rect: egui::Rect,
        ui: &mut egui::Ui,
        _snarl: &mut Snarl<GraphNode>,
    ) {
        // Detect clicks on the node to select it
        if ui.input(|i| i.pointer.any_click())
            && rect.contains(ui.input(|i| i.pointer.hover_pos().unwrap_or_default()))
        {
            self.selected = Some(node);
        }
    }
}

/// Displays the graph editor using egui-snarl.
///
/// Returns the id of the node currently selected by the user, if any.
pub fn show_graph(
    id: egui::Id,
    ui: &mut egui::Ui,
    graph_instance: &mut GraphInstance,
) -> Option<NodeId> {
    let style = SnarlStyle::default();

    // Initialize the viewer with the currently selected node, if any
    let selected_node_id = id.with("selected-node");
    let mut viewer = GraphViewer {
        selected: ui.ctx().data(|d| d.get_temp::<NodeId>(selected_node_id)),
    };

    // Show the graph editor
    SnarlWidget::new()
        .id(id)
        .style(style)
        .show(graph_instance, &mut viewer, ui);

    // Update the selected node in the egui context so it can be retrieved later
    match viewer.selected {
        Some(node) => ui.ctx().data_mut(|d| d.insert_temp(selected_node_id, node)),
        None => ui.ctx().data_mut(|d| d.remove::<NodeId>(selected_node_id)),
    }
    viewer.selected
}
