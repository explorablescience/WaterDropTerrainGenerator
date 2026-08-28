use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use waterdrop_terrain_generator::core::graph::{NodeGraph, NodeGraphProcessResult};
use waterdrop_terrain_generator::core::node::{
    NParamValue, Node, NodeCategory, NodeError, NodeIcon, NodeLocality, NodePortType, NodeSocket
};
use waterdrop_terrain_generator::core::tiling::{
    ChunkCoord, ChunkGrid, TileContext, TileHandle, TilePool
};
use waterdrop_terrain_generator::nodes::*;

#[test]
fn test_node_graph_connections() {
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 32, 1.0 / 32 as f32));
    let (node_a, node_b) = (
        graph.add_node(Box::new(Perlin::default())),
        graph.add_node(Box::new(Erosion::default()))
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
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 32, 1.0 / 32 as f32));
    let (node_a, node_b, node_c) = (
        graph.add_node(Box::new(Perlin::default())),
        graph.add_node(Box::new(Erosion::default())),
        graph.add_node(Box::new(Erosion::default()))
    );
    graph
        .connect(node_a, 0, node_b, 0)
        .and_then(|g| g.connect(node_b, 0, node_c, 0))
        .expect("Graph connections should succeed");
}

#[test]
fn test_node_graph_cycle_detection() {
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 32, 1.0 / 32 as f32));
    let (node_a, node_b) = (
        graph.add_node(Box::new(Erosion::default())),
        graph.add_node(Box::new(Erosion::default()))
    );
    graph
        .connect(node_a, 0, node_b, 0)
        .and_then(|g| g.connect(node_b, 0, node_a, 0)) // This creates a cycle
        .expect("Graph connections should succeed");

    // Run the graph and expect a cycle detection error
    let result = graph.process(node_b);
    assert!(result.is_err(), "Graph validation should fail due to cycle");
}

#[test]
fn test_node_graph_remove_node_disconnects_edges() {
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 32, 1.0 / 32 as f32));
    let (source, erosion) = (
        graph.add_node(Box::new(Flat)),
        graph.add_node(Box::new(Erosion::default()))
    );
    graph
        .connect(source, 0, erosion, 0)
        .expect("Graph connection should succeed");

    graph
        .remove_node(source)
        .expect("Removing an existing node should succeed");

    // The dangling input edge should be gone, so erosion now has an unconnected input.
    let result = graph.process(erosion);
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
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 32, 1.0 / 32 as f32));
    let (source, erosion) = (
        graph.add_node(Box::new(Flat)),
        graph.add_node(Box::new(Erosion::default()))
    );
    graph
        .connect(source, 0, erosion, 0)
        .expect("Graph connection should succeed");

    graph
        .remove_node(source)
        .expect("Removing an existing node should succeed");

    // The cached topo order from before the removal must not be reused.
    let result = graph.process(erosion);
    assert!(
        result.is_err(),
        "Processing should require re-validation after a node is removed"
    );
}

#[test]
fn test_node_graph_remove_node_unknown_id_errors() {
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 32, 1.0 / 32 as f32));
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
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 8, 1.0 / 8 as f32));
    let (source, erosion) = (
        graph.add_node(Box::new(Flat)),
        graph.add_node(Box::new(Erosion::default())) // size() == 3 -> padding of 2
    );
    graph
        .connect(source, 0, erosion, 0)
        .expect("Graph connection should succeed");

    let outputs = graph
        .process(erosion)
        .expect("Graph processing should succeed");
    let outputs = match outputs {
        NodeGraphProcessResult::Processed(_, outputs) => outputs,
        _ => panic!("Graph processing should have completed")
    };

    // internal_tile_size = tile_size + 2*padding(erosion) + 2*padding(source)
    //                     = 8 + 2*2 + 2*0 = 12
    let expected_internal_tile_size = tile_size + 2 * 2;
    assert_eq!(outputs.len(), 1);
    assert_eq!(
        outputs[0].len(),
        expected_internal_tile_size * expected_internal_tile_size
    );
}

