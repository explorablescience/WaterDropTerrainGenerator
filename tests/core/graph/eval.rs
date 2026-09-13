use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use waterdrop_terrain_generator::core::graph::{GraphNodeId, NodeGraph};
use waterdrop_terrain_generator::core::node::{
    NParamValue, Node, NodeCategory, NodeError, NodeIcon, NodeLocality, NodePortType, NodeSocket,
    SocketDtype
};
use waterdrop_terrain_generator::core::tiling::{
    ChunkCoord, ChunkGrid, ComputeTarget, TileContext
};
use waterdrop_terrain_generator::core::{CacheEntry, CacheUuid, TileHandle, TilePool};
use waterdrop_terrain_generator::nodes::*;

use crate::support::{FakeKernelNode, init_task_pool, poll_until_ready};

/// Drives `NodeGraph::get` to completion and, for a `Local` node, picks out one chunk's tiles - the
/// same ergonomics the old (now-removed) `NodeGraph::process_chunk(id, chunk)` offered. For a
/// `Global` node, `chunk` is irrelevant and ignored, mirroring how `CacheEntry::Global` is keyed.
fn process_chunk(
    graph: &mut NodeGraph,
    id: GraphNodeId,
    chunk: ChunkCoord
) -> Result<(CacheUuid, Vec<TileHandle>), NodeError> {
    init_task_pool();
    let entry = poll_until_ready(|| graph.get(id))?;
    Ok(match entry {
        CacheEntry::Local(mut map) => map
            .remove(&chunk)
            .unwrap_or_else(|| panic!("{:?} should be cached after processing", chunk)),
        CacheEntry::Global(uuid, tiles) => (uuid, tiles)
    })
}

/// `ChunkGrid::new`'s `world_scale` argument is a user-facing slider value (range `0..=2`,
/// default `1`) that `ChunkGrid` scales down internally before it becomes actual world units per
/// texel - pass this instead of `1.0` to get exactly 1 world unit per texel, so a test's expected
/// world-space positions can be written as plain, round numbers.
const WORLD_UNIT_PER_TEXEL: f32 = 20.0;

#[test]
fn test_node_graph_connections() {
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 32, 1.0 / 32.0));
    let (node_a, node_b) = (
        graph.add_node(Box::new(Perlin::default())),
        graph.add_node(Box::new(FakeKernelNode::default()))
    );

    // Valid connection
    let result = graph.connect(node_a, 0, node_b, 0);
    assert!(result.is_ok(), "Graph connection should succeed");

    // Invalid connection: NodeErosion has only one input socket (index 0)
    let result_invalid = graph.connect(node_a, 0, node_b, 1);
    assert!(
        result_invalid.is_err(),
        "Graph connection should fail due to invalid socket index"
    );
}

#[test]
fn test_node_graph_validation() {
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 32, 1.0 / 32.0));
    let (node_a, node_b, node_c) = (
        graph.add_node(Box::new(Perlin::default())),
        graph.add_node(Box::new(FakeKernelNode::default())),
        graph.add_node(Box::new(FakeKernelNode::default()))
    );
    graph
        .connect(node_a, 0, node_b, 0)
        .and_then(|g| g.connect(node_b, 0, node_c, 0))
        .expect("Graph connections should succeed");
}

#[test]
fn test_node_graph_cycle_detection() {
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 32, 1.0 / 32.0));
    let (node_a, node_b) = (
        graph.add_node(Box::new(FakeKernelNode::default())),
        graph.add_node(Box::new(FakeKernelNode::default()))
    );
    graph
        .connect(node_a, 0, node_b, 0)
        .and_then(|g| g.connect(node_b, 0, node_a, 0)) // This creates a cycle
        .expect("Graph connections should succeed");

    // Run the graph and expect a cycle detection error
    let result = graph.get(node_b);
    assert!(result.is_err(), "Graph validation should fail due to cycle");
}

