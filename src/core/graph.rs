use std::mem::discriminant;

use crate::core::node::{Node, NodeError};

/// Identifies a node within a [`NodeGraph`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(usize);

/// A connection from an output socket of one node to an input socket of another.
struct Edge {
    from_node: NodeId,
    from_socket: usize,
    to_node: NodeId,
    to_socket: usize,
}

/// A directed graph of [`Node`]s, wired together through their input/output sockets.
/// Nodes are executed in dependency order: a node only runs once every node feeding its inputs has already produced its outputs.
#[derive(Default)]
pub struct NodeGraph {
    nodes: Vec<Box<dyn Node>>,
    edges: Vec<Edge>,
}
impl NodeGraph {
    /// Adds a node to the graph and returns its [`NodeId`].
    pub fn add_node(&mut self, node: Box<dyn Node>) -> NodeId {
        let id = NodeId(self.nodes.len());
        self.nodes.push(node);
        id
    }

    /// Connects an output socket of `from_node` to an input socket of `to_node`.
    /// Fails if either socket doesn't exist or if their data types don't match.
    pub fn connect(
        &mut self,
        from_node: NodeId,
        from_socket: usize,
        to_node: NodeId,
        to_socket: usize,
    ) -> Result<&mut Self, NodeError> {
        // Validate that the socket indices exist and that their types match
        let from = self.node(from_node)?;
        let out_socket = from.outputs().get(from_socket).ok_or_else(|| {
            NodeError(format!("{} has no output socket {}", from.label(), from_socket))
        })?;
        let out_dtype = discriminant(&out_socket.dtype);

        let to = self.node(to_node)?;
        let in_socket = to.inputs().get(to_socket).ok_or_else(|| {
            NodeError(format!("{} has no input socket {}", to.label(), to_socket))
        })?;
        if discriminant(&in_socket.dtype) != out_dtype {
            return Err(NodeError(format!(
                "Cannot connect {}:{} to {}:{}, socket types differ",
                from.label(),
                from_socket,
                to.label(),
                to_socket
            )));
        }

        // Add the edge to the graph
        self.edges.push(Edge {
            from_node,
            from_socket,
            to_node,
            to_socket,
        });
        Ok(self)
    }

    /// Validates the graph and computes a topological sort of the nodes to determine execution order.
    /// This function must be called before executing the graph.
    /// Returns an error if the graph contains a cycle.
    pub fn validate(&mut self) -> Result<(), NodeError> {
        // Compute a topological sort of the nodes to determine execution order
        let mut in_degree = vec![0; self.nodes.len()];
        for edge in &self.edges {
            in_degree[edge.to_node.0] += 1;
        }

        let mut order = Vec::with_capacity(self.nodes.len());
        let mut stack: Vec<NodeId> = self
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(i, _)| if in_degree[i] == 0 { Some(NodeId(i)) } else { None })
            .collect();

        while let Some(node_id) = stack.pop() {
            order.push(node_id);
            for edge in &self.edges {
                if edge.from_node == node_id {
                    in_degree[edge.to_node.0] -= 1;
                    if in_degree[edge.to_node.0] == 0 {
                        stack.push(edge.to_node);
                    }
                }
            }
        }

        if order.len() != self.nodes.len() {
            return Err(NodeError("Graph contains a cycle".to_string()));
        }
        Ok(())
    }




    pub fn node(&self, id: NodeId) -> Result<&dyn Node, NodeError> {
        self.nodes
            .get(id.0)
            .map(|n| n.as_ref())
            .ok_or_else(|| NodeError(format!("Unknown node id {:?}", id)))
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;

    #[test]
    fn test_node_graph_connections() {
        let mut graph = NodeGraph::default();
        let (node_a, node_b) = (
            graph.add_node(Box::new(NodeGeneratorPerlin::default())),
            graph.add_node(Box::new(NodeErosion::default())),
        );

        // Valid connection
        let result = graph.connect(node_a, 0, node_b, 0); 
        assert!(result.is_ok(), "Graph connection should succeed");

        // Invalid connection: NodeErosion has only one input socket (index 0)
        let result_invalid = graph.connect(node_a, 0, node_b, 1);
        assert!(result_invalid.is_err(), "Graph connection should fail due to invalid socket index");
    }

    #[test]
    fn test_node_graph_validation() {
        let mut graph = NodeGraph::default();
        let (node_a, node_b, node_c) = (
            graph.add_node(Box::new(NodeGeneratorPerlin::default())),
            graph.add_node(Box::new(NodeErosion::default())),
            graph.add_node(Box::new(NodeErosion::default())),
        );
        graph
            .connect(node_a, 0, node_b, 0)
            .and_then(|g| g.connect(node_b, 0, node_c, 0))
            .expect("Graph connections should succeed");
    }

    #[test]
    fn test_node_graph_cycle_detection() {
        let mut graph = NodeGraph::default();
        let (node_a, node_b) = (
            graph.add_node(Box::new(NodeErosion::default())),
            graph.add_node(Box::new(NodeErosion::default())),
        );
        graph
            .connect(node_a, 0, node_b, 0)
            .and_then(|g| g.connect(node_b, 0, node_a, 0)) // This creates a cycle
            .expect("Graph connections should succeed");

        // Validate the graph and expect an error due to the cycle
        let result = graph.validate();
        assert!(result.is_err(), "Graph validation should fail due to cycle");
    }
}