#[test]
fn node_mut_lets_callers_mutate_a_node_in_place() {
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 4, 1.0 / 4 as f32));
    let id = graph.add_node(Box::new(Erosion::default()));
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
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 4, 1.0 / 4 as f32));
    let id = graph.add_node(Box::new(Flat));
    graph.remove_node(id).unwrap();
    assert!(graph.node_mut(id).is_err());
}

#[test]
fn mutating_a_node_invalidates_its_own_and_downstream_cached_output() {
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 8, 1.0 / 8 as f32));
    let source = graph.add_node(Box::new(Perlin::default()));
    let sink = graph.add_node(Box::new(Erosion::default()));
    graph.connect(source, 0, sink, 0).unwrap();

    let first = match graph.process(sink).unwrap() {
        NodeGraphProcessResult::Processed(g, _) => g,
        _ => panic!("expected the graph to finish processing")
    };

    // Re-processing without any change should serve the cached result: the reported
    // generation must not have advanced.
    let cached = match graph.process(sink).unwrap() {
        NodeGraphProcessResult::Processed(g, _) => g,
        _ => panic!("expected the graph to finish processing")
    };
    assert_eq!(
        first, cached,
        "an unchanged graph should be served from cache"
    );

    // Changing the source's parameter should force both it and its downstream consumer to
    // recompute, advancing the generation counter. This also covers a source that has since
    // been evicted from cache (its output no longer needed once its consumer was itself
    // cached): eviction leaves the source `Dirty` too, so invalidation can't just skip a node
    // for already being `Dirty` - it must still propagate to that node's own downstream.
    graph
        .node_mut(source)
        .unwrap()
        .set_param("frequency", NParamValue::Float(5.0))
        .unwrap();
    let after_change = match graph.process(sink).unwrap() {
        NodeGraphProcessResult::Processed(g, _) => g,
        _ => panic!("expected the graph to finish processing")
    };
    assert!(
        after_change > cached,
        "changing an upstream parameter should force recomputation"
    );
}

#[test]
fn tile_size_reports_the_size_the_graph_was_created_with() {
    let graph = NodeGraph::new(ChunkGrid::new(1, 1, 16, 1.0 / 16 as f32));
    assert_eq!(graph.tile_size(), 16);
}

#[test]
fn is_processing_is_false_before_anything_has_been_computed() {
    let graph = NodeGraph::new(ChunkGrid::new(1, 1, 4, 1.0 / 4 as f32));
    assert!(!graph.is_processing());
}

#[test]
fn is_processing_is_true_immediately_after_a_node_is_computed() {
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 4, 1.0 / 4 as f32));
    let id = graph.add_node(Box::new(Flat));
    graph.process(id).expect("processing should succeed");
    assert!(graph.is_processing());
}

const TEST_ICON: NodeIcon = NodeIcon {
    id: "test-icon",
    png_bytes: &[]
};

/// A node whose `process` counts how many times it actually runs (as opposed to being served
/// from cache), so a test can prove a `Global` node isn't recomputed once per chunk. Its output is
/// a constant field, so a test can also check that cropping/resampling it into a chunk-sized tile
/// doesn't corrupt the data.
#[derive(Debug)]
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
            dtype: NodePortType::Height,
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
}

/// A node whose `process` counts how many times it actually runs, used to distinguish "served
/// from cache" from "recomputed" - the generation counter `NodeGraph::process*` reports is a
/// coarse graph-wide "something changed" signal, not a per-node/per-chunk recompute stamp, so it
/// can't be used to tell the two apart on its own.
#[derive(Debug)]
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
            dtype: NodePortType::Height,
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
}

#[test]
fn requesting_the_same_chunk_twice_hits_the_per_chunk_cache() {
    let mut graph = NodeGraph::new(ChunkGrid::new(2, 1, 4, 1.0));
    let calls = Arc::new(AtomicUsize::new(0));
    let id = graph.add_node(Box::new(FakeCountingSource {
        calls: calls.clone()
    }));

    graph.process_chunk(id, ChunkCoord(0, 0)).unwrap();
    graph.process_chunk(id, ChunkCoord(0, 0)).unwrap();

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

    graph.process_chunk(id, ChunkCoord(0, 0)).unwrap();
    graph.process_chunk(id, ChunkCoord(1, 0)).unwrap();
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "each distinct chunk should be computed once"
    );

    // Re-requesting chunk 0 should still be cached, independent of chunk 1 having since run.
    graph.process_chunk(id, ChunkCoord(0, 0)).unwrap();
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "chunk 0's own cache should be unaffected by chunk 1 being computed"
    );
}

