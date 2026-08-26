<p align="center">
    <img src="assets/icon.png" height="128" alt="icon"/>
</p>

# WaterDrop Terrain Generator

![name](https://img.shields.io/badge/Made_by-ExplorableScience-9cf)
![language](https://img.shields.io/badge/Language-Rust-red)
![license](https://img.shields.io/badge/License-GPLv3-blue)
![status](https://img.shields.io/badge/Status-Work_in_Progress-orange)

## Presentation
**WaterDrop Terrain Generator** is a node-based terrain generation and erosion tool, built on top of **[WaterDropEngine](https://github.com/explorablescience/WaterDropEngine)** (*wde*).

Terrains are authored as a graph of nodes — generators, erosion, I/O — wired together and previewed live in 3D as the graph is edited.

*Like WaterDropEngine, this is a personal project: a playground to experiment with terrain generation and editor tooling for my own use, not a general-purpose product. It is still under active development, so expect things to move around.*

<p align="center">
    <img src="assets/screenshot.png" alt="screenshot" width="90%"/>
</p>

## Dependencies
The generator is built on:
- **[WaterDropEngine](https://github.com/explorablescience/WaterDropEngine)** for the ECS, renderer and editor scaffolding.
- **[Bevy](https://bevyengine.org/)**, which WaterDropEngine itself is built on.
- **[egui-snarl](https://github.com/rerun-io/egui-snarl)** for the node graph editor.

## Running the generator
You'll need Rust installed — see the [official instructions](https://www.rust-lang.org/tools/install) if you don't have it yet.

This project depends on **WaterDropEngine** as a sibling repository (see `Cargo.toml`), so clone both side by side:

```sh
git clone https://github.com/explorablescience/WaterDropEngine.git
git clone https://github.com/explorablescience/WaterDropTerrainGenerator.git
cd WaterDropTerrainGenerator
cargo run
```

## License
Distributed under the **GPLv3** license. See [LICENSE](LICENSE) for details.
