use egui_snarl::{
    InPin, NodeId, OutPin, Snarl,
    ui::{PinInfo, SnarlPin, SnarlStyle, SnarlViewer, SnarlWidget},
};
use wde::prelude::{ui::egui, *};

use crate::{
    TerrainGraphHolder,
    core::{graph::GraphNodeId, node::Node},
    nodes::{NodeErosion, NodeGeneratorFlat, NodeGeneratorPerlin},
};

pub type GraphInstance = Snarl<GraphNode>;
pub enum GraphNode {
    /// Represents a node in the graph that corresponds to a node in the underlying `NodeGraph`.
    Main(GraphNodeId),
}

/// Identifies a node both in the `egui-snarl` UI graph and in the underlying `NodeGraph`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedNode {
    pub snarl_id: NodeId,
    pub graph_id: GraphNodeId,
}

/// Represents the viewer for the graph editor, which manages the interaction between the UI and the underlying terrain graph.
struct GraphViewer {
    selected: Option<SelectedNode>,
    terrain_graph: TerrainGraphHolder,
}
impl SnarlViewer<GraphNode> for GraphViewer {
    fn title(&mut self, node: &GraphNode) -> String {
        let GraphNode::Main(graph_id) = node;
        self.terrain_graph
            .read()
            .graph()
            .node(*graph_id)
            .map(|n| n.label().to_string())
            .unwrap_or_default()
    }
    fn inputs(&mut self, node: &GraphNode) -> usize {
        let GraphNode::Main(graph_id) = node;
        self.terrain_graph
            .read()
            .graph()
            .node(*graph_id)
            .map(|n| n.inputs().len())
            .unwrap_or(0)
    }
    fn outputs(&mut self, node: &GraphNode) -> usize {
        let GraphNode::Main(graph_id) = node;
        self.terrain_graph
            .read()
            .graph()
            .node(*graph_id)
            .map(|n| n.outputs().len())
            .unwrap_or(0)
    }

    fn connect(&mut self, from: &OutPin, to: &InPin, snarl: &mut Snarl<GraphNode>) {
        // Connects pins in terrain graph
        let GraphNode::Main(from_graph_id) = &snarl[from.id.node];
        let GraphNode::Main(to_graph_id) = &snarl[to.id.node];
        if self
            .terrain_graph
            .write()
            .graph_mut()
            .connect(*from_graph_id, from.id.output, *to_graph_id, to.id.input)
            .is_err()
        {
            error!("Failed to connect nodes.");
            return;
        }

        // Connects pins in ui
        snarl.connect(from.id, to.id);
    }
    fn disconnect(&mut self, from: &OutPin, to: &InPin, snarl: &mut Snarl<GraphNode>) {
        let GraphNode::Main(from_graph_id) = &snarl[from.id.node];
        let GraphNode::Main(to_graph_id) = &snarl[to.id.node];
        if self
            .terrain_graph
            .write()
            .graph_mut()
            .disconnect(*from_graph_id, from.id.output, *to_graph_id, to.id.input)
            .is_err()
        {
            error!("Failed to disconnect nodes.");
            return;
        }
        snarl.disconnect(from.id, to.id);
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
        let GraphNode::Main(graph_id) = &instance[pin.id.node];
        let terrain_graph = self.terrain_graph.read();
        let node = terrain_graph
            .graph()
            .node(*graph_id)
            .expect("selected node should exist in the graph");
        let socket = &node.inputs()[pin.id.input];
        ui.label(socket.name);
        PinInfo::circle()
    }
    fn show_output(
        &mut self,
        pin: &OutPin,
        ui: &mut egui::Ui,
        instance: &mut GraphInstance,
    ) -> impl SnarlPin + 'static {
        let GraphNode::Main(graph_id) = &instance[pin.id.node];
        let terrain_graph = self.terrain_graph.read();
        let node = terrain_graph
            .graph()
            .node(*graph_id)
            .expect("selected node should exist in the graph");
        let socket = &node.outputs()[pin.id.output];
        ui.label(socket.name);
        PinInfo::circle()
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
                self.new_node(pos, snarl, Box::new(NodeGeneratorFlat));
                ui.close();
            }
            if ui.button("Perlin Generator").clicked() {
                self.new_node(pos, snarl, Box::new(NodeGeneratorPerlin::default()));
                ui.close();
            }
        });

        ui.menu_button("Simulation", |ui| {
            if ui.button("Erosion").clicked() {
                self.new_node(pos, snarl, Box::new(NodeErosion::default()));
                ui.close();
            }
        });
    }

    fn has_node_menu(&mut self, _node: &GraphNode) -> bool {
        true
    }
    fn show_node_menu(
        &mut self,
        node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut egui::Ui,
        snarl: &mut Snarl<GraphNode>,
    ) {
        if ui.button("Remove Node").clicked() {
            let GraphNode::Main(graph_id) = &snarl[node];
            if self
                .terrain_graph
                .write()
                .graph_mut()
                .remove_node(*graph_id)
                .is_ok()
            {
                snarl.remove_node(node);
                if self
                    .selected
                    .is_some_and(|selected| selected.snarl_id == node)
                {
                    self.selected = None;
                }
            } else {
                error!("Failed to remove node.");
            }
        }
    }

    fn node_frame(
        &mut self,
        default: egui::Frame,
        node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        _snarl: &Snarl<GraphNode>,
    ) -> egui::Frame {
        if self
            .selected
            .is_some_and(|selected| selected.snarl_id == node)
        {
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
        snarl: &mut Snarl<GraphNode>,
    ) {
        let to_global = ui
            .ctx()
            .layer_transform_to_global(ui.layer_id())
            .unwrap_or_default();
        let screen_rect = to_global * rect;

        let clicked_inside = ui.input(|i| {
            i.pointer.primary_released()
                && i.pointer
                    .interact_pos()
                    .is_some_and(|pos| screen_rect.contains(pos))
        });

        if clicked_inside {
            let GraphNode::Main(graph_id) = &snarl[node];
            self.selected = Some(SelectedNode {
                snarl_id: node,
                graph_id: *graph_id,
            });
        }
    }
}
impl GraphViewer {
    fn new_node(&mut self, pos: egui::Pos2, snarl: &mut Snarl<GraphNode>, node: Box<dyn Node>) {
        let graph_id = self.terrain_graph.write().graph_mut().add_node(node);
        let snarl_id = snarl.insert_node(pos, GraphNode::Main(graph_id));
        self.selected = Some(SelectedNode { snarl_id, graph_id });
    }
}

/// Displays the graph editor using egui-snarl.
///
/// Returns the id of the node currently selected by the user, if any.
pub fn show_graph(
    id: egui::Id,
    ui: &mut egui::Ui,
    graph_instance: &mut GraphInstance,
    terrain_graph: TerrainGraphHolder,
) -> Option<SelectedNode> {
    let style = SnarlStyle::default();

    // Initialize the viewer with the currently selected node, if any.
    let selected_node_id = egui::Id::new("panel-graph-selected-node");
    let mut viewer = GraphViewer {
        selected: ui
            .ctx()
            .data(|d| d.get_temp::<SelectedNode>(selected_node_id)),
        terrain_graph,
    };

    // Show the graph editor
    SnarlWidget::new()
        .id(id)
        .style(style)
        .show(graph_instance, &mut viewer, ui);

    // Update the selected node in the egui context so it can be retrieved later
    match viewer.selected {
        Some(node) => ui.ctx().data_mut(|d| d.insert_temp(selected_node_id, node)),
        None => ui
            .ctx()
            .data_mut(|d| d.remove::<SelectedNode>(selected_node_id)),
    }
    viewer.selected
}