#[test]
fn test_node_graph_remove_node_disconnects_edges() {
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 32, 1.0 / 32.0));
    let (source, erosion) = (
        graph.add_node(Box::new(Flat)),
        graph.add_node(Box::new(FakeKernelNode::default()))
    );
    graph
        .connect(source, 0, erosion, 0)
        .expect("Graph connection should succeed");

    graph
        .remove_node(source)
        .expect("Removing an existing node should succeed");

    // The dangling input edge should be gone, so erosion now has an unconnected input.
    let result = graph.get(erosion);
    assert!(
        result.is_err(),
        "Processing should fail once the upstream node feeding erosion is removed"
    );

    // The node itself is gone too.
    assert!(
        graph.node(source).is_err(),
        "Removed node should no longer be reachable"
    );
}

#[test]
fn test_node_graph_remove_node_resets_cached_topo() {
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 32, 1.0 / 32.0));
    let (source, erosion) = (
        graph.add_node(Box::new(Flat)),
        graph.add_node(Box::new(FakeKernelNode::default()))
    );
    graph
        .connect(source, 0, erosion, 0)
        .expect("Graph connection should succeed");

    graph
        .remove_node(source)
        .expect("Removing an existing node should succeed");

    // The cached topo order from before the removal must not be reused.
    let result = graph.get(erosion);
    assert!(
        result.is_err(),
        "Processing should require re-validation after a node is removed"
    );
}

#[test]
fn test_node_graph_remove_node_unknown_id_errors() {
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 32, 1.0 / 32.0));
    let node = graph.add_node(Box::new(Flat));
    graph
        .remove_node(node)
        .expect("First removal should succeed");

    let result = graph.remove_node(node);
    assert!(
        result.is_err(),
        "Removing an already-removed node should fail"
    );
}

#[test]
fn test_node_graph_process_grows_internal_tile_size_for_padding() {
    let tile_size = 8;
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 8, 1.0 / 8.0));
    let (source, erosion) = (
        graph.add_node(Box::new(Flat)),
        graph.add_node(Box::new(FakeKernelNode::default())) // size() == 3 -> padding of 2
    );
    graph
        .connect(source, 0, erosion, 0)
        .expect("Graph connection should succeed");

    let (_, outputs) = process_chunk(&mut graph, erosion, ChunkCoord(0, 0))
        .expect("Graph processing should succeed");

    // The graph pads the working tile internally to give `Erosion`'s 3x3 kernel valid neighbors
    // at the chunk's own edges, but crops back down to the chunk's own tile size before handing
    // the result back out - callers never see the padding margin.
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].len(), tile_size * tile_size);

    // The padding is still visible in what the arena reserved: `TilePool::from_arena` eagerly
    // reserves a bucket of tiles (16, `pool.rs::INITIAL_POOL_CAPACITY`) the first time a given
    // size is requested, and an arena's bucket is never shrunk back down. Two buckets end up
    // reserved: `NodeGraph::new` itself prefills one at the chunk grid's own tile size (8, before
    // any padding), and processing `erosion` then needs a second, bigger one for its padded
    // working size (tile_size + 2*padding(erosion) + 2*padding(source) = 8 + 2*2 + 2*0 = 12).
    const PREFILL: usize = 16;
    let internal_tile_size = tile_size + 2 * 2;
    let expected_bytes = PREFILL * tile_size * tile_size * std::mem::size_of::<f32>()
        + PREFILL * internal_tile_size * internal_tile_size * std::mem::size_of::<f32>();
    assert_eq!(graph.allocated_bytes(), expected_bytes);
}

#[test]
fn node_mut_lets_callers_mutate_a_node_in_place() {
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 4, 1.0 / 4.0));
    let id = graph.add_node(Box::new(FakeKernelNode::default()));
    graph
        .node_mut(id)
        .unwrap()
        .set_param("strength", NParamValue::Float(0.1))
        .unwrap();

    assert_eq!(
        graph.node(id).unwrap().get_param("strength"),
        Some(NParamValue::Float(0.1))
    );
}