#[test]
fn perlin_generator_samples_a_shared_world_coordinate_frame_across_chunks() {
    // world_scale = 1.0, so chunk 1 begins exactly where chunk 0's 4 world units end.
    let mut graph = NodeGraph::new(ChunkGrid::new(2, 1, 4, 1.0));
    let perlin = graph.add_node(Box::new(Perlin::default()));

    let chunk1 = match graph.process_chunk(perlin, ChunkCoord(1, 0)).unwrap() {
        NodeGraphProcessResult::Processed(_, tiles) => tiles[0].clone(),
        _ => panic!("expected the graph to finish processing")
    };

    // Default params: frequency = amplitude = 1.0, 4 octaves - reimplements the node's own
    // value-noise formula (see `node_generator_perlin.rs`) against an explicit world position,
    // rather than a tile-local `[0, 1]` one.
    const NOISE_PERIOD: i32 = 1024;
    fn hash(ix: i32, iy: i32) -> f32 {
        let px = ix.rem_euclid(NOISE_PERIOD) as u32;
        let py = iy.rem_euclid(NOISE_PERIOD) as u32;
        let mut h = px.wrapping_mul(374761393) ^ py.wrapping_mul(668265263);
        h = (h ^ (h >> 13)).wrapping_mul(1274126177);
        h ^= h >> 16;
        (h as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
    fn value_noise(x: f32, y: f32) -> f32 {
        let x0 = x.floor();
        let y0 = y.floor();
        let (ix0, iy0) = (x0 as i32, y0 as i32);
        let (fx, fy) = (x - x0, y - y0);
        let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
        let n00 = hash(ix0, iy0);
        let n10 = hash(ix0 + 1, iy0);
        let n01 = hash(ix0, iy0 + 1);
        let n11 = hash(ix0 + 1, iy0 + 1);
        let nx0 = n00 + sx * (n10 - n00);
        let nx1 = n01 + sx * (n11 - n01);
        nx0 + sy * (nx1 - nx0)
    }
    let expected_at = |wx: f32, wy: f32| -> f32 {
        let (mut frequency, mut amplitude, mut value) = (1.0f32, 1.0f32, 0.0f32);
        for _ in 0..4 {
            value += value_noise(wx * frequency, wy * frequency) * amplitude;
            frequency *= 2.0;
            amplitude *= 0.5;
        }
        value
    };

    // World space is centered on the whole grid: chunk 0 spans world x in [-4, 0) and chunk 1
    // spans [0, 4) at world y = -2, so chunk 1's texel (0, 0) sits at world position (0, -2) -
    // continuing chunk 0's span rather than restarting it.
    let s = chunk1.size();
    assert_eq!(s, 4, "no downstream kernel node means no margin padding");
    assert!(
        (chunk1[0] - expected_at(0.0, -2.0)).abs() < 1e-5,
        "chunk 1's first texel should sample world position (0, -2), continuing the same noise \
         field chunk 0 started, not restart at a tile-local (0, 0)"
    );
}

#[test]
fn a_global_node_is_evaluated_once_regardless_of_how_many_chunks_request_it() {
    // Reached through an explicit `NodeIntegrate` now, rather than wired straight into a `Local`
    // consumer - the engine no longer resamples a `Global` ancestor's output automatically.
    let mut graph = NodeGraph::new(ChunkGrid::new(2, 1, 4, 1.0));
    let calls = Arc::new(AtomicUsize::new(0));
    let source = graph.add_node(Box::new(FakeGlobalSource {
        calls: calls.clone(),
        native_resolution: 6
    }));
    let integrate = graph.add_node(Box::new(Integrate::default()));
    let erosion = graph.add_node(Box::new(Erosion::default()));
    graph.connect(source, 0, integrate, 0).unwrap();
    graph.connect(integrate, 0, erosion, 0).unwrap();

    graph.process_chunk(erosion, ChunkCoord(0, 0)).unwrap();
    graph.process_chunk(erosion, ChunkCoord(1, 0)).unwrap();
    // Re-requesting a chunk already computed should also not touch the global node again.
    graph.process_chunk(erosion, ChunkCoord(0, 0)).unwrap();

    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "the global node's whole-terrain pass should be memoized across every chunk"
    );
}

#[test]
fn integrate_maps_world_positions_into_its_globals_local_space_via_scale_and_position() {
    // World space is centered on the grid, so this single chunk's core spans world x in [-2, 2).
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 4, 1.0));
    let source = graph.add_node(Box::new(FakeGlobalWorldXMarker {
        native_resolution: 8
    }));
    // `scale = 4` gives the source a 4-world-unit physical footprint, `position = 0` centers that
    // footprint on the terrain's own origin.
    let integrate = graph.add_node(Box::new(Integrate {
        scale: 4.0,
        position: (0.0, 0.0)
    }));
    graph.connect(source, 0, integrate, 0).unwrap();

    let tiles = match graph.process_chunk(integrate, ChunkCoord(0, 0)).unwrap() {
        NodeGraphProcessResult::Processed(_, tiles) => tiles,
        _ => panic!("expected the graph to finish processing")
    };

    // `source` encodes each texel's own local x position as its value. With this scale/position,
    // world x in `[-2, 2)` maps to local x in `[-0.5, 0.25]` - landing exactly on 4 of `source`'s
    // own 8 texels, so the integrated output should read back precisely their local-space values,
    // not some other mapping.
    let s = tiles[0].size();
    assert_eq!(s, 4);
    let row: Vec<f32> = (0..4).map(|x| tiles[0][x]).collect();
    let expected = [-0.5, -0.25, 0.0, 0.25];
    for (got, want) in row.iter().zip(expected.iter()) {
        assert!(
            (got - want).abs() < 1e-4,
            "got {:?}, expected {:?}",
            row,
            expected
        );
    }
}

