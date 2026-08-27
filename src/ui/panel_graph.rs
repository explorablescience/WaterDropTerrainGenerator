use egui_snarl::{
    InPin, NodeId, OutPin, Snarl,
    ui::{
        BackgroundPattern, NodeLayout, PinInfo, PinPlacement, PinShape, SnarlPin, SnarlStyle,
        SnarlViewer, SnarlWidget, WireLayer, WireStyle
    }
};
use wde::prelude::{ui::egui, *};

use crate::{
    TerrainSessionHolder,
    core::{
        graph::GraphNodeId,
        node::{self, Node, NodeCategory, NodeError, NodeIcon}
    },
    ui::{
        theme::{self, palette::BG_GRAPH},
        widgets
    }
};

pub type GraphInstance = Snarl<GraphNode>;
pub enum GraphNode {
    Main(GraphNodeId)
}

/// Identifies a node both in the `egui-snarl` UI graph and in the underlying `NodeGraph`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedNode {
    pub snarl_id: NodeId,
    pub graph_id: GraphNodeId
}

struct GraphViewer {
    selected: Option<SelectedNode>,
    terrain_graph: TerrainSessionHolder
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
        // An input socket can only hold one connection, so this replaces whatever was previously plugged into `to`.
        let GraphNode::Main(from_graph_id) = &snarl[from.id.node];
        let GraphNode::Main(to_graph_id) = &snarl[to.id.node];
        if let Err(e) = self.terrain_graph.write().graph_mut().connect(
            *from_graph_id,
            from.id.output,
            *to_graph_id,
            to.id.input
        ) {
            match e {
                NodeError::SocketTypeMismatch { .. } => {
                    warn!("Cannot connect pins: socket types don't match.");
                }
                _ => error!("Failed to connect nodes: {}", e)
            }
            return;
        }

        // Mirrors the replacement above on the ui side, then connects pins in ui.
        for &remote in &to.remotes {
            snarl.disconnect(remote, to.id);
        }
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
        instance: &mut GraphInstance
    ) {
        let label = self.title(&instance[node]);
        let Some(category) = self.node_category(&instance[node]) else {
            return;
        };
        let icon = self.node_icon(&instance[node]);
        let color = theme::category_color(category);

        ui.horizontal(|ui| {
            if let Some(icon) = icon {
                let (rect, _) = ui.allocate_exact_size(
                    egui::Vec2::splat(theme::fonts::FONT_SIZE_NODE_TITLE),
                    egui::Sense::hover()
                );
                widgets::paint_node_icon(ui, rect, icon, color);
            }
            ui.label(
                egui::RichText::new(label)
                    .color(color)
                    .font(theme::heading_font(theme::fonts::FONT_SIZE_NODE_TITLE))
            );
        });
    }
    fn show_input(
        &mut self,
        pin: &InPin,
        ui: &mut egui::Ui,
        instance: &mut GraphInstance
    ) -> impl SnarlPin + 'static {
        let GraphNode::Main(graph_id) = &instance[pin.id.node];
        let terrain_graph = self.terrain_graph.read();
        let node = terrain_graph
            .graph()
            .node(*graph_id)
            .expect("selected node should exist in the graph");
        let socket = &node.inputs()[pin.id.input];
        let connected = !pin.remotes.is_empty();
        show_pin_label(ui, socket.name, connected);
        let factor = if connected { 1.2 } else { 0.6 };
        PinInfo::circle()
            .with_stroke(egui::Stroke::NONE)
            .with_fill(theme::category_color(node.category()).gamma_multiply(factor))
    }
    fn show_output(
        &mut self,
        pin: &OutPin,
        ui: &mut egui::Ui,
        instance: &mut GraphInstance
    ) -> impl SnarlPin + 'static {
        let GraphNode::Main(graph_id) = &instance[pin.id.node];
        let terrain_graph = self.terrain_graph.read();
        let node = terrain_graph
            .graph()
            .node(*graph_id)
            .expect("selected node should exist in the graph");
        let socket = &node.outputs()[pin.id.output];
        let connected = !pin.remotes.is_empty();
        show_pin_label(ui, socket.name, connected);
        let factor = if connected { 1.2 } else { 0.6 };
        PinInfo::circle()
            .with_stroke(egui::Stroke::NONE)
            .with_fill(theme::category_color(node.category()).gamma_multiply(factor))
    }

    fn has_graph_menu(&mut self, _pos: egui::Pos2, _snarl: &mut Snarl<GraphNode>) -> bool {
        true
    }
    fn show_graph_menu(
        &mut self,
        pos: egui::Pos2,
        ui: &mut egui::Ui,
        snarl: &mut Snarl<GraphNode>
    ) {
        // Match the styling of the top menu bar so this popup menu doesn't look inconsistent.
        ui.set_style(theme::menu_style());

        ui.label(
            egui::RichText::new("Add Node")
                .color(theme::palette::TEXT_MUTED)
                .font(theme::heading_font(theme::fonts::FONT_SIZE_SMALL))
        );
        ui.add_space(2.0);
        ui.separator();
        ui.add_space(2.0);

        // Every node type registers itself with `inventory::submit!` (see `node_registry`).
        for category in NodeCategory::ALL {
            let nodes: Vec<_> = node::registered_nodes()
                .filter(|descriptor| descriptor.category == category)
                .collect();
            if nodes.is_empty() {
                continue;
            }

            let color = theme::category_color(category);
            ui.menu_button(
                egui::RichText::new(category.display_name())
                    .color(color)
                    .strong(),
                |ui| {
                    for descriptor in &nodes {
                        let icon = widgets::node_icon_image(descriptor.icon, color);
                        let button = egui::Button::image_and_text(icon, descriptor.label)
                            .wrap_mode(egui::TextWrapMode::Extend);
                        if ui.add(button).clicked() {
                            self.new_node(pos, snarl, (descriptor.factory)());
                            ui.close();
                        }
                    }
                }
            );
        }
    }

    fn node_frame(
        &mut self,
        default: egui::Frame,
        node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        snarl: &Snarl<GraphNode>
    ) -> egui::Frame {
        let color = self
            .node_category(&snarl[node])
            .map(theme::category_color)
            .unwrap_or(theme::palette::NODE_SELECTED);
        self.selection_frame(default, node, color)
    }
    fn header_frame(
        &mut self,
        default: egui::Frame,
        _node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        _snarl: &Snarl<GraphNode>
    ) -> egui::Frame {
        default.stroke(egui::Stroke::NONE)
    }

    fn final_node_rect(
        &mut self,
        node: NodeId,
        rect: egui::Rect,
        ui: &mut egui::Ui,
        snarl: &mut Snarl<GraphNode>
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
                graph_id: *graph_id
            });
        }
    }
}
impl GraphViewer {
    /// Applies the selection highlight stroke only when `node` is selected.
    fn selection_frame(
        &self,
        default: egui::Frame,
        node: NodeId,
        color: egui::Color32
    ) -> egui::Frame {
        if self
            .selected
            .is_some_and(|selected| selected.snarl_id == node)
        {
            default.stroke(egui::Stroke::new(2.5, color))
        } else {
            default
        }
    }

