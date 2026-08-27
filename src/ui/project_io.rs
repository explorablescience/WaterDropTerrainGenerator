use std::collections::HashMap;
use std::path::Path;

use egui_snarl::{InPinId, NodeId, OutPinId};
use wde::prelude::ui::egui;

use crate::{
    TerrainSession, TerrainSessionHolder,
    core::{graph::GraphNodeId, session as project},
    ui::panel_graph::{GraphInstance, GraphNode}
};

/// Serializes the current graph editor layout and terrain graph to `path` as JSON.
pub(super) fn save_project(
    path: &Path,
    graph_instance: &GraphInstance,
    terrain_graph: &TerrainSessionHolder
) -> Result<(), String> {
    let positions: HashMap<GraphNodeId, [f32; 2]> = graph_instance
        .nodes_pos_ids()
        .map(|(_, pos, node)| {
            let GraphNode::Main(graph_id) = node;
            (*graph_id, [pos.x, pos.y])
        })
        .collect();

    project::save_project(path, terrain_graph.read().graph(), &positions)
}

/// Replaces the current graph editor layout and terrain graph with the project stored at `path`.
pub(super) fn load_project(
    path: &Path,
    graph_instance: &mut GraphInstance,
    terrain_graph: &TerrainSessionHolder
) -> Result<(), String> {
    let tile_size = terrain_graph.read().graph().tile_size();
    let built = project::load_project(path, tile_size)?;

    let mut new_graph_instance = GraphInstance::default();
    let mut snarl_id_of: HashMap<GraphNodeId, NodeId> = HashMap::new();
    for (&graph_id, &pos) in &built.positions {
        let snarl_id =
            new_graph_instance.insert_node(egui::pos2(pos[0], pos[1]), GraphNode::Main(graph_id));
        snarl_id_of.insert(graph_id, snarl_id);
    }
    for (from_node, from_socket, to_node, to_socket) in &built.edges {
        new_graph_instance.connect(
            OutPinId {
                node: snarl_id_of[from_node],
                output: *from_socket
            },
            InPinId {
                node: snarl_id_of[to_node],
                input: *to_socket
            }
        );
    }

    let mut terrain_graph = terrain_graph.write();
    *terrain_graph = TerrainSession::default();
    *terrain_graph.graph_mut() = built.graph;
    drop(terrain_graph);
    *graph_instance = new_graph_instance;
    Ok(())
}