#[test]
fn node_mut_on_an_unknown_id_fails_without_mutating_anything() {
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 4, 1.0 / 4.0));
    let id = graph.add_node(Box::new(Flat));
    graph.remove_node(id).unwrap();
    assert!(graph.node_mut(id).is_err());
}

#[test]
fn mutating_a_node_invalidates_its_own_and_downstream_cached_output() {
    // `Perlin` dispatches to the GPU by default, which has no render world to talk to in a plain
    // `cargo test` process - force CPU so its `process` actually runs here.
    let mut graph =
        NodeGraph::new(ChunkGrid::new(1, 1, 8, 1.0 / 8.0).with_compute_target(ComputeTarget::Cpu));
    let source = graph.add_node(Box::new(Perlin::default()));
    let sink = graph.add_node(Box::new(FakeKernelNode::default()));
    graph.connect(source, 0, sink, 0).unwrap();

    let (first_uuid, _) = process_chunk(&mut graph, sink, ChunkCoord(0, 0)).unwrap();

    // Re-processing without any change should serve the cached result: the reported cache uuid
    // must not have advanced.
    let (cached_uuid, _) = process_chunk(&mut graph, sink, ChunkCoord(0, 0)).unwrap();
    assert_eq!(
        first_uuid, cached_uuid,
        "an unchanged graph should be served from cache"
    );

    // Changing the source's parameter should force both it and its downstream consumer to
    // recompute, advancing the cache uuid. This also covers a source that has since been evicted
    // from cache (its output no longer needed once its consumer was itself cached): eviction
    // leaves the source dirty too, so invalidation can't just skip a node for already being
    // dirty - it must still propagate to that node's own downstream.
    graph
        .node_mut(source)
        .unwrap()
        .set_param("frequency", NParamValue::Float(5.0))
        .unwrap();
    let (after_uuid, _) = process_chunk(&mut graph, sink, ChunkCoord(0, 0)).unwrap();
    assert!(
        after_uuid > cached_uuid,
        "changing an upstream parameter should force recomputation"
    );
}

#[test]
fn tile_size_reports_the_size_the_graph_was_created_with() {
    let graph = NodeGraph::new(ChunkGrid::new(1, 1, 16, 1.0 / 16.0));
    assert_eq!(graph.tile_size(), 16);
}

#[test]
fn is_processing_is_false_before_anything_has_been_computed() {
    let graph = NodeGraph::new(ChunkGrid::new(1, 1, 4, 1.0 / 4.0));
    assert!(!graph.is_processing());
}

const TEST_ICON: NodeIcon = NodeIcon {
    id: "test-icon",
    png_bytes: &[]
};

/// A node whose `process` deliberately takes a moment, so a test can reliably observe
/// `is_processing()` before the async task pool has had a chance to finish it - a trivial node's
/// near-instant compute can complete within the very same `get()` call that spawned it, making
/// that race nondeterministic.
#[derive(Debug, Default, Clone)]
struct FakeSlowSource;
impl Node for FakeSlowSource {
    fn label(&self) -> &str {
        "Fake Slow Source"
    }
    fn category(&self) -> NodeCategory {
        NodeCategory::Generation
    }
    fn icon(&self) -> NodeIcon {
        TEST_ICON
    }
    fn outputs(&self) -> &[NodeSocket] {
        &[NodeSocket {
            name: "Height",
            dtype: SocketDtype::Fixed(NodePortType::Height),
            required: true
        }]
    }
    fn process(
        &self,
        pool: &Arc<TilePool>,
        _inputs: &[TileHandle],
        _ctx: &TileContext
    ) -> Result<Vec<TileHandle>, NodeError> {
        std::thread::sleep(std::time::Duration::from_millis(50));
        Ok(vec![Arc::new(pool.allocate())])
    }
    fn clone_boxed(&self) -> Box<dyn Node> {
        Box::new(self.clone())
    }
}

#[test]
fn is_processing_is_true_immediately_after_a_node_is_computed() {
    init_task_pool();
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 4, 1.0 / 4.0));
    let id = graph.add_node(Box::new(FakeSlowSource));
    graph.get(id).expect("processing should succeed");
    assert!(graph.is_processing());
}

