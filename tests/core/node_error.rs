use waterdrop_terrain_editor::core::graph::GraphNodeId;
use waterdrop_terrain_editor::core::node_error::NodeError;
use waterdrop_terrain_editor::core::node_message::NodeMessageSeverity;

#[test]
fn input_not_connected_displays_the_node_and_socket_names() {
    let err = NodeError::InputNotConnected {
        node_id: GraphNodeId(0),
        node: "Erosion".to_string(),
        socket: "Height".to_string()
    };
    assert_eq!(err.to_string(), "\"Erosion\" input \"Height\" is not connected");
}

#[test]
fn socket_type_mismatch_displays_both_endpoints() {
    let err = NodeError::SocketTypeMismatch {
        from_node: "A".to_string(),
        from_socket: "Height".to_string(),
        to_node: "B".to_string(),
        to_socket: "Color".to_string()
    };
    assert_eq!(err.to_string(), "Cannot connect A:Height to B:Color, socket types differ");
}

#[test]
fn socket_occupied_has_a_fixed_message() {
    assert_eq!(
        NodeError::SocketOccupied.to_string(),
        "Socket is already occupied by another connection"
    );
}

#[test]
fn cyclic_graph_has_a_fixed_message() {
    assert_eq!(NodeError::CyclicGraph.to_string(), "Graph contains a cycle");
}

#[test]
fn processing_failed_displays_its_inner_message_verbatim() {
    let err: NodeError = "disk is full".into();
    assert_eq!(err.to_string(), "disk is full");
}

#[test]
fn from_str_and_from_string_both_produce_processing_failed() {
    let from_str: NodeError = "boom".into();
    let from_string: NodeError = String::from("boom").into();
    assert_eq!(from_str.to_string(), from_string.to_string());
}

#[test]
fn only_input_not_connected_is_a_warning() {
    let warning = NodeError::InputNotConnected {
        node_id: GraphNodeId(0),
        node: "Erosion".to_string(),
        socket: "Height".to_string()
    };
    assert_eq!(warning.severity(), NodeMessageSeverity::Warning);

    let errors = [
        NodeError::NodeNotFound(GraphNodeId(0)),
        NodeError::SocketOccupied,
        NodeError::CyclicGraph,
        NodeError::ProcessingFailed("x".to_string())
    ];
    for err in errors {
        assert_eq!(err.severity(), NodeMessageSeverity::Error);
    }
}
