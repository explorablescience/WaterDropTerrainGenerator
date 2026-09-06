//! This module contains the core evaluation logic for the node graph, including caching and parallel chunk processing.
use wde::prelude::*;

use crate::core::graph::NodeGraph;
use crate::core::graph::topology::GraphNodeId;
use crate::core::node::NodeError;

impl NodeGraph {
    /// Checks if all required inputs of a node and its ancestors are connected.
    pub fn has_valid_connections(&self, node_id: GraphNodeId) -> Result<(), NodeError> {
        let _span = debug_span!("has_valid_connections", node_id = ?node_id).entered();
        for &ancestor in &self.topology.collect_ancestors(node_id)? {
            let node = self.topology.node(ancestor)?;
            for (socket, input) in node.inputs().iter().enumerate() {
                if input.required {
                    let Some((from_node, _)) = self
                        .topology
                        .inputs(ancestor)?
                        .get(socket)
                        .and_then(Option::as_ref)
                    else {
                        return Err(NodeError::InputNotConnected {
                            node_id: ancestor,
                            node: node.label().to_string(),
                            socket: input.name.to_string()
                        });
                    };
                    // Recursively check the ancestor's required inputs
                    self.has_valid_connections(*from_node)?;
                }
            }
        }
        Ok(())
    }
}
