//! Library crate for the terrain editor.

use std::sync::{Arc, RwLock};

use bevy::prelude::*;

use crate::core::{
    graph::{NodeGraph, GraphNodeId},
    tile_allocator::TileBuffer
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

#[derive(Default)]
pub struct TerrainGraph {
    graph: NodeGraph,
    state: Option<(u32, Vec<Arc<TileBuffer>>)> // (generation, output tiles)
}
impl TerrainGraph {
    /// Processes the terrain graph starting from the given node ID and tile size.
    /// Returns an error if the processing fails.
    pub fn process(&mut self, node_id: GraphNodeId, tile_size: usize) -> Result<(), String> {
        let generation = self.state.as_ref().map(|(g, _)| *g).unwrap_or(0);
        let output_tiles = self.graph.process(node_id, tile_size).map_err(|e| e.0)?;
        self.state = Some((generation + 1, output_tiles));
        Ok(())
    }

    pub fn graph(&self) -> &NodeGraph {
        &self.graph
    }
    pub fn graph_mut(&mut self) -> &mut NodeGraph {
        &mut self.graph
    }
    /// Returns the current state of the terrain graph, including the generation and output tiles.
    pub fn state(&self) -> Option<(u32, &[Arc<TileBuffer>])> {
        self.state.as_ref().map(|(g, tiles)| (*g, tiles.as_slice()))
    }
}
