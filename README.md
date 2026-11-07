# CarveCraft Studio

A modern desktop app for designing and cutting CNC router projects: draw 2D vectors and text,
build 3D relief models, create toolpaths and save G-code.

## Features

- **Start screen** – new job, template, open file, recent files, quick guide.
- **2D design** – rectangle, circle, ellipse, polygon, star, polyline, freehand and text;
  duplicate, delete, mirror, rotate, centre, offset, circular and grid arrays, layers, rulers and snapping.
- **3D modelling** – shape and prism from vectors, extrude along a curve, two-rail sweep,
  textures, a relief library and height-map import; combine modes (add, subtract, merge, low),
  height scale, tilt, fade, invert, bake, and sculpting (smooth, deposit, remove, erase, flatten).
  Export the model as STL.
- **Toolpaths** – profile (outside/inside/on), pocket (raster/offset), drilling, V-carve,
  quick engrave, fluting, 3D roughing, 3D finishing and texture strokes, with an editable tool list
  (diameter, angle, stepover, pass depth, feed, plunge, RPM, flutes and chip load).
- **Preview** – animated material-removal simulation in the 3D view, with speed and pause.
- **Output** – G-code for GRBL, Mach3/UCCNC or LinuxCNC (mm or inch) with G2/G3 arcs, Z zero on
  the material surface or the machine bed. Profile and pocket outlines use exact Clipper2 offsets.
- Dark and light themes. Jobs are saved as `.ccj` files.

See [FEATURES.md](FEATURES.md) for a CAM feature checklist against VCarve Pro and Aspire.

## Build and run

Needs [Rust](https://rustup.rs) (stable).

```
cargo build --release
cargo run --release
```

Options:

- `carvecraft <file.ccj>` opens a saved job.
- `carvecraft --screenshot out.png` starts, draws about 120 frames, saves a picture of the window and exits.

## Mouse

Wheel zooms, right-drag pans, and left-drag orbits the 3D view.
Keys: Ctrl+Z / Ctrl+Y undo and redo, Ctrl+D duplicate, Del delete, Esc back to Select, F2 / F3 switch 2D / 3D.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option. Unless you say otherwise, any contribution you submit is offered under the
same two licences.

CarveCraft is an independent project, not affiliated with Vectric; VCarve and Aspire are
their trademarks.
