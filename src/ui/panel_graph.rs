use egui_snarl::{InPin, InPinId, NodeId, OutPin, OutPinId, Snarl, ui::{PinInfo, SnarlPin, SnarlStyle, SnarlViewer, SnarlWidget}};
use wde::prelude::{ui::egui};

pub type GraphInstance = Snarl<GraphNodes>;

pub enum GraphNodes {
    TestInput,
    TestOutput
}

struct GraphViewer;
impl SnarlViewer<GraphNodes> for GraphViewer {
    fn title(&mut self, node: &GraphNodes) -> String {
        match node {
            GraphNodes::TestInput => "Test Input".to_string(),
            GraphNodes::TestOutput => "Test Output".to_string()
        }
    }
    
    fn show_header(
        &mut self,
        node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut egui::Ui,
        snarl: &mut GraphInstance
    ) {
        let label = self.title(&snarl[node]);
        ui.label(label);
    }

    fn inputs(&mut self, node: &GraphNodes) -> usize {
        match node {
            GraphNodes::TestInput => 1,
            GraphNodes::TestOutput => 0
        }
    }

    fn outputs(&mut self, node: &GraphNodes) -> usize {
        match node {
            GraphNodes::TestInput => 0,
            GraphNodes::TestOutput => 1
        }
    }

    fn show_input(&mut self, _pin: &InPin, ui: &mut egui::Ui, _snarl: &mut GraphInstance) -> impl SnarlPin + 'static {
        ui.label("in");
        PinInfo::circle()
    }

    fn show_output(&mut self, _pin: &OutPin, ui: &mut egui::Ui, _snarl: &mut GraphInstance) -> impl SnarlPin + 'static {
        ui.label("out");
        PinInfo::circle()
    }
}


/// Initializes a new graph instance
pub fn init_graph() -> GraphInstance {
    let mut snarl = Snarl::new();
    let input = snarl.insert_node(egui::pos2(0.0, 0.0), GraphNodes::TestInput);
    let output = snarl.insert_node(egui::pos2(200.0, 0.0), GraphNodes::TestOutput);
    snarl.connect(OutPinId { node: input, output: 0 }, InPinId { node: output, input: 0 });
    snarl
}

/// Displays the graph editor using egui-snarl.
pub fn show_graph(id: egui::Id, ui: &mut egui::Ui, graph_instance: &mut GraphInstance) {
    let style = SnarlStyle::default();
    SnarlWidget::new()
        .id(id)
        .style(style)
        .show(graph_instance, &mut GraphViewer {}, ui);
}