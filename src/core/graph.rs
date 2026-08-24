use std::collections::HashMap;
use std::mem::discriminant;

use crate::core::node::{Node, NodeError};
use crate::core::tile_allocator::{TileHandle, TilePool};

/// Identifies a node within a [`NodeGraph`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GraphNodeId(usize);

/// A connection from an output socket of one node to an input socket of another.
struct Edge {
    from_node: GraphNodeId,
    from_socket: usize,
    to_node: GraphNodeId,
    to_socket: usize
}

/// A directed graph of [`Node`]s, wired together through their input/output sockets.
/// Nodes are executed in dependency order: a node only runs once every node feeding its inputs has already produced its outputs.
#[derive(Default)]
pub struct NodeGraph {
    /// Removed nodes leave a `None` tombstone behind so that every other node's [`NodeId`]
    /// (a plain index into this vec) stays valid.
    nodes: Vec<Option<Box<dyn Node>>>,
    edges: Vec<Edge>,
    /// Dependency order computed by the last successful [`Self::validate`] call, for the node
    /// it validated. Consumed by [`Self::process`] so the topological sort isn't redone on every
    /// run. Cleared whenever the graph is mutated, since that can invalidate the order.
    cached_topo: Option<(GraphNodeId, Vec<GraphNodeId>)>
}
impl NodeGraph {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            cached_topo: None
        }
    }

    /// Adds a node to the graph and returns its [`NodeId`].
    pub fn add_node(&mut self, node: Box<dyn Node>) -> GraphNodeId {
        let id = GraphNodeId(self.nodes.len());
        self.nodes.push(Some(node));
        self.cached_topo = None;
        id
    }

    /// Removes the node with the given [`NodeId`] from the graph, disconnecting any edges to or from it and invalidating the cached topological order.
    /// Returns an error if the node doesn't exist (or was already removed).
    pub fn remove_node(&mut self, node_id: GraphNodeId) -> Result<(), NodeError> {
        let slot = self
            .nodes
            .get_mut(node_id.0)
            .ok_or(NodeError::NodeNotFound(node_id))?;
        if slot.take().is_none() {
            return Err(NodeError::NodeNotFound(node_id));
        }

        self.edges
            .retain(|e| e.from_node != node_id && e.to_node != node_id);
        self.cached_topo = None;
        Ok(())
    }

    /// Connects an output socket of `from_node` to an input socket of `to_node`.
    /// Fails if either socket doesn't exist or if their data types don't match.
    pub fn connect(
        &mut self,
        from_node: GraphNodeId,
        from_socket: usize,
        to_node: GraphNodeId,
        to_socket: usize
    ) -> Result<&mut Self, NodeError> {
        // Validate that the socket indices exist and that their types match
        let from = self.node(from_node)?;
        let out_socket = from.outputs().get(from_socket).ok_or_else(|| {
            NodeError::OutputSocketNotFound {
                node: from.label().to_string(),
                socket: from_socket
            }
        })?;
        let out_dtype = discriminant(&out_socket.dtype);

        let to = self.node(to_node)?;
        let in_socket = to.inputs().get(to_socket).ok_or_else(|| {
            NodeError::InputSocketNotFound {
                node: to.label().to_string(),
                socket: to_socket
            }
        })?;
        if discriminant(&in_socket.dtype) != out_dtype {
            return Err(NodeError::SocketTypeMismatch {
                from_node: from.label().to_string(),
                from_socket,
                to_node: to.label().to_string(),
                to_socket
            });
        }

        // Add the edge to the graph
        self.edges.push(Edge {
            from_node,
            from_socket,
            to_node,
            to_socket
        });
        self.cached_topo = None;
        Ok(self)
    }

    /// Disconnects an output socket of `from_node` from an input socket of `to_node`.
    /// Fails if the connection doesn't exist.
    pub fn disconnect(
        &mut self,
        from_node: GraphNodeId,
        from_socket: usize,
        to_node: GraphNodeId,
        to_socket: usize
    ) -> Result<&mut Self, NodeError> {
        let index = self.edges.iter().position(|e| {
            e.from_node == from_node
                && e.from_socket == from_socket
                && e.to_node == to_node
                && e.to_socket == to_socket
        });
        if let Some(i) = index {
            self.edges.remove(i);
            self.cached_topo = None;
            Ok(self)
        } else {
            Err(NodeError::NotConnected {
                from_node,
                from_socket,
                to_node,
                to_socket
            })
        }
    }

    /// Returns a reference to the node with the given [`NodeId`].
    pub fn node(&self, id: GraphNodeId) -> Result<&dyn Node, NodeError> {
        self.nodes
            .get(id.0)
            .and_then(|slot| slot.as_deref())
            .ok_or(NodeError::NodeNotFound(id))
    }

    /// Returns a mutable reference to the node with the given [`NodeId`].
    pub fn node_mut(&mut self, id: GraphNodeId) -> Result<&mut (dyn Node + '_), NodeError> {
        match self.nodes.get_mut(id.0) {
            Some(Some(node)) => Ok(node.as_mut()),
            _ => Err(NodeError::NodeNotFound(id))
        }
    }

    /// Returns `node_id` together with all of its ancestors, in dependency order.
    fn ancestors_topo(&self, node_id: GraphNodeId) -> Result<Vec<GraphNodeId>, NodeError> {
        // Walk backward from `node_id` to find the set of nodes it depends on.
        let mut included = vec![false; self.nodes.len()];
        included[node_id.0] = true;
        let mut stack = vec![node_id];
        while let Some(id) = stack.pop() {
            for edge in &self.edges {
                if edge.to_node == id && !included[edge.from_node.0] {
                    included[edge.from_node.0] = true;
                    stack.push(edge.from_node);
                }
            }
        }

        // Topologically sort that subset (Kahn's algorithm restricted to `included`).
        let mut in_degree = vec![0; self.nodes.len()];
        for edge in &self.edges {
            if included[edge.to_node.0] && included[edge.from_node.0] {
                in_degree[edge.to_node.0] += 1;
            }
        }
        let mut order = Vec::new();
        let mut ready: Vec<GraphNodeId> = (0..self.nodes.len())
            .filter(|&i| included[i] && in_degree[i] == 0)
            .map(GraphNodeId)
            .collect();
        while let Some(id) = ready.pop() {
            order.push(id);
            for edge in &self.edges {
                if edge.from_node == id && included[edge.to_node.0] {
                    in_degree[edge.to_node.0] -= 1;
                    if in_degree[edge.to_node.0] == 0 {
                        ready.push(edge.to_node);
                    }
                }
            }
        }

        if order.len() != included.iter().filter(|&&b| b).count() {
            return Err(NodeError::CyclicGraph);
        }
        Ok(order)
    }

    /// Evaluates `node_id` and produces its output tiles as square tiles of `tile_size` texels per side.
    /// Due to padding requirements of the nodes, the internal tile size used for computation may be larger than `tile_size`.
    /// Returns an error if the graph has not been validated or if any node fails to process.
    pub fn process(
        &mut self,
        node_id: GraphNodeId,
        tile_size: usize
    ) -> Result<Vec<TileHandle>, NodeError> {
        let order = match &self.cached_topo {
            Some((validated_id, order)) if *validated_id == node_id => order,
            _ => {
                // Validate the graph to update the cache
                let order = self.ancestors_topo(node_id)?;
                self.cached_topo = Some((node_id, order.clone()));
                &self.cached_topo.as_ref().unwrap().1
            }
        };

        let mut internal_tile_size = tile_size;
        for &id in order {
            let padding = self.node(id)?.size().div_ceil(2); // half of the kernel size, ceiled
            internal_tile_size += 2 * padding;
        }

        let pool = TilePool::new(internal_tile_size);

        let mut outputs: HashMap<GraphNodeId, Vec<TileHandle>> = HashMap::new();
        for &id in order {
            let node = self.node(id)?;
            let inputs = node
                .inputs()
                .iter()
                .enumerate()
                .map(|(socket, _)| {
                    let (from_node, from_socket) = self
                        .edges
                        .iter()
                        .find(|e| e.to_node == id && e.to_socket == socket)
                        .map(|e| (e.from_node, e.from_socket))
                        .ok_or_else(|| NodeError::InputNotConnected {
                            node: node.label().to_string(),
                            socket
                        })?;
                    outputs
                        .get(&from_node)
                        .and_then(|outs| outs.get(from_socket))
                        .cloned()
                        .ok_or_else(|| NodeError::OutputNotAvailable {
                            node: node.label().to_string()
                        })
                })
                .collect::<Result<Vec<_>, NodeError>>()?;

            let result = node.process(&pool, &inputs)?;
            outputs.insert(id, result);
        }

        outputs
            .remove(&node_id)
            .ok_or(NodeError::NodeNotEvaluated(node_id))
    }

    pub fn get_nodes_labels(&self) -> Vec<(GraphNodeId, String)> {
        self.nodes
            .iter()
            .enumerate()
            .filter_map(|(i, node)| node.as_ref().map(|n| (GraphNodeId(i), n.label().to_string())))
            .collect()
    }
}
