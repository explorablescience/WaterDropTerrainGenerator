use std::collections::HashSet;

use crate::core::node::{Node, NodeError, NodeLocality, NodePortType, SocketDtype};

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
        let from_label = from_socket_desc.name;

        let to_socket_desc = self
            .entry(to_node)?
            .instance
            .inputs()
            .get(to_socket)
            .ok_or(NodeError::InputSocketNotFound {
                node: format!("{:?}", to_node),
                socket: to_socket
            })?;
        let to_label = to_socket_desc.name;

        let from_dtype =
            self.resolve_socket_dtype(from_node, from_socket, false, &mut HashSet::new());
        let to_dtype = self.resolve_socket_dtype(to_node, to_socket, true, &mut HashSet::new());
        if let (Some(f), Some(t)) = (from_dtype, to_dtype)
            && f != t
        {
            return Err(NodeError::SocketTypeMismatch {
                from_node: format!("{:?}", from_node),
                from_socket: from_label.to_string(),
                to_node: format!("{:?}", to_node),
                to_socket: to_label.to_string()
            });
        }
        // No locality check needed here
        // An input pin can only ever hold one connection: replace whatever was already plugged into it.
        let existing = self.entry(to_node)?.inputs[to_socket];
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

    /// Every edge currently in the graph, as `(from_node, from_socket, to_node, to_socket)`.
    pub fn edges(&self) -> impl Iterator<Item = (GraphNodeId, usize, GraphNodeId, usize)> + '_ {
        self.edges
            .iter()
            .map(|e| (e.from_node, e.from_socket, e.to_node, e.to_socket))
    }

    /// Ancestors of `node_id`, including itself; stops expanding through a `Global` ancestor.
    pub(crate) fn collect_ancestors(
        &self,
        node_id: GraphNodeId
    ) -> Result<HashSet<GraphNodeId>, NodeError> {
        self.node(node_id)?;
        let mut seen = HashSet::from([node_id]);
        let mut stack = vec![node_id];
        while let Some(id) = stack.pop() {
            if id != node_id && matches!(self.node(id)?.locality(), NodeLocality::Global { .. }) {
                continue;
            }
            for (from_node, _) in self.inputs(id)?.iter().flatten() {
                if seen.insert(*from_node) {
                    stack.push(*from_node);
                }
            }
        }
        Ok(seen)
    }

    /// Falls back to the numeric index if the node or socket no longer exists.
    fn output_socket_label(&self, node_id: GraphNodeId, socket: usize) -> String {
        self.entry(node_id)
            .ok()
            .and_then(|entry| entry.instance.outputs().get(socket))
            .map(|s| s.name.to_string())
            .unwrap_or_else(|| socket.to_string())
    }

    /// Falls back to the numeric index if the node or socket no longer exists.
    fn input_socket_label(&self, node_id: GraphNodeId, socket: usize) -> String {
        self.entry(node_id)
            .ok()
            .and_then(|entry| entry.instance.inputs().get(socket))
            .map(|s| s.name.to_string())
            .unwrap_or_else(|| socket.to_string())
    }

    /// The concrete `NodePortType` an output socket resolves to - immediate for `Fixed`, or
    /// derived from wiring for `Generic` (e.g. `Combine`'s output once both its inputs are wired).
    pub fn output_dtype(&self, node_id: GraphNodeId, socket: usize) -> Option<NodePortType> {
        self.resolve_socket_dtype(node_id, socket, false, &mut HashSet::new())
    }

    /// `visited` guards against infinite recursion through a cycle of all-`Generic` nodes.
    fn resolve_socket_dtype(
        &self,
        node_id: GraphNodeId,
        socket: usize,
        is_input: bool,
        visited: &mut HashSet<(GraphNodeId, bool, usize)>
    ) -> Option<NodePortType> {
        let entry = self.entry(node_id).ok()?;
        let dtype = if is_input {
            entry.instance.inputs().get(socket)?.dtype
        } else {
            entry.instance.outputs().get(socket)?.dtype
        };
        match dtype {
            SocketDtype::Fixed(t) => Some(t),
            SocketDtype::Generic => {
                if !visited.insert((node_id, is_input, socket)) {
                    return None;
                }
                self.resolve_generic_group_dtype(node_id, visited)
            }
        }
    }

    fn resolve_generic_group_dtype(
        &self,
        node_id: GraphNodeId,
        visited: &mut HashSet<(GraphNodeId, bool, usize)>
    ) -> Option<NodePortType> {
        let entry = self.entry(node_id).ok()?;
        for (i, socket) in entry.instance.inputs().iter().enumerate() {
            if socket.dtype != SocketDtype::Generic {
                continue;
            }
            if let Some((from_node, from_socket)) = entry.inputs[i]
                && let Some(t) = self.resolve_socket_dtype(from_node, from_socket, false, visited)
            {
                return Some(t);
            }
        }
        for (i, socket) in entry.instance.outputs().iter().enumerate() {
            if socket.dtype != SocketDtype::Generic {
                continue;
            }
            let downstream = self
                .edges
                .iter()
                .filter(|e| e.from_node == node_id && e.from_socket == i)
                .map(|e| (e.to_node, e.to_socket));
            for (to_node, to_socket) in downstream {
                if let Some(t) = self.resolve_socket_dtype(to_node, to_socket, true, visited) {
                    return Some(t);
                }
            }
        }
        None
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