/// A node whose `process` counts how many times it actually runs (as opposed to being served
/// from cache), so a test can prove a `Global` node isn't recomputed once per chunk. Its output is
/// a constant field, so a test can also check that cropping/resampling it into a chunk-sized tile
/// doesn't corrupt the data.
#[derive(Debug, Clone)]
struct FakeGlobalSource {
    calls: Arc<AtomicUsize>,
    native_resolution: usize
}
impl Node for FakeGlobalSource {
    fn label(&self) -> &str {
        "Fake Global Source"
    }
    fn category(&self) -> NodeCategory {
        NodeCategory::Generation
    }
    fn icon(&self) -> NodeIcon {
        TEST_ICON
    }
    fn locality(&self) -> NodeLocality {
        NodeLocality::Global {
            native_resolution: self.native_resolution
        }
    }
    fn outputs(&self) -> &[NodeSocket] {
        &[NodeSocket {
            name: "Height",
            dtype: SocketDtype::Fixed(NodePortType::Height),
            required: true
        }]
    }
    fn process(
        &self,
        pool: &Arc<TilePool>,
        _inputs: &[TileHandle],
        _ctx: &TileContext
    ) -> Result<Vec<TileHandle>, NodeError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let mut output = pool.allocate();
        output.iter_mut().for_each(|v| *v = 1.0);
        Ok(vec![Arc::new(output)])
    }
    fn clone_boxed(&self) -> Box<dyn Node> {
        Box::new(self.clone())
    }
}

/// A node whose `process` counts how many times it actually runs, used to distinguish "served
/// from cache" from "recomputed".
#[derive(Debug, Clone)]
struct FakeCountingSource {
    calls: Arc<AtomicUsize>
}
impl Node for FakeCountingSource {
    fn label(&self) -> &str {
        "Fake Counting Source"
    }
    fn category(&self) -> NodeCategory {
        NodeCategory::Generation
    }
    fn icon(&self) -> NodeIcon {
        TEST_ICON
    }
    fn outputs(&self) -> &[NodeSocket] {
        &[NodeSocket {
            name: "Height",
            dtype: SocketDtype::Fixed(NodePortType::Height),
            required: true
        }]
    }
    fn process(
        &self,
        pool: &Arc<TilePool>,
        _inputs: &[TileHandle],
        _ctx: &TileContext
    ) -> Result<Vec<TileHandle>, NodeError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(vec![Arc::new(pool.allocate())])
    }
    fn clone_boxed(&self) -> Box<dyn Node> {
        Box::new(self.clone())
    }
}

#[test]
fn a_global_nodes_local_ancestor_is_recomputed_when_the_globals_own_resolution_changes() {
    // Regression test: `Perlin` (`Local`) promoted into `HydraulicErosion`'s (`Global`) frame used
    // to be cached under a single `EvalScope::Global` shared by every resolution. Changing
    // `HydraulicErosion`'s own `native_resolution` correctly invalidated *its own* cache
    // (`mark_dirty` propagates downstream from it), but never touched `Perlin`'s stale,
    // differently-sized cached tile, since `Perlin` is upstream of it - `HydraulicErosion` would
    // then try to build its new-sized output from that stale input and panic.
    // `Perlin` dispatches to the GPU by default, which has no render world to talk to in a plain
    // `cargo test` process - force CPU so its `process` actually runs here.
    let mut graph =
        NodeGraph::new(ChunkGrid::new(1, 1, 4, 1.0).with_compute_target(ComputeTarget::Cpu));
    let source = graph.add_node(Box::new(Perlin::default()));
    let erosion = graph.add_node(Box::new(HydraulicErosion {
        droplets: 0, // isolate the caching bug from the simulation itself
        native_resolution: 8,
        ..HydraulicErosion::default()
    }));
    graph.connect(source, 0, erosion, 0).unwrap();
    process_chunk(&mut graph, erosion, ChunkCoord(0, 0)).unwrap();

    graph
        .node_mut(erosion)
        .unwrap()
        .set_param("native_resolution", NParamValue::Int(16))
        .unwrap();

    let (_, tiles) = process_chunk(&mut graph, erosion, ChunkCoord(0, 0)).unwrap();
    assert_eq!(
        tiles[0].size(),
        16,
        "the resized global pass should rebuild its promoted local ancestor at the new resolution"
    );
}

