//! Shared test helpers for driving the async `NodeGraph::get` API from a plain `#[test]` fn.

use std::sync::Arc;

use bevy::tasks::{AsyncComputeTaskPool, TaskPool};
use waterdrop_terrain_generator::core::node::{
    NParamConstraints, NParamDesc, NParamValue, Node, NodeCategory, NodeError, NodeIcon,
    NodePortType, NodeSocket, ParamUnit, SocketDtype
};
use waterdrop_terrain_generator::core::{TileContext, TileHandle, TilePool};

/// `NodeGraph::get` spawns chunk jobs on Bevy's async compute task pool, which panics if used
/// before it's initialized. Idempotent, so every test that calls `get` can call this unconditionally.
pub fn init_task_pool() {
    AsyncComputeTaskPool::get_or_init(TaskPool::new);
}

/// Repeatedly calls `f` (e.g. `|| graph.get(id)`) until it returns `Ok(Some(_))` or `Err(_)`.
pub fn poll_until_ready<T, E>(mut f: impl FnMut() -> Result<Option<T>, E>) -> Result<T, E> {
    for _ in 0..1_000_000 {
        if let Some(value) = f()? {
            return Ok(value);
        }
        std::thread::yield_now();
    }
    panic!("timed out waiting for graph evaluation to finish");
}

/// A single-input/single-output `Local` node with a 3-texel kernel (like the since-removed
/// `Erosion` node) and a settable `strength` param - stands in for a real kernel-padding node in
/// tests that exercise padding/tile-size math or generic param mutation, not any real algorithm.
#[derive(Debug, Default, Clone)]
pub struct FakeKernelNode {
    strength: f32
}
impl Node for FakeKernelNode {
    fn label(&self) -> &str {
        "Fake Kernel Node"
    }
    fn category(&self) -> NodeCategory {
        NodeCategory::Modification
    }
    fn icon(&self) -> NodeIcon {
        NodeIcon {
            id: "test-fake-kernel",
            png_bytes: &[]
        }
    }
    fn size(&self) -> usize {
        3
    }
    fn inputs(&self) -> &[NodeSocket] {
        &[NodeSocket {
            name: "Height",
            dtype: SocketDtype::Fixed(NodePortType::Height),
            required: true
        }]
    }
    fn outputs(&self) -> &[NodeSocket] {
        &[NodeSocket {
            name: "Height",
            dtype: SocketDtype::Fixed(NodePortType::Height),
            required: true
        }]
    }
    fn desc_params(&self) -> &[NParamDesc] {
        &[NParamDesc {
            key: "strength",
            label: "Strength",
            category: "Modification",
            default: NParamValue::Float(1.0),
            constraints: Some(NParamConstraints::FloatRange { min: 0.0, max: 1.0 }),
            unit: ParamUnit::None
        }]
    }
    fn get_param(&self, key: &str) -> Option<NParamValue> {
        (key == "strength").then_some(NParamValue::Float(self.strength))
    }
    fn set_param(&mut self, key: &str, value: NParamValue) -> Result<(), NodeError> {
        match (key, value) {
            ("strength", NParamValue::Float(v)) => {
                self.strength = v;
                Ok(())
            }
            _ => Err("Unknown parameter".into())
        }
    }
    fn process(
        &self,
        pool: &Arc<TilePool>,
        inputs: &[TileHandle],
        _ctx: &TileContext
    ) -> Result<Vec<TileHandle>, NodeError> {
        let mut output = pool.allocate();
        output.copy_from_slice(&inputs[0]);
        Ok(vec![Arc::new(output)])
    }
    fn clone_boxed(&self) -> Box<dyn Node> {
        Box::new(self.clone())
    }
}
