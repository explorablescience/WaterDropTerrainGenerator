use crate::core::{node::Node, node_error::NodeError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GraphNodeId(pub usize);

struct NodeEntry {
    instance: Box<dyn Node>,
    inputs: Vec<Option<(GraphNodeId, usize)>>, // indexed by input socket
    outputs: Vec<GraphNodeId>
}

struct EdgeEntry {
    from_node: GraphNodeId,
    from_socket: usize,
    to_node: GraphNodeId,
    to_socket: usize
}

/// Pure graph structure: nodes, edges, connectivity. No evaluation state.
#[derive(Default)]
pub struct Topology {
    nodes: Vec<Option<NodeEntry>>, // None = removed slot, keeps ids stable
    edges: Vec<EdgeEntry>
}

impl Topology {
    pub fn add_node(&mut self, node: Box<dyn Node>) -> GraphNodeId {
        let inputs = vec![None; node.inputs().len()];
        let id = GraphNodeId(self.nodes.len());
        self.nodes.push(Some(NodeEntry {
            instance: node,
            inputs,
            outputs: Vec::new()
        }));
        id
    }

    pub fn remove_node(&mut self, node_id: GraphNodeId) -> Result<(), NodeError> {
        self.entry(node_id)?;
        let touching: Vec<_> = self
            .edges
            .iter()
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
        to_socket: usize
    ) -> Result<(), NodeError> {
        let from_socket_desc = self
            .entry(from_node)?
            .instance
            .outputs()
            .get(from_socket)
            .ok_or(NodeError::OutputSocketNotFound {
                node: format!("{:?}", from_node),
                socket: from_socket
            })?;
        let from_dtype = from_socket_desc.dtype;
        let from_label = from_socket_desc.name;

        let to_entry = self.entry(to_node)?;
        let to_socket_desc = to_entry.instance.inputs().get(to_socket).ok_or(
            NodeError::InputSocketNotFound {
                node: format!("{:?}", to_node),
                socket: to_socket
            }
        )?;
        if to_socket_desc.dtype != from_dtype {
            return Err(NodeError::SocketTypeMismatch {
                from_node: format!("{:?}", from_node),
                from_socket: from_label.to_string(),
                to_node: format!("{:?}", to_node),
                to_socket: to_socket_desc.name.to_string()
            });
        }
        // An input pin can only ever hold one connection: replace whatever was already
        // plugged into it rather than rejecting the new connection.
        let existing = to_entry.inputs[to_socket];
        if let Some((old_from_node, old_from_socket)) = existing {
            self.disconnect(old_from_node, old_from_socket, to_node, to_socket)?;
        }

        self.entry_mut(to_node)?.inputs[to_socket] = Some((from_node, from_socket));
        self.entry_mut(from_node)?.outputs.push(to_node);
        self.edges.push(EdgeEntry {
            from_node,
            from_socket,
            to_node,
            to_socket
        });
        Ok(())
    }

    pub fn disconnect(
        &mut self,
        from_node: GraphNodeId,
        from_socket: usize,
        to_node: GraphNodeId,
        to_socket: usize
    ) -> Result<(), NodeError> {
        let idx = self
            .edges
            .iter()
            .position(|e| {
                e.from_node == from_node
                    && e.from_socket == from_socket
                    && e.to_node == to_node
                    && e.to_socket == to_socket
            })
            .ok_or(NodeError::NotConnected {
                from_node,
                from_socket: self.output_socket_label(from_node, from_socket),
                to_node,
                to_socket: self.input_socket_label(to_node, to_socket)
            })?;
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

    /// Human-readable name of the output socket at `socket` on `node_id`, falling back to its
    /// numeric index if the node or socket no longer exists.
    fn output_socket_label(&self, node_id: GraphNodeId, socket: usize) -> String {
        self.entry(node_id)
            .ok()
            .and_then(|entry| entry.instance.outputs().get(socket))
            .map(|s| s.name.to_string())
            .unwrap_or_else(|| socket.to_string())
    }

    /// Human-readable name of the input socket at `socket` on `node_id`, falling back to its
    /// numeric index if the node or socket no longer exists.
    fn input_socket_label(&self, node_id: GraphNodeId, socket: usize) -> String {
        self.entry(node_id)
            .ok()
            .and_then(|entry| entry.instance.inputs().get(socket))
            .map(|s| s.name.to_string())
            .unwrap_or_else(|| socket.to_string())
    }

    fn entry(&self, id: GraphNodeId) -> Result<&NodeEntry, NodeError> {
        self.nodes
            .get(id.0)
            .and_then(Option::as_ref)
            .ok_or(NodeError::NodeNotFound(id))
    }

    fn entry_mut(&mut self, id: GraphNodeId) -> Result<&mut NodeEntry, NodeError> {
        self.nodes
            .get_mut(id.0)
            .and_then(Option::as_mut)
            .ok_or(NodeError::NodeNotFound(id))
    }
}
