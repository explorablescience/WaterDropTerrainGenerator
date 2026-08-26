use crate::core::graph::GraphNodeId;
use crate::core::node_message::NodeMessageSeverity;

/// An error produced while working with a [`Node`] or the [`NodeGraph`](crate::core::graph::NodeGraph) it belongs to.
#[derive(Debug)]
pub enum NodeError {
    /// No node exists with this id (it was never added, or has since been removed).
    NodeNotFound(GraphNodeId),
    /// The node has no output socket at this index.
    OutputSocketNotFound { node: String, socket: usize },
    /// The node has no input socket at this index.
    InputSocketNotFound { node: String, socket: usize },
    /// The socket is already occupied by another connection, and cannot be connected to.
    SocketOccupied,
    /// The output and input sockets being connected carry different data types.
    SocketTypeMismatch {
        from_node: String,
        from_socket: String,
        to_node: String,
        to_socket: String
    },
    /// No connection exists between the given output and input sockets.
    NotConnected {
        from_node: GraphNodeId,
        from_socket: String,
        to_node: GraphNodeId,
        to_socket: String
    },
    /// An input socket of the node has no incoming connection. `node_id` identifies the node
    /// unambiguously (two nodes of the same type share the same `node` label), which callers
    /// need to tell whether this error is about the node they're looking at or an ancestor of it.
    InputNotConnected { node_id: GraphNodeId, node: String, socket: String },
    /// The dependency graph contains a cycle, so it cannot be topologically sorted.
    CyclicGraph,
    /// A connected input socket's source node has not produced its output yet (internal
    /// scheduling bug: the topological order should guarantee this never happens).
    OutputNotAvailable { node: String },
    /// The requested node was never evaluated, so it has no output (internal scheduling bug).
    NodeNotEvaluated(GraphNodeId),
    /// A node failed while processing its inputs, with a node-specific message.
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
    /// Severity to display this error with in the editor's UI.
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