#[test]
fn integrates_physical_size_is_independent_of_the_globals_native_resolution() {
    // Regression test: `scale`/`position` alone should determine a global shape's world footprint
    // - `native_resolution` should only pick how finely that same footprint is sampled, never how
    // big it is. `FakeGlobalWorldXMarker`'s value is an exactly linear function of its own local
    // position, and bilinear interpolation reproduces an exactly linear function everywhere
    // between grid points regardless of how coarse or fine the grid is - so if two sources at
    // different resolutions (same scale/position) disagree anywhere, resolution must be leaking
    // into the physical footprint.
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 4, 1.0));
    let low_res = graph.add_node(Box::new(FakeGlobalWorldXMarker {
        native_resolution: 4
    }));
    let high_res = graph.add_node(Box::new(FakeGlobalWorldXMarker {
        native_resolution: 64
    }));
    let integrate_low = graph.add_node(Box::new(Integrate {
        scale: 5.0,
        position: (0.0, 0.0)
    }));
    let integrate_high = graph.add_node(Box::new(Integrate {
        scale: 5.0,
        position: (0.0, 0.0)
    }));
    graph.connect(low_res, 0, integrate_low, 0).unwrap();
    graph.connect(high_res, 0, integrate_high, 0).unwrap();

    let low = match graph
        .process_chunk(integrate_low, ChunkCoord(0, 0))
        .unwrap()
    {
        NodeGraphProcessResult::Processed(_, tiles) => tiles[0].clone(),
        _ => panic!("expected the graph to finish processing")
    };
    let high = match graph
        .process_chunk(integrate_high, ChunkCoord(0, 0))
        .unwrap()
    {
        NodeGraphProcessResult::Processed(_, tiles) => tiles[0].clone(),
        _ => panic!("expected the graph to finish processing")
    };

    for i in 0..low.size() * low.size() {
        assert!(
            (low[i] - high[i]).abs() < 1e-4,
            "texel {} differs between a 4-texel and a 64-texel global source at the same scale/position ({} vs {}) - \
             native_resolution must be leaking into the physical footprint",
            i,
            low[i],
            high[i]
        );
    }
}