/// A minimal `Local` node that forwards its input unchanged, used to observe exactly what a
/// `Global` ancestor's output looks like once resampled into a `Local` consumer's own frame.
#[derive(Debug, Default, Clone)]
struct FakePassthroughSink;
impl Node for FakePassthroughSink {
    fn label(&self) -> &str {
        "Fake Passthrough Sink"
    }
    fn category(&self) -> NodeCategory {
        NodeCategory::Modification
    }
    fn icon(&self) -> NodeIcon {
        TEST_ICON
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
    fn process(
        &self,
        _pool: &Arc<TilePool>,
        inputs: &[TileHandle],
        _ctx: &TileContext
    ) -> Result<Vec<TileHandle>, NodeError> {
        Ok(vec![inputs[0].clone()])
    }
    fn clone_boxed(&self) -> Box<dyn Node> {
        Box::new(self.clone())
    }
}

#[test]
fn requesting_the_same_chunk_twice_hits_the_per_chunk_cache() {
    // A single-chunk grid: `NodeGraph::get` processes every chunk of a dirty node together (see
    // `Processor::spawn_local`), so a multi-chunk grid would already show more than one call after
    // just the first request, which isn't what this test is isolating.
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 4, 1.0));
    let calls = Arc::new(AtomicUsize::new(0));
    let id = graph.add_node(Box::new(FakeCountingSource {
        calls: calls.clone()
    }));

    process_chunk(&mut graph, id, ChunkCoord(0, 0)).unwrap();
    process_chunk(&mut graph, id, ChunkCoord(0, 0)).unwrap();

    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "the second request for the same chunk should be served from cache"
    );
}

#[test]
fn different_chunks_of_the_same_node_are_computed_and_cached_independently() {
    let mut graph = NodeGraph::new(ChunkGrid::new(2, 1, 4, 1.0));
    let calls = Arc::new(AtomicUsize::new(0));
    let id = graph.add_node(Box::new(FakeCountingSource {
        calls: calls.clone()
    }));

    process_chunk(&mut graph, id, ChunkCoord(0, 0)).unwrap();
    process_chunk(&mut graph, id, ChunkCoord(1, 0)).unwrap();
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "each distinct chunk should be computed once"
    );

    // Re-requesting chunk 0 should still be cached, independent of chunk 1 having since run.
    process_chunk(&mut graph, id, ChunkCoord(0, 0)).unwrap();
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "chunk 0's own cache should be unaffected by chunk 1 being computed"
    );
}

/// A minimal `Local` node whose output is exactly each texel's own world x position. Used instead
/// of a real generator node (e.g. `Perlin`) to check chunk coordinate framing (a `core/tiling`
/// concern) without coupling this test to a specific node's own noise formula, which lives in
/// `nodes/` and is outside this test suite's coverage.
#[derive(Debug, Default, Clone)]
struct FakeLocalWorldXMarker;
impl Node for FakeLocalWorldXMarker {
    fn label(&self) -> &str {
        "Fake Local World X Marker"
    }
    fn category(&self) -> NodeCategory {
        NodeCategory::Generation
    }
    fn icon(&self) -> NodeIcon {
        TEST_ICON
    }
    fn outputs(&self) -> &[NodeSocket] {
        &[NodeSocket {
            name: "Height",
            dtype: SocketDtype::Fixed(NodePortType::Height),
            required: true
        }]
    }
    fn process(
        &self,
        pool: &Arc<TilePool>,
        _inputs: &[TileHandle],
        ctx: &TileContext
    ) -> Result<Vec<TileHandle>, NodeError> {
        let mut output = pool.allocate();
        let s = output.size();
        for y in 0..s {
            for x in 0..s {
                output[y * s + x] = ctx.world_pos(x, y).0;
            }
        }
        Ok(vec![Arc::new(output)])
    }
    fn clone_boxed(&self) -> Box<dyn Node> {
        Box::new(self.clone())
    }
}

