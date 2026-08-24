//! Library crate for the terrain editor.

use std::sync::{Arc, RwLock};

use bevy::{platform::collections::HashMap, prelude::*};

use crate::core::{
    graph::{GraphNodeId, NodeGraph, NodeGraphProcessResult},
    node_error::NodeError,
    tile_allocator::TileHandle,
};

pub mod core;
pub mod nodes;
pub mod render;
pub mod ui;

/// Resource that holds the terrain graph and its state.
#[derive(Resource, Default, Clone)]
pub struct TerrainGraphHolder(pub Arc<RwLock<TerrainGraph>>);
impl TerrainGraphHolder {
    /// Returns a read-only reference to the terrain graph.
    pub fn read(&self) -> std::sync::RwLockReadGuard<'_, TerrainGraph> {
        self.0.read().unwrap()
    }

    /// Returns a mutable reference to the terrain graph.
    pub fn write(&self) -> std::sync::RwLockWriteGuard<'_, TerrainGraph> {
        self.0.write().unwrap()
    }
}

/// Represents the terrain graph, which consists of a node graph and its associated state.
pub struct TerrainGraph {
    /// The underlying node graph that defines the terrain generation process.
    graph: NodeGraph,
    /// Mapping from a node's unique id to its latest resulting generation.
    generations: HashMap<GraphNodeId, u32>,
    /// The currently selected node in the graph editor, if any.
    pub selected_node: Option<GraphNodeId>,
}
impl Default for TerrainGraph {
    fn default() -> Self {
        Self {
            graph: NodeGraph::new(128),
            generations: HashMap::new(),
            selected_node: None,
        }
    }
}
impl TerrainGraph {
    /// Processes the terrain graph starting from the specified node ID.
    /// Each instance of a node graph contains an internal state that tracks the latest generation of output tiles for each node.
    /// 
    /// # Returns
    /// - `Ok(Some((generation, output_tiles)))` if the graph has been processed successfully and new output tiles are available since the last processing.
    /// - `Ok(None)` if the graph is still processing or if no new output tiles are available since the last processing.
    /// - `Err(NodeError)` if an error occurs during processing.
    pub fn process(&mut self, node_id: GraphNodeId) -> Result<Option<(u32, Vec<TileHandle>)>, NodeError> {
        let generation = self.generations.get(&node_id).copied().unwrap_or(0);
        match self.graph.process(node_id) {
            Ok(NodeGraphProcessResult::Processed(new_generation, output_tiles)) => {
                if new_generation <= generation {
                    // No new generation is available
                    return Ok(None);
                }

                // New generation is available
                self.generations.insert(node_id, new_generation);
                Ok(Some((new_generation, output_tiles)))
            }
            Ok(NodeGraphProcessResult::Processing) => {
                // Graph is still processing
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }

    pub fn graph(&self) -> &NodeGraph {
        &self.graph
    }
    pub fn graph_mut(&mut self) -> &mut NodeGraph {
        &mut self.graph
    }
}
