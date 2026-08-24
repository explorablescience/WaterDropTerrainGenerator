use crate::core::{
    node::{Node, NodeError},
    tile_allocator::TileHandle,
};

/// Represents a unique identifier for a node in the graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GraphNodeId(pub usize);

/// Represents the result of processing a node graph.
pub enum NodeGraphProcessResult {
    /// The graph has been processed successfully, and the output tiles are available (generation, output tiles).
    Processed(u32, Vec<TileHandle>),
    /// The graph is still being processed, and the output tiles are not yet available.
    Processing,
}

/// Represents an entry for a node in the graph, containing its unique identifier and the node itself.
pub struct NodeEntry {
    id: GraphNodeId,
    instance: Box<dyn Node>,
    data: Option<Vec<TileHandle>>,
    dirty: bool,
}

/// Represents an entry for an edge in the graph, containing the source and destination node identifiers and socket indices.
pub struct EdgeEntry {
    from_node: GraphNodeId,
    from_socket: usize,
    to_node: GraphNodeId,
    to_socket: usize,
}

/// Represents a node graph that can be processed to generate output tiles.
pub struct NodeGraph {
    /// Size of the tiles to be generated
    tile_size: usize,

    nodes: Vec<NodeEntry>,
    edges: Vec<EdgeEntry>,
}
impl NodeGraph {
    /// Creates a new `NodeGraph` with the specified tile size.
    pub fn new(tile_size: usize) -> Self {
        Self {
            tile_size,
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }

    /// Adds a new node to the graph and returns its unique identifier.
    pub fn add_node(&mut self, node: Box<dyn Node>) -> GraphNodeId {
        todo!()
    }
    /// Removes a node from the graph by its unique identifier.
    pub fn remove_node(&mut self, node_id: GraphNodeId) -> Result<(), NodeError> {
        todo!()
    }

    /// Connects two nodes in the graph by specifying their unique identifiers and socket indices.
    /// Returns an error if the connection fails.
    pub fn connect(
        &mut self,
        from_node: GraphNodeId,
        from_socket: usize,
        to_node: GraphNodeId,
        to_socket: usize,
    ) -> Result<&mut Self, NodeError> {
        todo!()
    }
    /// Disconnects two nodes in the graph by specifying their unique identifiers and socket indices.
    /// Returns an error if the disconnection fails.
    pub fn disconnect(
        &mut self,
        from_node: GraphNodeId,
        from_socket: usize,
        to_node: GraphNodeId,
        to_socket: usize,
    ) -> Result<&mut Self, NodeError> {
        todo!()
    }

    /// Processes the graph starting from the specified node ID.
    /// Returns a `NodeGraphProcessResult` indicating the outcome of the processing.
    pub fn process(&mut self, node_id: GraphNodeId) -> Result<NodeGraphProcessResult, NodeError> {
        todo!()
    }

    pub fn node(&self, id: GraphNodeId) -> Result<&dyn Node, NodeError> {
        todo!()
    }
    pub fn node_mut(&mut self, id: GraphNodeId) -> Result<&mut dyn Node, NodeError> {
        todo!()
    }
}
