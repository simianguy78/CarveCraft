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
- **Output** – generic G-code (mm, absolute) with Z zero on the material surface or the machine bed.
- Dark and light themes. Jobs are saved as `.ccj` files.

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
