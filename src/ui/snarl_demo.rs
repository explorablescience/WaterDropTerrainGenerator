use egui_snarl::{
    InPin, InPinId, OutPin, OutPinId, Snarl,
    ui::{PinInfo, SnarlPin, SnarlStyle, SnarlViewer, SnarlWidget}
};
use wde::prelude::ui::egui;

/// Base id salt of the demo graph widget, so other UI (e.g. a node info panel)
/// can look up its selection state via [`egui_snarl::ui::get_selected_nodes`].
///
/// The caller is expected to derive the actual widget id from this (see
/// [`show_demo_graph`]), bumping it whenever the pane's rect changes size, since
/// egui-snarl only auto-fits the view to the graph the first time a given id is shown.
pub const GRAPH_ID_SALT: &str = "snarl-demo-graph";

/// The two node kinds shown in the demo graph, each with a single pin.
pub enum DemoNode {
    /// Source node, exposing one output.
    In,
    /// Sink node, exposing one input.
    Out
}
impl DemoNode {
    pub fn label(&self) -> &'static str {
        match self {
            DemoNode::In => "In",
            DemoNode::Out => "Out"
        }
    }
}

/// Minimal [`SnarlViewer`] wiring up [`DemoNode`]'s single pin per node.
struct DemoViewer;
impl SnarlViewer<DemoNode> for DemoViewer {
    fn title(&mut self, node: &DemoNode) -> String {
        node.label().to_string()
    }

    fn inputs(&mut self, node: &DemoNode) -> usize {
        match node {
            DemoNode::In => 0,
            DemoNode::Out => 1
        }
    }

    fn outputs(&mut self, node: &DemoNode) -> usize {
        match node {
            DemoNode::In => 1,
            DemoNode::Out => 0
        }
    }

    fn show_input(&mut self, _pin: &InPin, ui: &mut egui::Ui, _snarl: &mut Snarl<DemoNode>) -> impl SnarlPin + 'static {
        ui.label("in");
        PinInfo::circle()
    }

    fn show_output(&mut self, _pin: &OutPin, ui: &mut egui::Ui, _snarl: &mut Snarl<DemoNode>) -> impl SnarlPin + 'static {
        ui.label("out");
        PinInfo::circle()
    }
}

/// Builds the demo graph: an "In" node wired to an "Out" node.
pub fn new_demo_graph() -> Snarl<DemoNode> {
    let mut snarl = Snarl::new();
    let input = snarl.insert_node(egui::pos2(0.0, 0.0), DemoNode::In);
    let output = snarl.insert_node(egui::pos2(200.0, 0.0), DemoNode::Out);
    snarl.connect(OutPinId { node: input, output: 0 }, InPinId { node: output, input: 0 });
    snarl
}

/// Draws the demo graph into `ui`, under the given widget `id` (see [`GRAPH_ID_SALT`]).
pub fn show_demo_graph(ui: &mut egui::Ui, snarl: &mut Snarl<DemoNode>, style: SnarlStyle, id: egui::Id) {
    SnarlWidget::new().id(id).style(style).show(snarl, &mut DemoViewer, ui);
}
