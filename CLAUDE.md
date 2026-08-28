# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

WaterDrop Terrain Generator is a node-graph terrain editor: an egui-snarl canvas where nodes (noise, primitives, erosion, ...) connect into a graph, evaluated per-chunk to produce heightmaps, previewed live in an embedded 3D viewport. It's a standalone Bevy ECS application built on **WaterDropEngine (wde)**, a sibling personal game engine consumed via a Cargo path dependency at `../WaterDropEngine/crates/core/wde` — see [../WaterDropEngine/CLAUDE.md](../WaterDropEngine/CLAUDE.md) for that engine's own architecture (render graph, RenderAsset/RenderBinding system, dual-world renderer, etc.), which this project builds directly on top of.

## Commands

```bash
# Run (debug)
cargo run --bin waterdrop-terrain-generator

# Run with engine debug features (see Cargo features below)
cargo run --features debug,log-debug

# Build (release: LTO, opt-level 3, codegen-units 1)
cargo build --release --bin waterdrop-terrain-generator

# Integration tests (see Tests below — there are no unit tests in src/)
cargo test

# Lint
cargo clippy --workspace --all-targets -- -D warnings

# Format (requires nightly — rustfmt.toml sets unstable_features = true)
cargo +nightly fmt --all
cargo +nightly fmt --all --check
```

Not a Cargo workspace — single binary crate, no parent `Cargo.toml`. CI (`.github/workflows/ci.yml`) runs `--workspace` flags anyway; they just resolve to this one crate.

### Cargo features

All pass through to the `wde` dependency (no locally feature-gated code): `log-debug`, `debug`, `watch`, `tracing`, `gpu-debug` — same meanings as in WaterDropEngine.

### `build.rs`

Copies `res/` and `assets/` into `target/{debug,release}/` on every build (`cargo:rerun-if-changed` on both).

## Workspace structure

```
src/
  main.rs               # App entry point: wires wde plugins (logger, renderer, pbr, camera, editor) + TerrainSessionHolder + RenderPlugin + UIPlugin
  lib.rs                # Re-exports core, nodes, render, ui
  core/
    graph/               # NodeGraph: topology, evaluation engine, per-chunk/global cache
    node/                 # The `Node` trait, param/socket/error types, inventory-based node registry
    tiling/               # ChunkGrid/ChunkCoord, TileContext, TilePool/TileBuffer, sampling helpers
    parallelism/           # TerrainSession/TerrainSessionHolder, async per-chunk job scheduling (ChunkJobs)
  nodes/                 # Concrete Node implementations (generation, modification, utility, export)
  render/                 # Live 3D terrain preview of the selected node (see Architecture below)
  ui/                     # egui-snarl graph canvas, properties panel, terrain settings, theme
tests/core/             # Integration tests mirroring core/'s layout (no unit tests live in src/)
vendor/egui-snarl-0.9.0/ # Vendored patch, see vendor/README.md and Cargo.toml's [patch.crates-io]
res/, assets/            # Runtime assets (copied to target/ by build.rs)
```

## Architecture

### Node graph (`core/graph`, `core/node`)

`NodeGraph` owns a `Topology` (pure structure: nodes/edges, stable `GraphNodeId`s via tombstoning), an `EvalCache` (`Mutex<HashMap<(GraphNodeId, EvalScope), NodeState>>` — `EvalScope` is `Chunk(ChunkCoord)` or `Global`, so the same node can cache a different tile per chunk plus one global entry), a shared `TilePool`, and the `ChunkGrid`. `mark_dirty` propagates downstream through the topology and stops at `Baked` nodes.

Node types implement `trait Node` (`core/node/mod.rs`): `label`/`category`/`icon`, `size()` (kernel radius, default 0 — e.g. `Erosion` declares 3 for its 3×3 neighbor average), `locality()` (`NodeLocality::Local` or `Global { native_resolution }`, default `Local`), `inputs`/`outputs` sockets, param get/set, and `process(pool, inputs, ctx) -> Result<Vec<TileHandle>, NodeError>`. Registration is `inventory`-based (`core/node/registry.rs`) — every node file ends with `inventory::submit! { NodeDescriptor { ... } }`; no derive macro, no central list to update.

`core/graph/eval.rs` is the evaluation engine: topological recursion with cycle detection, `required_internal_tile_size` sums `Local` ancestors' kernel padding to size tile margins, `Global` ancestors stop fan-out (their kernel padding is self-contained). `process_chunk_shared` is what background chunk jobs call; `process_sync` drives the (non-parallelizable) `Global`-locality preview path directly.

### Chunking (`core/tiling`)