/// A `Global` node whose output encodes each texel's own local-space x position, so a test can
/// check exactly how it gets sampled (directly, or through an integration node).
#[derive(Debug)]
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
            dtype: NodePortType::Height,
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
}

#[test]
fn a_directly_requested_global_node_always_returns_its_own_bare_result_centered_at_zero() {
    // A `Global` node never places itself on the terrain: previewing or exporting it directly
    // shows its own bare, self-centered shape - the same regardless of which chunk asked for it.
    // Mapping it onto the actual terrain is the explicit job of an integration node instead.
    let mut graph = NodeGraph::new(ChunkGrid::new(2, 1, 4, 1.0));
    let source = graph.add_node(Box::new(FakeGlobalWorldXMarker {
        native_resolution: 8
    }));

    let chunk0 = match graph.process_chunk(source, ChunkCoord(0, 0)).unwrap() {
        NodeGraphProcessResult::Processed(_, tiles) => tiles[0].clone(),
        _ => panic!("expected the graph to finish processing")
    };
    let chunk1 = match graph.process_chunk(source, ChunkCoord(1, 0)).unwrap() {
        NodeGraphProcessResult::Processed(_, tiles) => tiles[0].clone(),
        _ => panic!("expected the graph to finish processing")
    };

    assert_eq!(
        chunk0.to_vec(),
        chunk1.to_vec(),
        "which chunk asked for it shouldn't matter - a global node's bare result is the same either way"
    );

    // Its own local space is centered at 0: the middle texel of an 8-wide buffer (index 4) sits
    // right at local x = 0.
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
    let mut graph = NodeGraph::new(ChunkGrid::new(2, 1, 4, 1.0));
    let local = graph.add_node(Box::new(Flat));
    let global = graph.add_node(Box::new(FakeGlobalSource {
        calls: Arc::new(AtomicUsize::new(0)),
        native_resolution: 6
    }));

    graph.process_chunk(local, ChunkCoord(0, 0)).unwrap();
    let expected_local = 4 * 4 * std::mem::size_of::<f32>(); // tile_size = 4, no padding
    assert_eq!(graph.cached_bytes(), expected_local);

    graph.process_chunk(global, ChunkCoord(0, 0)).unwrap();
    let expected_global = 6 * 6 * std::mem::size_of::<f32>(); // native_resolution = 6
    assert_eq!(
        graph.cached_bytes(),
        expected_local + expected_global,
        "the global node's own whole-terrain buffer should add to the total alongside the local \
         chunk's, not replace it or be left out"
    );
}

#[test]
fn cached_bytes_reflects_current_cache_contents_after_a_pool_resizing_selection_change() {
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 4, 1.0));
    let flat = graph.add_node(Box::new(Flat)); // no kernel -> internal tile size 4
    let erosion = graph.add_node(Box::new(Erosion::default())); // padding 2 -> internal tile size 8
    graph.connect(flat, 0, erosion, 0).unwrap();

    graph.process_chunk(flat, ChunkCoord(0, 0)).unwrap();
    assert_eq!(graph.cached_bytes(), 4 * 4 * std::mem::size_of::<f32>());

    // Selecting `erosion` needs a bigger padded pool, which discards the previous chunk-scoped
    // cache (including flat's now-stale entry from the smaller pool) and recomputes both nodes at
    // the new size. `flat`'s own intermediate result is then evicted once erosion (its only
    // consumer) is itself fully cached, leaving just erosion's output live.
    graph.process_chunk(erosion, ChunkCoord(0, 0)).unwrap();
    assert_eq!(
        graph.cached_bytes(),
        8 * 8 * std::mem::size_of::<f32>(),
        "should reflect exactly what's cached now (erosion's output, at the new padded size) \
         rather than a stale or partial figure from before the pool was resized"
    );
}