#[test]
fn local_nodes_sample_a_shared_world_coordinate_frame_across_chunks() {
    // 1 world unit per texel, so chunk 1 begins exactly where chunk 0's 4 world units end.
    let mut graph = NodeGraph::new(ChunkGrid::new(2, 1, 4, WORLD_UNIT_PER_TEXEL));
    let marker = graph.add_node(Box::new(FakeLocalWorldXMarker));

    let (_, tiles) = process_chunk(&mut graph, marker, ChunkCoord(1, 0)).unwrap();
    let chunk1 = tiles[0].clone();

    let s = chunk1.size();
    assert_eq!(s, 4, "no downstream kernel node means no margin padding");
    // World space is centered on the whole grid: chunk 0 spans world x in [-4, 0) and chunk 1
    // spans [0, 4), so chunk 1's texel (0, 0) sits at world x 0 - continuing chunk 0's span
    // rather than restarting it at a tile-local 0.
    assert!(
        (chunk1[0] - 0.0).abs() < 1e-5,
        "chunk 1's first texel should sample world x 0, continuing chunk 0's span"
    );
    assert!(
        (chunk1[s - 1] - 3.0).abs() < 1e-5,
        "chunk 1's last texel in the row should sample world x 3"
    );
}

#[test]
fn a_global_node_is_evaluated_once_regardless_of_how_many_chunks_request_it() {
    // A `Global` ancestor's output is resampled fresh for each requesting chunk, but its own
    // whole-terrain pass is only ever computed once and reused - resampling doesn't retrigger it.
    let mut graph = NodeGraph::new(ChunkGrid::new(2, 1, 4, 1.0));
    let calls = Arc::new(AtomicUsize::new(0));
    let source = graph.add_node(Box::new(FakeGlobalSource {
        calls: calls.clone(),
        native_resolution: 6
    }));
    let erosion = graph.add_node(Box::new(FakeKernelNode::default()));
    graph.connect(source, 0, erosion, 0).unwrap();

    process_chunk(&mut graph, erosion, ChunkCoord(0, 0)).unwrap();
    process_chunk(&mut graph, erosion, ChunkCoord(1, 0)).unwrap();
    // Re-requesting a chunk already computed should also not touch the global node again.
    process_chunk(&mut graph, erosion, ChunkCoord(0, 0)).unwrap();

    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "the global node's whole-terrain pass should be memoized across every chunk"
    );
}

#[test]
fn a_global_ancestors_output_is_automatically_resampled_into_a_local_consumers_real_world_frame() {
    // Connecting a `Global` node straight into a `Local` node needs no manual placement step: the
    // graph engine resamples the `Global` ancestor's whole-terrain tile into the `Local` node's
    // own chunk frame automatically, in real world coordinates on both sides.
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 4, WORLD_UNIT_PER_TEXEL));
    let source = graph.add_node(Box::new(FakeGlobalWorldXMarker {
        native_resolution: 8
    }));
    let sink = graph.add_node(Box::new(FakePassthroughSink));
    graph.connect(source, 0, sink, 0).unwrap();

    let (_, tiles) = process_chunk(&mut graph, sink, ChunkCoord(0, 0)).unwrap();

    // `source`'s value at any point is exactly its own world x - an exactly linear function, which
    // bilinear resampling reproduces everywhere regardless of `native_resolution` - so the sink
    // should read back precisely its own chunk's real world x positions, not source's raw texels.
    let s = tiles[0].size();
    assert_eq!(s, 4);
    for y in 0..s {
        for x in 0..s {
            let expected = -2.0 + x as f32; // this single chunk's core spans world x in [-2, 2)
            let got = tiles[0][y * s + x];
            assert!(
                (got - expected).abs() < 1e-4,
                "texel ({}, {}): expected world x {}, got {}",
                x,
                y,
                expected,
                got
            );
        }
    }
}

