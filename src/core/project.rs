//! Serializes a [`NodeGraph`] to disk and rebuilds one from disk, independent of any particular
//! graph editor UI. A UI layer only needs to supply each node's editor position on save, and gets
//! back enough information on load (positions, connections) to rebuild its own view without
//! having to separately introspect the loaded `NodeGraph`.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::core::{
    chunk_grid::ChunkGrid,
    graph::{GraphNodeId, NodeGraph},
    node_parameters::NParamValue,
    node_registry
};

/// On-disk representation of a saved node graph.
#[derive(Serialize, Deserialize)]
struct ProjectFile {
    version: u32,
    /// Absent in `version: 1` files, saved before chunking existed - `load_project` falls back to
    /// a single-chunk grid built from the caller-supplied tile size in that case.
    #[serde(default)]
    chunk_grid: Option<ChunkGridDto>,
    nodes: Vec<ProjectNode>,
    edges: Vec<ProjectEdge>
}

/// On-disk representation of a [`ChunkGrid`]: the terrain-level layout is a deliberate,
/// persisted property of the project, not a runtime/session setting.
#[derive(Serialize, Deserialize)]
struct ChunkGridDto {
    chunks_x: u32,
    chunks_y: u32,
    tile_size: usize,
    world_scale: f32
}
impl ChunkGridDto {
    fn from_grid(grid: &ChunkGrid) -> Self {
        Self {
            chunks_x: grid.chunks_x(),
            chunks_y: grid.chunks_y(),
            tile_size: grid.tile_size(),
            world_scale: grid.world_scale()
        }
    }
    fn into_grid(self) -> ChunkGrid {
        ChunkGrid::new(self.chunks_x, self.chunks_y, self.tile_size, self.world_scale)
    }
}

#[derive(Serialize, Deserialize)]
struct ProjectNode {
    /// Id local to this file, used only to resolve `ProjectEdge` endpoints below.
    id: usize,
    /// Matches `Node::label()`, which registered node types also use as their "Add Node" menu
    /// entry (see `node_registry`), so it doubles as a stable type identifier.
    node_type: String,
    position: [f32; 2],
    params: Vec<(String, ParamValueDto)>
}

#[derive(Serialize, Deserialize)]
struct ProjectEdge {
    from_node: usize,
    from_socket: usize,
    to_node: usize,
    to_socket: usize
}

/// Mirrors `NParamValue`, minus the stateless `Action` variant: a button press has nothing to
/// persist, so nodes are reloaded with actions left untriggered.
#[derive(Serialize, Deserialize, Clone)]
enum ParamValueDto {
    Int(i32),
    Float(f32),
    Bool(bool),
    String(String),
    Enum(String),
    Vector2(f32, f32),
    Vector2Int(i32, i32)
}
impl ParamValueDto {
    fn from_value(value: &NParamValue) -> Option<Self> {
        match value {
            NParamValue::Int(v) => Some(Self::Int(*v)),
            NParamValue::Float(v) => Some(Self::Float(*v)),
            NParamValue::Bool(v) => Some(Self::Bool(*v)),
            NParamValue::String(v) => Some(Self::String(v.clone())),
            NParamValue::Enum(v) => Some(Self::Enum(v.clone())),
            NParamValue::Vector2(x, y) => Some(Self::Vector2(*x, *y)),
            NParamValue::Vector2Int(x, y) => Some(Self::Vector2Int(*x, *y)),
            NParamValue::Action { .. } => None
        }
    }
    fn into_value(self) -> NParamValue {
        match self {
            Self::Int(v) => NParamValue::Int(v),
            Self::Float(v) => NParamValue::Float(v),
            Self::Bool(v) => NParamValue::Bool(v),
            Self::String(v) => NParamValue::String(v),
            Self::Enum(v) => NParamValue::Enum(v),
            Self::Vector2(x, y) => NParamValue::Vector2(x, y),
            Self::Vector2Int(x, y) => NParamValue::Vector2Int(x, y)
        }
    }
}

