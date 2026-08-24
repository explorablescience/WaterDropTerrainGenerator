use crate::core::{node::Node, node_error::NodeError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GraphNodeId(pub usize);

struct NodeEntry {
    instance: Box<dyn Node>,
    inputs: Vec<Option<(GraphNodeId, usize)>>, // indexed by input socket
    outputs: Vec<GraphNodeId>,
}

struct EdgeEntry {
    from_node: GraphNodeId,
    from_socket: usize,
    to_node: GraphNodeId,
    to_socket: usize,
}

/// Pure graph structure: nodes, edges, connectivity. No evaluation state.
#[derive(Default)]
pub struct Topology {
    nodes: Vec<Option<NodeEntry>>, // None = removed slot, keeps ids stable
    edges: Vec<EdgeEntry>,
}

impl Topology {
    pub fn add_node(&mut self, node: Box<dyn Node>) -> GraphNodeId {
        let inputs = vec![None; node.inputs().len()];
        let id = GraphNodeId(self.nodes.len());
        self.nodes.push(Some(NodeEntry { instance: node, inputs, outputs: Vec::new() }));
        id
    }

    pub fn remove_node(&mut self, node_id: GraphNodeId) -> Result<(), NodeError> {
        self.entry(node_id)?;
        let touching: Vec<_> = self.edges.iter()
            .filter(|e| e.from_node == node_id || e.to_node == node_id)
            .map(|e| (e.from_node, e.from_socket, e.to_node, e.to_socket))
            .collect();
        for (f, fs, t, ts) in touching {
            self.disconnect(f, fs, t, ts)?;
        }
        self.nodes[node_id.0] = None;
        Ok(())
    }

    pub fn connect(
        &mut self,
        from_node: GraphNodeId,
        from_socket: usize,
        to_node: GraphNodeId,
        to_socket: usize,
    ) -> Result<(), NodeError> {
        self.entry(from_node)?;
        let slot = self.entry_mut(to_node)?
            .inputs
            .get_mut(to_socket)
            .ok_or(NodeError::InputSocketNotFound { node: format!("{:?}", to_node), socket: to_socket })?;
        if slot.is_some() {
            return Err(NodeError::SocketOccupied);
        }
        *slot = Some((from_node, from_socket));
        self.entry_mut(from_node)?.outputs.push(to_node);
        self.edges.push(EdgeEntry { from_node, from_socket, to_node, to_socket });
        Ok(())
    }

    pub fn disconnect(
        &mut self,
        from_node: GraphNodeId,
        from_socket: usize,
        to_node: GraphNodeId,
        to_socket: usize,
    ) -> Result<(), NodeError> {
        let idx = self.edges.iter()
            .position(|e| e.from_node == from_node && e.from_socket == from_socket
                && e.to_node == to_node && e.to_socket == to_socket)
            .ok_or(NodeError::NotConnected { from_node, from_socket, to_node, to_socket })?;
        self.edges.remove(idx);
        self.entry_mut(to_node)?.inputs[to_socket] = None;
        let outputs = &mut self.entry_mut(from_node)?.outputs;
        if let Some(pos) = outputs.iter().position(|&o| o == to_node) {
            outputs.remove(pos);
        }
        Ok(())
    }

    pub fn node(&self, id: GraphNodeId) -> Result<&(dyn Node + 'static), NodeError> {
        Ok(self.entry(id)?.instance.as_ref())
    }

    pub fn node_mut(&mut self, id: GraphNodeId) -> Result<&mut (dyn Node + 'static), NodeError> {
        Ok(self.entry_mut(id)?.instance.as_mut())
    }

    pub fn inputs(&self, id: GraphNodeId) -> Result<&[Option<(GraphNodeId, usize)>], NodeError> {
        Ok(&self.entry(id)?.inputs)
    }

    pub fn outputs(&self, id: GraphNodeId) -> Result<&[GraphNodeId], NodeError> {
        Ok(&self.entry(id)?.outputs)
    }

    fn entry(&self, id: GraphNodeId) -> Result<&NodeEntry, NodeError> {
        self.nodes.get(id.0).and_then(Option::as_ref).ok_or(NodeError::NodeNotFound(id))
    }

    fn entry_mut(&mut self, id: GraphNodeId) -> Result<&mut NodeEntry, NodeError> {
        self.nodes.get_mut(id.0).and_then(Option::as_mut).ok_or(NodeError::NodeNotFound(id))
    }
}