`ChunkGrid { chunks_x, chunks_y, tile_size, world_scale }` + `ChunkCoord(i32, i32)` is the terrain's chunk partitioning, configured from the UI's Terrain Settings panel. `TileContext` is the position-aware frame passed to `Node::process` (world origin/step/extent for a given chunk, or the bare `[-0.5,0.5)` frame for `Global` nodes). `TilePool`/`TileHandle` is a free-list allocator for the `Vec<f32>` tile buffers, returned to the pool on `Drop`.

### Async evaluation (`core/parallelism`)

`TerrainSessionHolder` (`Arc<RwLock<TerrainSession>>`, a Bevy `Resource`) owns the `NodeGraph`, the currently-selected/displayed node, and per-`(node, chunk)` generation bookkeeping. `ChunkJobs` spawns one `AsyncComputeTaskPool` task per chunk (`process_chunk_shared`) and polls them across frames — a frame only picks up whichever chunks finished since the last one, so local-locality nodes fill in gradually rather than blocking.

### Terrain preview rendering (`render/`)

Renders the currently selected node's output as a live 3D mesh. **Not** a naive per-chunk-mesh renderer: heightmap data lives in a GPU texture array (`R32Float`, one layer per chunk, format chosen over `wde-terrain`'s `R8Unorm` for full-range/precision heights), and every chunk is drawn as one instanced GPU draw call of a single shared flat grid mesh (`utils::build_shared_chunk_mesh`) — the vertex shader (`res/terrain_preview/terrain_preview.vert.wgsl`) displaces height and computes normals per-instance via `textureLoad` against that chunk's layer (no sampler — mesh vertices land exactly on texel centers, matching the old CPU-baked math exactly), reading a per-chunk `world_offset`/`cell_size`/`layer` descriptor from a storage buffer indexed by `@builtin(instance_index)` (`chunk_array.rs`). This exists specifically to sidestep the engine's shared bindless SSBO mesh arena's fixed vertex/index capacity (see WaterDropEngine's CLAUDE.md and `ssbo_mesh.rs`), which a large dynamically-regenerated multi-chunk terrain would overflow with one mesh per chunk.

`padded_heightmap` (`utils.rs`) still does CPU-side border stitching from neighbor chunks (1 texel low / 2 high padding) before a chunk's data is uploaded as a texture layer — this is unchanged from before the texture-array rewrite, just fed into a `copy_from_buffer_layered` GPU upload instead of baked into vertex positions. It defensively checks `core_data.len() == tile_size²` before indexing: a chunk whose background job hasn't caught up with a just-changed tile size still holds data sized for the *old* tile size, and this used to panic (`generate_chunks.rs`'s array-recreation path re-uploads *every* known chunk, not just freshly-changed ones, so it can race a stale chunk).

Two non-obvious gotchas from getting this working, both fixed at the point they'd bite anyone extending this again:
- A `layer_count == 1` texture array gets a plain `D2` view from the engine's `Texture::new` (its convention for "non-array"), which won't bind against this pipeline's `D2Array`-typed layout — `sync_preview_state` clamps `layer_count` to at least 2 to avoid it (the `Global`-locality preview always has exactly one chunk).
- The engine's `add_texture_array_view` binding builder used to hardcode `filterable: true` in its bind-group layout; fixed upstream in WaterDropEngine (`wde-wgpu`'s `BindGroupLayoutBuilder::add_texture_array_view` and the one caller in `wde-renderer`'s `render_binding.rs`) to thread the texture's actual `filterable` flag through, mirroring the existing `TextureView` (non-array) code path — needed since `R32Float` isn't filterable without a device feature we don't request.
- The initial heightmap upload for a freshly-created texture array can race the GPU asset prep (a `Texture` asset added this frame isn't necessarily `GpuTexture`-ready this frame) — `TerrainPreviewGpu::pending_writes` is a persistent retry queue, not a one-shot upload, so a write that arrives before the texture is ready lands as soon as it is instead of being silently dropped.

### UI (`ui/`)

`panel_graph.rs` is the `egui-snarl` node canvas (`GraphViewer: SnarlViewer<GraphNode>`, bridging snarl events to the real `NodeGraph`); `panel_properties.rs` renders the selected node's params/messages; `panel_terrain_settings.rs` is the Chunk Grid config window (Chunks X/Y, Tile Size, World Scale, Apply). `theme.rs` centralizes the dark egui palette (including per-`NodeCategory` colors) and embedded fonts. `editor.rs`/`editor_behavior.rs` build the `egui_tiles` 3-pane dock layout (viewport / graph / properties).

## Code style

- Edition 2024, `cargo +nightly fmt` (own `rustfmt.toml`: `unstable_features = true`, no trailing commas, Unix newlines, field-init shorthand). Not inherited from WaterDropEngine — this crate has its own copy.
- Clippy run with `-D warnings`.
- No unit tests in `src/`; integration tests live in `tests/core/`, mirroring `core/`'s module layout 1:1. Coverage is scoped to `core/` only — `nodes/`, `render/`, `ui/` have no automated tests.
