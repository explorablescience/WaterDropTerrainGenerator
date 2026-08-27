use crate::core::graph::GraphNodeId;
use crate::core::node_message::NodeMessageSeverity;

#[derive(Debug)]
pub enum NodeError {
    NodeNotFound(GraphNodeId),
    OutputSocketNotFound {
        node: String,
        socket: usize
    },
    InputSocketNotFound {
        node: String,
        socket: usize
    },
    SocketOccupied,
    SocketTypeMismatch {
        from_node: String,
        from_socket: String,
        to_node: String,
        to_socket: String
    },
    NotConnected {
        from_node: GraphNodeId,
        from_socket: String,
        to_node: GraphNodeId,
        to_socket: String
    },
    /// `node_id` identifies the node unambiguously (two nodes of the same type share the same `node` label).
    InputNotConnected {
        node_id: GraphNodeId,
        node: String,
        socket: String
    },
    CyclicGraph,
    /// Internal scheduling bug: the topological order should guarantee this never happens.
    OutputNotAvailable {
        node: String
    },
    /// Internal scheduling bug.
    NodeNotEvaluated(GraphNodeId),
    ProcessingFailed(String)
}

impl std::fmt::Display for NodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NodeNotFound(id) => write!(f, "Unknown node id {:?}", id),
            Self::OutputSocketNotFound { node, socket } => {
                write!(f, "{} has no output socket {}", node, socket)
            }
            Self::InputSocketNotFound { node, socket } => {
                write!(f, "{} has no input socket {}", node, socket)
            }
            Self::SocketOccupied => write!(f, "Socket is already occupied by another connection"),
            Self::SocketTypeMismatch {
                from_node,
                from_socket,
                to_node,
                to_socket
            } => write!(
                f,
                "Cannot connect {}:{} to {}:{}, socket types differ",
                from_node, from_socket, to_node, to_socket
            ),
            Self::NotConnected {
                from_node,
                from_socket,
                to_node,
                to_socket
            } => write!(
                f,
                "No connection from {:?}:{} to {:?}:{}",
                from_node, from_socket, to_node, to_socket
            ),
            Self::InputNotConnected { node, socket, .. } => {
                write!(f, "\"{}\" input \"{}\" is not connected", node, socket)
            }
            Self::CyclicGraph => write!(f, "Graph contains a cycle"),
            Self::OutputNotAvailable { node } => {
                write!(f, "No output available to feed {}", node)
            }
            Self::NodeNotEvaluated(id) => write!(f, "{:?} was not evaluated", id),
            Self::ProcessingFailed(msg) => write!(f, "{}", msg)
        }
    }
}

impl std::error::Error for NodeError {}

impl NodeError {
    pub fn severity(&self) -> NodeMessageSeverity {
        match self {
            Self::InputNotConnected { .. } => NodeMessageSeverity::Warning,
            _ => NodeMessageSeverity::Error
        }
    }
}

impl From<&str> for NodeError {
    fn from(msg: &str) -> Self {
        Self::ProcessingFailed(msg.to_string())
    }
}

impl From<String> for NodeError {
    fn from(msg: String) -> Self {
        Self::ProcessingFailed(msg)
    }
}
