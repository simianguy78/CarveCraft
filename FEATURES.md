# CAM feature checklist: VCarve Pro & Aspire vs CarveCraft

The VCarve Pro / Aspire column is compiled from Vectric's public product information
(version 12 era); names can differ slightly between versions. CarveCraft status reflects
the code in this repository.

**Key:** ✅ supported · 🟡 partly · ❌ not yet · — not in that product

## 2D toolpaths

| Feature | VCarve Pro | Aspire | CarveCraft |
|---|:-:|:-:|:-:|
| Profile – outside / inside / on the line | ✅ | ✅ | ✅ |
| Profile – start depth, cut depth, pass depth | ✅ | ✅ | ✅ |
| Profile – allowance (leave material) | ✅ | ✅ | ✅ |
| Profile – climb / conventional direction | ✅ | ✅ | ✅ |
| Profile – holding tabs (count, length, thickness) | ✅ | ✅ | ✅ (evenly spaced; can't drag tabs to place them) |
| Profile – ramped plunges | ✅ | ✅ | ✅ (smooth ramp; no zig-zag/spiral choice) |
| Profile – lead in / lead out (arc or line) | ✅ | ✅ | ❌ |
| Profile – separate last (finishing) pass | ✅ | ✅ | ❌ (use a second profile with allowance) |
| Profile – corner sharpening (V-bit) | ✅ | ✅ | ❌ |
| Profile – edit passes individually | ✅ | ✅ | ❌ |
| Pocket – offset and raster clearing | ✅ | ✅ | ✅ |
| Pocket – raster angle | ✅ | ✅ | ❌ (raster runs along X) |
| Pocket – allowance, start depth, direction | ✅ | ✅ | ✅ |
| Pocket – finishing pass round the wall | ✅ | ✅ | ✅ |
| Pocket – ramp plunge moves | ✅ | ✅ | ❌ |
| Pocket – area clearance with a second (larger) tool | ✅ | ✅ | ❌ |
| Pocket – stay-down linking between passes | ✅ | ✅ | ✅ |
| Drilling – peck drilling | ✅ | ✅ | ✅ (retracts every pass depth) |
| Drilling – dwell, hole sizes from circles | ✅ | ✅ | 🟡 (drills the centre of each shape; no dwell) |
| V-Carving – sharp V-grooves | ✅ | ✅ | ✅ (grid-based, about 0.03 mm) |
| V-Carving – flat depth | ✅ | ✅ | ✅ |
| V-Carving – flat-area clearance tool | ✅ | ✅ | ❌ |
| V-Carving – corner sharpening, start depth | ✅ | ✅ | ❌ |
| V-Carve Inlay (male / female) | ✅ | ✅ | ❌ |
| Quick Engrave (V-bit, constant depth) | ✅ | ✅ | ✅ |
| Fluting (ramp in/out along a vector) | ✅ | ✅ | 🟡 (one smooth ramp shape; no ramp-type choices) |
| Prism carving toolpath | ✅ | ✅ | ❌ (prism exists as a 3D model shape only) |
| Chamfer toolpath | ✅ | ✅ | ❌ |
| Thread milling | ✅ | ✅ | ❌ |
| Moulding toolpath (profile swept along a rail) | ✅ | ✅ | ❌ |
| Texture toolpath (hand-tooled strokes) | ✅ | ✅ | ✅ |
| Laser toolpaths (with laser module) | ✅ | ✅ | ❌ |

## 3D toolpaths

| Feature | VCarve Pro | Aspire | CarveCraft |
|---|:-:|:-:|:-:|
| 3D roughing (Z-level / raster) | ✅ | ✅ | 🟡 (Z-level raster; model sampled on a grid) |
| 3D finishing – raster | ✅ | ✅ | 🟡 (along X only; no raster angle) |
| 3D finishing – offset / spiral strategies | ✅ | ✅ | ❌ |
| Machining boundary (selected vectors, model, job) | ✅ | ✅ | ❌ (whole job only) |
| Machine multiple 3D components | 🟡 (imported clipart) | ✅ | ✅ |
| Project 2D toolpaths onto a 3D surface | ✅ | ✅ | ❌ |
| Rotary / wrapped machining | ✅ | ✅ | ❌ |
| Double-sided machining | ✅ | ✅ | ❌ |

## Tools, preview and output

| Feature | VCarve Pro | Aspire | CarveCraft |
|---|:-:|:-:|:-:|
| Tool database (end mill, ball nose, V-bit) | ✅ | ✅ | ✅ |
| Tool types: flat-bottom V, radiused, form, drill, engraving tip | ✅ | ✅ | ❌ |
| Feeds, speeds, flutes, chip load | ✅ | ✅ | ✅ |
| Saved tool library with per-material settings | ✅ | ✅ | ✅ (groups, notes, materials, import/export) |
| Material setup: size, thickness, Z zero top / bed | ✅ | ✅ | ✅ |
| XY datum: corners, centre, offset | ✅ | ✅ | 🟡 (bottom-left or centre) |
| Safe Z and home position | ✅ | ✅ | 🟡 (safe Z; returns to the origin at the end) |
| Toolpath list: show/hide, reorder, recalculate | ✅ | ✅ | ✅ |
| Toolpath templates, copy, merge | ✅ | ✅ | ❌ |
| Toolpath summary / machining time estimate | ✅ | ✅ | 🟡 (time estimate per toolpath) |
| Preview: animated material removal | ✅ | ✅ | ✅ (speed and pause) |
| Preview: per-toolpath colours, wood textures | ✅ | ✅ | 🟡 (one wood colour) |
| Post-processors | ✅ (hundreds) | ✅ (hundreds) | 🟡 (GRBL, Mach3/UCCNC, LinuxCNC; mm or inch) |
| Arc output (G2/G3) | ✅ | ✅ | ✅ |
| Save toolpaths together or one file per tool | ✅ | ✅ | 🟡 (all ticked, or one toolpath at a time) |
| Toolpath tiling (jobs larger than the machine) | ✅ | ✅ | ❌ |
| Sheets and nesting | ✅ | ✅ | ❌ |
| Direct output to machine (send G-code) | ✅ | ✅ | ❌ |

## Accuracy notes

- Profile and pocket outlines use exact Clipper2 offsets (within 0.002 mm), with round
  outside corners and sharp inside corners.
- V-carving and 3D toolpaths still sample on a grid (about 0.03 mm for V-carve, finer for
  3D), so they are close to, but not yet as exact as, Vectric's.
- Test any new post-processor with a dry run (spindle off or cutting air) before cutting.