    /// Looks up the category of the underlying terrain-graph node, if it still exists.
    fn node_category(&self, node: &GraphNode) -> Option<NodeCategory> {
        let GraphNode::Main(graph_id) = node;
        self.terrain_graph
            .read()
            .graph()
            .node(*graph_id)
            .ok()
            .map(|n| n.category())
    }
    /// Looks up the icon of the underlying terrain-graph node, if it still exists.
    fn node_icon(&self, node: &GraphNode) -> Option<NodeIcon> {
        let GraphNode::Main(graph_id) = node;
        self.terrain_graph
            .read()
            .graph()
            .node(*graph_id)
            .ok()
            .map(|n| n.icon())
    }

    fn new_node(&mut self, pos: egui::Pos2, snarl: &mut Snarl<GraphNode>, node: Box<dyn Node>) {
        let graph_id = self.terrain_graph.write().graph_mut().add_node(node);
        let snarl_id = snarl.insert_node(pos, GraphNode::Main(graph_id));
        self.selected = Some(SelectedNode { snarl_id, graph_id });
    }

    /// Clears the selection if it pointed at the removed node.
    fn remove_node(&mut self, node: NodeId, snarl: &mut Snarl<GraphNode>) {
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

/// Draws a pin's socket label, colored to reflect whether the socket is currently wired up.
fn show_pin_label(ui: &mut egui::Ui, name: &str, connected: bool) {
    let color = if connected {
        theme::palette::TEXT_MUTED
    } else {
        theme::palette::TEXT_DISABLED
    };

    ui.label(
        egui::RichText::new(name)
            .color(color)
            .font(egui::FontId::new(
                theme::fonts::FONT_SIZE_NODE_PIN,
                egui::FontFamily::Proportional
            ))
    );
}

pub fn show_graph(
    id: egui::Id,
    ui: &mut egui::Ui,
    graph_instance: &mut GraphInstance,
    terrain_graph: TerrainSessionHolder
) -> Option<SelectedNode> {
    let style = SnarlStyle {
        node_layout: Some(
            NodeLayout::coil()
                .with_min_pin_row_height(theme::layout::NODE_PIN_ROW_HEIGHT)
                .with_equal_pin_rows()
        ),
        node_frame: None,
        header_frame: None,

        collapsible: Some(false),
        header_drag_space: Some(egui::Vec2::ZERO),
        pin_size: Some(14.0),
        pin_fill: Some(theme::palette::PIN_DEFAULT),
        pin_stroke: Some(egui::Stroke::NONE),
        pin_shape: Some(PinShape::Circle),
        pin_placement: Some(PinPlacement::Inside),

        wire_width: Some(2.5),
        upscale_wire_frame: Some(true),
        wire_style: Some(WireStyle::Bezier5),
        wire_layer: Some(WireLayer::BehindNodes),

        bg_frame: Some(egui::Frame::NONE.fill(BG_GRAPH)),
        bg_pattern: Some(BackgroundPattern::grid(egui::vec2(50.0, 50.0), 0.0)),
        bg_pattern_stroke: Some(egui::Stroke::new(
            0.1,
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 12)
        )),

        min_scale: Some(0.4),
        max_scale: Some(1.0),
        centering: Some(true),
        crisp_magnified_text: Some(true),
        wire_smoothness: Some(0.0),
        ..SnarlStyle::default()
    };

    let selected_node_id = egui::Id::new("panel-graph-selected-node");
    let mut viewer = GraphViewer {
        selected: ui
            .ctx()
            .data(|d| d.get_temp::<SelectedNode>(selected_node_id)),
        terrain_graph
    };

    SnarlWidget::new()
        .id(id)
        .style(style)
        .show(graph_instance, &mut viewer, ui);

    // Skip if the user is typing into some other widget (e.g. a parameter field) that should receive the key instead.
    let delete_pressed =
        ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace));
    if delete_pressed
        && !ui.ctx().wants_keyboard_input()
        && let Some(selected) = viewer.selected
    {
        viewer.remove_node(selected.snarl_id, graph_instance);
    }

    match viewer.selected {
        Some(node) => ui.ctx().data_mut(|d| d.insert_temp(selected_node_id, node)),
        None => ui
            .ctx()
            .data_mut(|d| d.remove::<SelectedNode>(selected_node_id))
    }
    viewer.selected
}