#[test]
fn a_global_ancestors_resampled_output_is_independent_of_its_own_native_resolution() {
    // Regression test: a `Global` source's own `native_resolution` should only pick how finely its
    // whole-terrain tile is sampled, never the real-world position its values land at once
    // resampled into a `Local` consumer. `FakeGlobalWorldXMarker`'s value is an exactly linear
    // function of world x, and bilinear interpolation reproduces an exactly linear function
    // everywhere regardless of how coarse or fine the source grid is - so if two sources at
    // different resolutions disagree anywhere once resampled, resolution must be leaking through.
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 4, 1.0));
    let low_res = graph.add_node(Box::new(FakeGlobalWorldXMarker {
        native_resolution: 4
    }));
    let high_res = graph.add_node(Box::new(FakeGlobalWorldXMarker {
        native_resolution: 64
    }));
    let sink_low = graph.add_node(Box::new(FakePassthroughSink));
    let sink_high = graph.add_node(Box::new(FakePassthroughSink));
    graph.connect(low_res, 0, sink_low, 0).unwrap();
    graph.connect(high_res, 0, sink_high, 0).unwrap();

    let (_, low_tiles) = process_chunk(&mut graph, sink_low, ChunkCoord(0, 0)).unwrap();
    let low = low_tiles[0].clone();
    let (_, high_tiles) = process_chunk(&mut graph, sink_high, ChunkCoord(0, 0)).unwrap();
    let high = high_tiles[0].clone();

    for i in 0..low.size() * low.size() {
        assert!(
            (low[i] - high[i]).abs() < 1e-4,
            "texel {} differs between a 4-texel and a 64-texel global source ({} vs {}) - \
             native_resolution must be leaking into the resampled position",
            i,
            low[i],
            high[i]
        );
    }
}

/// A `Global` node whose output encodes each texel's own world x position, so a test can check
/// exactly how it gets sampled (directly, or resampled into a `Local` consumer).
#[derive(Debug, Clone)]
struct FakeGlobalWorldXMarker {
    native_resolution: usize
}
impl Node for FakeGlobalWorldXMarker {
    fn label(&self) -> &str {
        "Fake Global World X Marker"
    }
    fn category(&self) -> NodeCategory {
        NodeCategory::Generation
    }
    fn icon(&self) -> NodeIcon {
        TEST_ICON
    }
    fn locality(&self) -> NodeLocality {
        NodeLocality::Global {
            native_resolution: self.native_resolution
        }
    }
    fn outputs(&self) -> &[NodeSocket] {
        &[NodeSocket {
            name: "Height",
            dtype: SocketDtype::Fixed(NodePortType::Height),
            required: true
        }]
    }
    fn process(
        &self,
        pool: &Arc<TilePool>,
        _inputs: &[TileHandle],
        ctx: &TileContext
    ) -> Result<Vec<TileHandle>, NodeError> {
        let mut output = pool.allocate();
        let s = output.size();
        for y in 0..s {
            for x in 0..s {
                output[y * s + x] = ctx.world_pos(x, y).0;
            }
        }
        Ok(vec![Arc::new(output)])
    }
    fn clone_boxed(&self) -> Box<dyn Node> {
        Box::new(self.clone())
    }
}

#[test]
fn a_directly_requested_global_node_always_returns_its_own_whole_terrain_result() {
    // A `Global` node evaluates the whole terrain in one pass regardless of which chunk asked for
    // it - previewing or exporting it directly always shows that same single result.
    let mut graph = NodeGraph::new(ChunkGrid::new(2, 1, 4, 1.0));
    let source = graph.add_node(Box::new(FakeGlobalWorldXMarker {
        native_resolution: 8
    }));

    let (_, chunk0_tiles) = process_chunk(&mut graph, source, ChunkCoord(0, 0)).unwrap();
    let chunk0 = chunk0_tiles[0].clone();
    let (_, chunk1_tiles) = process_chunk(&mut graph, source, ChunkCoord(1, 0)).unwrap();
    let chunk1 = chunk1_tiles[0].clone();

    assert_eq!(
        chunk0.to_vec(),
        chunk1.to_vec(),
        "which chunk asked for it shouldn't matter - a global node's whole-terrain result is the same either way"
    );

    // Its frame is centered on the terrain's own world origin: the middle texel of an 8-wide
    // buffer (index 4) sits right at world x = 0.
    let s = chunk0.size();
    assert_eq!(s, 8);
    assert!(
        chunk0[4].abs() < 1e-6,
        "the center of a global node's bare result should sit at local (0, 0)"
    );
}

