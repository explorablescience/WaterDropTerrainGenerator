//! Library crate for the terrain editor.

use std::sync::{Arc, RwLock};
use std::time::Duration;

use bevy::{platform::collections::HashMap, prelude::*};

use crate::core::{
    graph::{GraphNodeId, NodeGraph, NodeGraphProcessResult},
    node_error::NodeError,
    node_message::{MessageLifetime, NodeMessage, TimedNodeMessage},
    tile_allocator::TileHandle
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
    /// Feedback from the most recent `on_action`/`set_param` call on each node: an error persists
    /// until the next call on that node, while a success confirmation fades out on its own.
    action_messages: HashMap<GraphNodeId, TimedNodeMessage>
}
impl Default for TerrainGraph {
    fn default() -> Self {
        Self {
            graph: NodeGraph::new(128),
            generations: HashMap::new(),
            selected_node: None,
            action_messages: HashMap::new()
        }
    }
}
impl TerrainGraph {
    /// How long a success confirmation from `on_action` stays visible before it fades out.
    pub const ACTION_MESSAGE_DURATION: Duration = Duration::from_secs(3);

    /// Records the feedback of an `on_action`/`set_param` call on `node_id`, replacing whatever
    /// was shown before it. `Ok` carries the confirmation text to show (fades out on its own);
    /// `Err` carries the failure to show (persists until the next call on this node).
    pub fn set_action_result(&mut self, node_id: GraphNodeId, result: Result<String, NodeError>) {
        let timed = match result {
            Ok(text) => TimedNodeMessage::new(
                NodeMessage::info(text),
                MessageLifetime::Timed(Self::ACTION_MESSAGE_DURATION)
            ),
            Err(err) => TimedNodeMessage::new(
                NodeMessage { severity: err.severity(), text: err.to_string() },
                MessageLifetime::Persistent
            )
        };
        self.action_messages.insert(node_id, timed);
    }

    /// Drops any action feedback currently shown for `node_id`, e.g. after an action succeeds
    /// without a confirmation message of its own, so a stale error from a previous attempt
    /// doesn't linger.
    pub fn clear_action_message(&mut self, node_id: GraphNodeId) {
        self.action_messages.remove(&node_id);
    }

    /// The still-live action feedback for `node_id`, if any.
    pub fn action_message(&self, node_id: GraphNodeId) -> Option<&NodeMessage> {
        self.action_messages
            .get(&node_id)
            .filter(|m| !m.is_expired())
            .map(|m| &m.message)
    }

    /// Time left before `node_id`'s action feedback expires on its own, if it's timed and still
    /// live. Used to schedule a repaint so the UI updates the moment the message should disappear.
    pub fn action_message_remaining(&self, node_id: GraphNodeId) -> Option<Duration> {
        self.action_messages.get(&node_id).and_then(TimedNodeMessage::remaining)
    }

    /// Drops any action feedback that has expired, so it doesn't linger in memory forever.
    pub fn prune_expired_messages(&mut self) {
        self.action_messages.retain(|_, m| !m.is_expired());
    }

    /// Processes the terrain graph starting from the specified node ID.
    /// Each instance of a node graph contains an internal state that tracks the latest generation of output tiles for each node.
    ///
    /// # Returns
    /// - `Ok(Some((generation, output_tiles)))` if the graph has been processed successfully and new output tiles are available since the last processing.
    /// - `Ok(None)` if the graph is still processing or if no new output tiles are available since the last processing.
    /// - `Err(NodeError)` if an error occurs during processing.
    pub fn process(
        &mut self,
        node_id: GraphNodeId
    ) -> Result<Option<(u32, Vec<TileHandle>)>, NodeError> {
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
            Err(e) => Err(e)
        }
    }

    pub fn graph(&self) -> &NodeGraph {
        &self.graph
    }
    pub fn graph_mut(&mut self) -> &mut NodeGraph {
        &mut self.graph
    }
}