/// A freshly rebuilt graph, along with the editor position and connections of every node in it -
/// everything a UI layer needs to rebuild its own node-position view (e.g. an `egui-snarl`
/// `Snarl`) without needing any further introspection into `NodeGraph`'s internal topology.
pub struct BuiltGraph {
    pub graph: NodeGraph,
    pub positions: HashMap<GraphNodeId, [f32; 2]>,
    /// `(from_node, from_socket, to_node, to_socket)` for every connection in the graph.
    pub edges: Vec<(GraphNodeId, usize, GraphNodeId, usize)>
}

/// Serializes `graph` (every node, its parameters and connections) plus each node's editor
/// position to `path` as JSON. `positions` need not cover every node in `graph`: a missing entry
/// is saved as `[0.0, 0.0]`.
pub fn save_project(
    path: &Path,
    graph: &NodeGraph,
    positions: &HashMap<GraphNodeId, [f32; 2]>
) -> Result<(), String> {
    let ids: Vec<GraphNodeId> = graph.node_ids().collect();
    let id_map: HashMap<GraphNodeId, usize> =
        ids.iter().enumerate().map(|(file_id, id)| (*id, file_id)).collect();

    let mut nodes = Vec::with_capacity(ids.len());
    for &graph_id in &ids {
        let node = graph.node(graph_id).map_err(|e| e.to_string())?;
        let params = node
            .desc_params()
            .iter()
            .filter_map(|desc| {
                let value = node.get_param(desc.key)?;
                Some((desc.key.to_string(), ParamValueDto::from_value(&value)?))
            })
            .collect();

        nodes.push(ProjectNode {
            id: id_map[&graph_id],
            node_type: node.label().to_string(),
            position: positions.get(&graph_id).copied().unwrap_or([0.0, 0.0]),
            params
        });
    }

    let mut edges = Vec::new();
    for &graph_id in &ids {
        let inputs = graph.inputs(graph_id).map_err(|e| e.to_string())?;
        for (socket, input) in inputs.iter().enumerate() {
            let Some((from_node, from_socket)) = input else { continue };
            edges.push(ProjectEdge {
                from_node: id_map[from_node],
                from_socket: *from_socket,
                to_node: id_map[&graph_id],
                to_socket: socket
            });
        }
    }

    let file = ProjectFile {
        version: 2,
        chunk_grid: Some(ChunkGridDto::from_grid(graph.chunk_grid())),
        nodes,
        edges
    };
    let json = serde_json::to_string_pretty(&file).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

/// Rebuilds a fresh graph (with the given `tile_size`) from the project stored at `path`.
pub fn load_project(path: &Path, tile_size: usize) -> Result<BuiltGraph, String> {
    let json = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let file: ProjectFile = serde_json::from_str(&json).map_err(|e| e.to_string())?;

    let chunk_grid = file
        .chunk_grid
        .map(ChunkGridDto::into_grid)
        .unwrap_or_else(|| ChunkGrid::single(tile_size));
    let mut graph = NodeGraph::new(chunk_grid);
    let mut positions = HashMap::new();
    let mut id_map: HashMap<usize, GraphNodeId> = HashMap::new();

    for node in &file.nodes {
        let descriptor = node_registry::registered_nodes()
            .find(|d| d.label == node.node_type)
            .ok_or_else(|| format!("Unknown node type '{}'", node.node_type))?;

        let mut instance = (descriptor.factory)();
        for (key, value) in &node.params {
            // Best-effort: an unknown/invalid param shouldn't abort loading the rest of the graph.
            let _ = instance.set_param(key, value.clone().into_value());
        }

        let graph_id = graph.add_node(instance);
        positions.insert(graph_id, node.position);
        id_map.insert(node.id, graph_id);
    }

    let mut edges = Vec::with_capacity(file.edges.len());
    for edge in &file.edges {
        let &from_node = id_map
            .get(&edge.from_node)
            .ok_or("Edge refers to an unknown node")?;
        let &to_node = id_map
            .get(&edge.to_node)
            .ok_or("Edge refers to an unknown node")?;

        graph
            .connect(from_node, edge.from_socket, to_node, edge.to_socket)
            .map_err(|e| e.to_string())?;
        edges.push((from_node, edge.from_socket, to_node, edge.to_socket));
    }

    Ok(BuiltGraph { graph, positions, edges })
}
