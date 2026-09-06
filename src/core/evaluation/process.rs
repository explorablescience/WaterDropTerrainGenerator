use crate::core::*;

pub struct Processor {
}
impl Processor {
    pub fn new() -> Self {
        Self {
        }
    }

    pub fn process(
        &mut self,
        topology: &Topology,
        node_id: GraphNodeId,
    ) -> Result<CacheEntry, NodeError> {
        Err(NodeError::CyclicGraph)
    }
}
