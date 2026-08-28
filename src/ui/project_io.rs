//! "File / Save" and "File / Load" menu items: serializes the whole project (chunk grid, node
//! graph, per-node params and graph-editor layout) to a fixed `terrain.wdtg` JSON file.

use std::collections::HashMap;
use std::path::Path;

use bevy::prelude::*;
use egui_snarl::{InPinId, OutPinId};
use rfd::FileDialog;
use serde::{Deserialize, Serialize};
use wde::prelude::{ui::egui, *};

use crate::{
    TerrainSessionHolder,
    core::{
        graph::{GraphNodeId, NodeGraph},
        node::{self, NParamValue},
        tiling::ChunkGrid
    },
    ui::panel_graph::{self, GraphEditorState, GraphInstance, GraphNode}
};

pub const PROJECT_FILE_NAME: &str = "terrain.wdtg";

#[derive(Serialize, Deserialize)]
struct ProjectFile {
    chunk_grid: SavedChunkGrid,
    nodes: Vec<SavedNode>,
    edges: Vec<SavedEdge>
}

#[derive(Serialize, Deserialize)]
struct SavedChunkGrid {
    chunks_x: u32,
    chunks_y: u32,
    tile_size: usize,
    world_scale: f32
}

/// `id` is only used to remap edges (`SavedEdge::from_node`/`to_node`) on load - it isn't
/// meaningful once loaded, since the graph is rebuilt with fresh node ids.
#[derive(Serialize, Deserialize)]
struct SavedNode {
    id: usize,
    /// Matches both [`Node::label`](crate::core::node::Node::label) and
    /// [`NodeDescriptor::label`](crate::core::node::NodeDescriptor), which the registry keeps in
    /// sync - see the node's own `inventory::submit!`.
    type_label: String,
    position: (f32, f32),
    params: Vec<(String, NParamValue)>
}

#[derive(Serialize, Deserialize)]
struct SavedEdge {
    from_node: usize,
    from_socket: usize,
    to_node: usize,
    to_socket: usize
}

pub fn draw_project_menu(
    ctx: Res<UIContext>,
    mut ui_menu: ResMut<UIMenu>,
    terrain_graph: Res<TerrainSessionHolder>,
    mut graph_instance: ResMut<GraphEditorState>
) {
    let save_clicked = ui_menu.clicked_mut("File/Save Project");
    if *save_clicked {
        *save_clicked = false;
        let path = FileDialog::new()
            .add_filter("WaterDrop Terrain Generator project", &["wdtg"])
            .set_file_name(PROJECT_FILE_NAME)
            .save_file();
        if let Some(path) = path {
            match save_project(&terrain_graph, &graph_instance.0, &path) {
                Ok(()) => info!("Saved project to {}", path.display()),
                Err(e) => error!("Failed to save project: {e}")
            }
        }
    }

    let load_clicked = ui_menu.clicked_mut("File/Load Project");
    if *load_clicked {
        *load_clicked = false;
        if let Some(path) = FileDialog::new()
            .add_filter("WaterDrop Terrain Generator project", &["wdtg"])
            .pick_file()
        {
            match load_project(&terrain_graph, &mut graph_instance.0, &path) {
                Ok(()) => {
                    panel_graph::clear_selection(&ctx.0);
                    panel_graph::clear_pins(&ctx.0);
                    info!("Loaded project from {}", path.display());
                }
                Err(e) => error!("Failed to load project: {e}")
            }
        }
    }
}

fn save_project(
    terrain_graph: &TerrainSessionHolder,
    graph_instance: &GraphInstance,
    path: &Path
) -> Result<(), String> {
    let session = terrain_graph.read();
    let graph = session.graph();
    let chunk_grid = *graph.chunk_grid();

    let nodes = graph_instance
        .nodes_pos_ids()
        .map(|(_snarl_id, pos, node)| {
            let GraphNode::Main(graph_id) = node;
            let n = graph.node(*graph_id).map_err(|e| e.to_string())?;
            let params = n
                .desc_params()
                .iter()
                .filter_map(|desc| {
                    n.get_param(desc.key)
                        .filter(|v| !matches!(v, NParamValue::Action { .. }))
                        .map(|v| (desc.key.to_string(), v))
                })
                .collect();
            Ok(SavedNode {
                id: graph_id.0,
                type_label: n.label().to_string(),
                position: (pos.x, pos.y),
                params
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    let edges = graph
        .edges()
        .map(
            |(from_node, from_socket, to_node, to_socket)| SavedEdge {
                from_node: from_node.0,
                from_socket,
                to_node: to_node.0,
                to_socket
            }
        )
        .collect();

    let file = ProjectFile {
        chunk_grid: SavedChunkGrid {
            chunks_x: chunk_grid.chunks_x(),
            chunks_y: chunk_grid.chunks_y(),
            tile_size: chunk_grid.tile_size(),
            world_scale: chunk_grid.world_scale()
        },
        nodes,
        edges
    };

    let json = serde_json::to_string_pretty(&file).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

fn load_project(
    terrain_graph: &TerrainSessionHolder,
    graph_instance: &mut GraphInstance,
    path: &Path
) -> Result<(), String> {
    let json = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let file: ProjectFile = serde_json::from_str(&json).map_err(|e| e.to_string())?;

    let chunk_grid = ChunkGrid::new(
        file.chunk_grid.chunks_x,
        file.chunk_grid.chunks_y,
        file.chunk_grid.tile_size,
        file.chunk_grid.world_scale
    );
    let mut graph = NodeGraph::new(chunk_grid);

    let mut graph_ids: HashMap<usize, GraphNodeId> = HashMap::new();
    for saved in &file.nodes {
        let descriptor = node::registered_nodes()
            .find(|d| d.label == saved.type_label)
            .ok_or_else(|| format!("Unknown node type '{}'", saved.type_label))?;
        let mut instance = (descriptor.factory)();
        for (key, value) in &saved.params {
            if let Err(e) = instance.set_param(key, value.clone()) {
                warn!("Skipping stale parameter '{key}' on '{}': {e}", saved.type_label);
            }
        }
        graph_ids.insert(saved.id, graph.add_node(instance));
    }
    for edge in &file.edges {
        let (Some(&from), Some(&to)) =
            (graph_ids.get(&edge.from_node), graph_ids.get(&edge.to_node))
        else {
            continue;
        };
        graph
            .connect(from, edge.from_socket, to, edge.to_socket)
            .map_err(|e| e.to_string())?;
    }

    *graph_instance = GraphInstance::new();
    let mut snarl_ids: HashMap<usize, egui_snarl::NodeId> = HashMap::new();
    for saved in &file.nodes {
        let snarl_id = graph_instance.insert_node(
            egui::pos2(saved.position.0, saved.position.1),
            GraphNode::Main(graph_ids[&saved.id])
        );
        snarl_ids.insert(saved.id, snarl_id);
    }
    for edge in &file.edges {
        let (Some(&from), Some(&to)) =
            (snarl_ids.get(&edge.from_node), snarl_ids.get(&edge.to_node))
        else {
            continue;
        };
        graph_instance.connect(
            OutPinId {
                node: from,
                output: edge.from_socket
            },
            InPinId {
                node: to,
                input: edge.to_socket
            }
        );
    }

    terrain_graph.write().reset_graph(graph);
    Ok(())
}