#[test]
fn cached_bytes_totals_local_chunk_tiles_and_global_tiles_together() {
    // Regression test: the footer's memory stat used to read `pool().allocated_bytes()`, which
    // only ever reflected the shared *chunk* pool - so a `Global` node's own (differently-sized)
    // buffer never counted at all, and the number reported depended on whichever node happened to
    // be processed last rather than being a real total.
    //
    // `TilePool::from_arena` eagerly reserves a bucket of 16 tiles (`INITIAL_POOL_CAPACITY`) the
    // first time a given size is requested from an arena, and a bucket is never shrunk back down -
    // so each arena's `allocated_bytes` is exactly `16 * size^2 * 4` per distinct size it has ever
    // been asked for, not just whatever's currently cached.
    const PREFILL: usize = 16;
    let mut graph = NodeGraph::new(ChunkGrid::new(2, 1, 4, 1.0));
    let local = graph.add_node(Box::new(Flat));
    let global = graph.add_node(Box::new(FakeGlobalSource {
        calls: Arc::new(AtomicUsize::new(0)),
        native_resolution: 6
    }));

    process_chunk(&mut graph, local, ChunkCoord(0, 0)).unwrap();
    let expected_local = PREFILL * 4 * 4 * std::mem::size_of::<f32>(); // tile_size = 4, no padding
    assert_eq!(graph.allocated_bytes(), expected_local);

    process_chunk(&mut graph, global, ChunkCoord(0, 0)).unwrap();
    let expected_global = PREFILL * 6 * 6 * std::mem::size_of::<f32>(); // native_resolution = 6
    assert_eq!(
        graph.allocated_bytes(),
        expected_local + expected_global,
        "the global node's own whole-terrain buffer should add to the total alongside the local \
         chunk's, not replace it or be left out"
    );
}

#[test]
fn cached_bytes_accumulates_every_pool_size_a_selection_change_has_ever_required() {
    // Regression test note: this used to check that resizing the pool for a bigger selection
    // evicted the smaller size's footprint entirely. That's no longer how the arena works -
    // `TileArena` never shrinks a size bucket once reserved (see `pool.rs`'s doc comment), so
    // switching to a node that needs a bigger padded tile *adds* a new bucket rather than
    // replacing the old one.
    const PREFILL: usize = 16;
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 4, 1.0));
    let flat = graph.add_node(Box::new(Flat)); // no kernel -> internal tile size 4
    let erosion = graph.add_node(Box::new(FakeKernelNode::default())); // padding 2 -> internal tile size 8
    graph.connect(flat, 0, erosion, 0).unwrap();

    process_chunk(&mut graph, flat, ChunkCoord(0, 0)).unwrap();
    let size4_bytes = PREFILL * 4 * 4 * std::mem::size_of::<f32>();
    assert_eq!(graph.allocated_bytes(), size4_bytes);

    // Selecting `erosion` needs a bigger padded pool (size 8), which reserves a second bucket
    // alongside the first rather than replacing it.
    process_chunk(&mut graph, erosion, ChunkCoord(0, 0)).unwrap();
    let size8_bytes = PREFILL * 8 * 8 * std::mem::size_of::<f32>();
    assert_eq!(
        graph.allocated_bytes(),
        size4_bytes + size8_bytes,
        "the arena keeps every size bucket it has ever reserved, so both sizes should still count"
    );
}
