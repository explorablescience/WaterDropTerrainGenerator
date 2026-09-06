//! This module contains the core evaluation logic for the node graph, including caching and parallel chunk processing.
use wde::prelude::*;

use std::collections::HashSet;

use crate::core::CacheEntry;
use crate::core::graph::topology::GraphNodeId;
use crate::core::graph::NodeGraph;
use crate::core::node::{NodeError, NodeLocality};

impl NodeGraph {
    /// Collects all ancestor nodes of the given `node_id`, including itself.
    fn collect_ancestors(&self, node_id: GraphNodeId) -> Result<HashSet<GraphNodeId>, NodeError> {
        let _span = debug_span!("collect_ancestors", node_id = ?node_id).entered();
        self.topology.node(node_id)?;
        let mut seen = HashSet::from([node_id]);
        let mut stack = vec![node_id];
        while let Some(id) = stack.pop() {
            if id != node_id
                && matches!(
                    self.topology.node(id)?.locality(),
                    NodeLocality::Global { .. }
                )
            {
                continue;
            }
            for (from_node, _) in self.topology.inputs(id)?.iter().flatten() {
                if seen.insert(*from_node) {
                    stack.push(*from_node);
                }
            }
        }
        Ok(seen)
    }
    
    /// Checks if all required inputs of a node and its ancestors are connected.
    pub fn has_valid_connections(&self, node_id: GraphNodeId) -> Result<(), NodeError> {
        let _span = debug_span!("has_valid_connections", node_id = ?node_id).entered();
        for &ancestor in &self.collect_ancestors(node_id)? {
            let node = self.topology.node(ancestor)?;
            for (socket, input) in node.inputs().iter().enumerate() {
                if input.required {
                    let Some((from_node, _)) = self.topology.inputs(ancestor)?.get(socket).and_then(Option::as_ref) else {
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
