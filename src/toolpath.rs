//! Tools, toolpath generation, cutting simulation and G-code output.

use crate::cam::{self, Post};
use crate::geom::{self, Field, Poly};
use crate::model::{self, Model};
use eframe::egui::{pos2, Pos2, Rect};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Origin {
    BottomLeft,
    Center,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Material {
    pub w: f32,
    pub h: f32,
    pub thickness: f32,
    pub origin: Origin,
    pub safe_z: f32,
    /// Z zero on the machine bed instead of the material surface.
    #[serde(default)]
    pub z_bed: bool,
    /// Which machine controller the G-code is written for.
    #[serde(default)]
    pub post: Post,
}

impl Default for Material {
    fn default() -> Self {
        Self { w: 300.0, h: 200.0, thickness: 20.0, origin: Origin::BottomLeft, safe_z: 5.0, z_bed: false, post: Post::default() }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolKind {
    EndMill,
    BallNose,
    VBit,
}

impl ToolKind {
    pub fn label(self) -> &'static str {
        match self {
            ToolKind::EndMill => "End mill",
            ToolKind::BallNose => "Ball nose",
            ToolKind::VBit => "V-bit",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub kind: ToolKind,
    pub diameter: f32,
    pub angle: f32,
    pub stepover: f32,
    pub pass_depth: f32,
    pub feed: f32,
    pub plunge: f32,
    pub rpm: f32,
    pub number: u32,
    #[serde(default = "two_flutes")]
    pub flutes: u32,
}

fn two_flutes() -> u32 {
    2
}

impl Tool {
    fn new(n: u32, name: &str, kind: ToolKind, dia: f32, angle: f32, stepover: f32, pass: f32, feed: f32) -> Tool {
        Tool { name: name.into(), kind, diameter: dia, angle, stepover, pass_depth: pass, feed, plunge: feed * 0.4, rpm: 18000.0, number: n, flutes: 2 }
    }

    pub fn defaults() -> Vec<Tool> {
        vec![
            Tool::new(1, "End Mill 6 mm", ToolKind::EndMill, 6.0, 0.0, 2.4, 3.0, 1500.0),
            Tool::new(2, "End Mill 3 mm", ToolKind::EndMill, 3.0, 0.0, 1.2, 1.5, 1000.0),
            Tool::new(3, "Ball Nose 3 mm", ToolKind::BallNose, 3.0, 0.0, 0.3, 1.5, 2000.0),
            Tool::new(4, "Ball Nose 6 mm", ToolKind::BallNose, 6.0, 0.0, 0.6, 3.0, 2500.0),
            Tool::new(5, "V-Bit 60°", ToolKind::VBit, 12.0, 60.0, 0.4, 2.0, 1200.0),
            Tool::new(6, "V-Bit 90°", ToolKind::VBit, 12.0, 90.0, 0.5, 2.0, 1200.0),
            Tool::new(7, "Engraving 30°", ToolKind::VBit, 3.0, 30.0, 0.2, 0.5, 800.0),
        ]
    }

    pub fn radius(&self) -> f32 {
        self.diameter * 0.5
    }

    /// Height of the tool surface above its tip at radial distance `d`.
    pub fn surface(&self, d: f32) -> f32 {
        let r = self.radius();
        match self.kind {
            ToolKind::EndMill => 0.0,
            ToolKind::BallNose => r - (r * r - d.min(r).powi(2)).max(0.0).sqrt(),
            ToolKind::VBit => d / (self.angle.max(1.0) * 0.5).to_radians().tan(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    Outside,
    Inside,
    On,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Op {
    Profile {
        depth: f32,
        side: Side,
        /// Depth the first pass starts from (0 = the material top).
        #[serde(default)]
        start: f32,
        /// Extra material left on the wall (negative cuts further).
        #[serde(default)]
        allowance: f32,
        /// Climb milling (else conventional).
        #[serde(default = "yes")]
        climb: bool,
        #[serde(default)]
        tabs: cam::Tabs,
        /// Ramp each pass in over this distance (0 = plunge straight down).
        #[serde(default)]
        ramp: f32,
    },
    Pocket {
        depth: f32,
        raster: bool,
        #[serde(default)]
        start: f32,
        #[serde(default)]
        allowance: f32,
        #[serde(default = "yes")]
        climb: bool,
    },
    Drill { depth: f32 },
    VCarve { flat_depth: f32 },
    Engrave { depth: f32 },
    Fluting { depth: f32 },
    Rough3D { allowance: f32 },
    Finish3D,
    Texture { depth: f32, length: f32 },
}

fn yes() -> bool {
    true
}

impl Op {
    pub fn profile(depth: f32, side: Side) -> Op {
        Op::Profile { depth, side, start: 0.0, allowance: 0.0, climb: true, tabs: cam::Tabs::default(), ramp: 0.0 }
    }
    pub fn pocket(depth: f32, raster: bool) -> Op {
        Op::Pocket { depth, raster, start: 0.0, allowance: 0.0, climb: true }
    }
    pub fn label(&self) -> &'static str {
        match self {
            Op::Profile { .. } => "Profile",
            Op::Pocket { .. } => "Pocket",
            Op::Drill { .. } => "Drilling",
            Op::VCarve { .. } => "V-Carve",
            Op::Engrave { .. } => "Quick Engrave",
            Op::Fluting { .. } => "Fluting",
            Op::Rough3D { .. } => "3D Roughing",
            Op::Finish3D => "3D Finishing",
            Op::Texture { .. } => "Texture",
        }
    }
    pub fn needs_vectors(&self) -> bool {
        !matches!(self, Op::Rough3D { .. } | Op::Finish3D | Op::Texture { .. })
    }
}

pub type Chains = Vec<Vec<[f32; 3]>>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Toolpath {
    pub id: u64,
    pub name: String,
    pub op: Op,
    pub tool: Tool,
    pub polys: Vec<Poly>,
    pub visible: bool,
    #[serde(skip)]
    pub chains: Arc<Chains>,
    #[serde(skip)]
    pub length: f32,
    #[serde(skip)]
    pub minutes: f32,
}

impl Toolpath {
    pub fn calculate(&mut self, model: &Model, mat: &Material) {
        let chains = generate(&self.op, &self.tool, &self.polys, model, mat);
        let mut len = 0.0;
        let mut plunges = 0.0;
        for c in &chains {
            for w in c.windows(2) {
                let d = [w[1][0] - w[0][0], w[1][1] - w[0][1], w[1][2] - w[0][2]];
                len += (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            }
            if let Some(f) = c.first() {
                plunges += mat.safe_z - f[2];
            }
        }
        self.length = len;
        self.minutes = len / self.tool.feed.max(1.0) + plunges / self.tool.plunge.max(1.0) + chains.len() as f32 * 0.02;
        self.chains = Arc::new(chains);
    }
}

/// Pass depths from `start` down to `depth`, each at most `pass` deep.
fn pass_levels_from(start: f32, depth: f32, pass: f32) -> Vec<f32> {
    let span = (depth - start).max(0.01);
    let n = (span / pass.max(0.05)).ceil().max(1.0) as usize;
    (1..=n).map(|k| -(start + span * k as f32 / n as f32)).collect()
}

fn pass_levels(depth: f32, pass: f32) -> Vec<f32> {
    let depth = depth.max(0.01);
    let n = (depth / pass.max(0.05)).ceil().max(1.0) as usize;
    (1..=n).map(|k| -(depth * k as f32 / n as f32)).collect()
}

fn close(pts: &[Pos2]) -> Vec<Pos2> {
    let mut v = pts.to_vec();
    if let Some(f) = pts.first() {
        v.push(*f);
    }
    v
}

fn chain_at(pts: &[Pos2], z: f32) -> Vec<[f32; 3]> {
    pts.iter().map(|p| [p.x, p.y, z]).collect()
}

fn field_cell(polys: &[Poly], div: f32, lo: f32, hi: f32) -> f32 {
    let b = geom::bounds_of(polys);
    (b.width().max(b.height()) / div).clamp(lo, hi)
}

pub fn generate(op: &Op, tool: &Tool, polys: &[Poly], model: &Model, mat: &Material) -> Chains {
    let r = tool.radius();
    let mut out: Chains = Vec::new();
    match op {
        Op::Profile { depth, side, start, allowance, climb, tabs, ramp } => {
            // Tool-centre loops from exact offsets (Clipper2), allowance leaving extra material.
            let off = r + allowance;
            let (mut closed_loops, open): (Vec<Poly>, Vec<Poly>) = match side {
                Side::On => polys.iter().cloned().partition(|p| p.closed),
                Side::Outside => (geom::offset_polys(&closed_only(polys), off), Vec::new()),
                Side::Inside => (geom::offset_polys(&closed_only(polys), -off), Vec::new()),
            };
            // Outside (and on-line) cuts have the part on the loop's inside; inside cuts on its outside.
            cam::orient_loops(&mut closed_loops, !matches!(side, Side::Inside), *climb);
            let tab_top = -depth + tabs.thickness;
            let mut z_prev = -start;
            for z in pass_levels_from(*start, *depth, tool.pass_depth) {
                for l in &closed_loops {
                    let perimeter = geom::poly_length(&close(&l.pts));
                    let ch = cam::ramped_loop(&l.pts, z_prev, z, *ramp);
                    out.push(if tabs.on && z < tab_top { cam::apply_tabs(&ch, tabs, tab_top, perimeter) } else { ch });
                }
                for l in &open {
                    out.push(chain_at(&l.pts, z));
                }
                z_prev = z;
            }
        }
        Op::Pocket { depth, raster, start, allowance, climb } => {
            let closed = closed_only(polys);
            // Where the tool's centre may go: the shape shrunk by the tool radius (and allowance).
            let region = geom::offset_polys(&closed, -(r + allowance));
            if region.is_empty() {
                return out;
            }
            let so = tool.stepover.max(0.05);
            for z in pass_levels_from(*start, *depth, tool.pass_depth) {
                let mut level: Vec<cam::Chain> = Vec::new();
                let mut boundary = region.clone();
                cam::orient_loops(&mut boundary, false, *climb);
                if *raster {
                    // Zig-zag lines clipped exactly to the region, then one pass round its edge.
                    let b = geom::bounds_of(&region);
                    let rows = ((b.height() / so).ceil() as usize).max(1);
                    let step = b.height() / rows as f32;
                    for row in 0..=rows {
                        let y = (b.min.y + row as f32 * step).clamp(b.min.y + 1e-3, b.max.y - 1e-3);
                        let mut segs: Vec<cam::Chain> = cam::scanline(&region, y).into_iter().map(|(x0, x1)| vec![[x0, y, z], [x1, y, z]]).collect();
                        if row % 2 == 1 {
                            segs.reverse();
                            for s in &mut segs {
                                s.reverse();
                            }
                        }
                        level.extend(segs);
                    }
                } else {
                    // Offset rings from the centre outwards; the last one is the finishing edge.
                    let mut rings: Vec<Vec<Poly>> = Vec::new();
                    let mut cur = geom::offset_polys(&region, -so);
                    while !cur.is_empty() && rings.len() < 2000 {
                        let next = geom::offset_polys(&cur, -so);
                        rings.push(cur);
                        cur = next;
                    }
                    for mut ring in rings.into_iter().rev() {
                        cam::orient_loops(&mut ring, false, *climb);
                        for l in ring {
                            let from = level.last().and_then(|c| c.last()).map(|p| pos2(p[0], p[1])).unwrap_or(l.pts[0]);
                            level.push(chain_at(&close(&cam::start_nearest(&l.pts, from)), z));
                        }
                    }
                }
                for l in &boundary {
                    let from = level.last().and_then(|c| c.last()).map(|p| pos2(p[0], p[1])).unwrap_or(l.pts[0]);
                    level.push(chain_at(&close(&cam::start_nearest(&l.pts, from)), z));
                }
                // Stay down between passes wherever the link is inside the pocket.
                out.extend(cam::link_chains(level, &region));
            }
        }
        Op::Drill { depth } => {
            for p in polys {
                let c = geom::bounds_of(std::slice::from_ref(p)).center();
                let mut ch = vec![[c.x, c.y, 0.5]];
                for z in pass_levels(*depth, tool.pass_depth) {
                    ch.push([c.x, c.y, z]);
                    ch.push([c.x, c.y, 0.5]);
                }
                out.push(ch);
            }
        }
        Op::VCarve { flat_depth } => {
            let closed = closed_only(polys);
            if closed.is_empty() {
                return out;
            }
            let tan_h = (tool.angle.max(1.0) * 0.5).to_radians().tan();
            let cell = field_cell(&closed, 700.0, 0.03, 0.3);
            let f = Field::build(&closed, cell, cell * 3.0);
            let dmax = f.max();
            let max_tool_depth = r / tan_h;
            let mut step = (tool.stepover).clamp(0.05, 2.0);
            if dmax / step > 250.0 {
                step = dmax / 250.0;
            }
            let mut d = step * 0.5;
            while d < dmax {
                let mut z = d / tan_h;
                if *flat_depth > 0.0 {
                    z = z.min(*flat_depth);
                }
                z = z.min(max_tool_depth);
                for l in f.contours(d) {
                    out.push(chain_at(&close(&l), -z));
                }
                d += step;
            }
            out.reverse(); // deepest (centre) first reads nicer in the list but cuts identically
        }
        Op::Engrave { depth } => {
            for p in polys {
                let pts = if p.closed { close(&p.pts) } else { p.pts.clone() };
                out.push(chain_at(&pts, -depth));
            }
        }
        Op::Fluting { depth } => {
            for p in polys {
                let pts = if p.closed { close(&p.pts) } else { p.pts.clone() };
                let total = geom::poly_length(&pts).max(0.01);
                let ramp = total * 0.3;
                let mut ch = Vec::new();
                let mut s = 0.0;
                for (i, q) in pts.iter().enumerate() {
                    if i > 0 {
                        // resample long segments so the ramp is smooth
                        let a = pts[i - 1];
                        let seg = (*q - a).length();
                        let n = (seg / 1.0).ceil().max(1.0) as usize;
                        for k in 1..=n {
                            let t = k as f32 / n as f32;
                            let pp = a + (*q - a) * t;
                            let ss = s + seg * t;
                            let f = (ss / ramp).min((total - ss) / ramp).clamp(0.0, 1.0);
                            let f = f * f * (3.0 - 2.0 * f);
                            ch.push([pp.x, pp.y, -depth * f]);
                        }
                        s += seg;
                    } else {
                        ch.push([q.x, q.y, 0.0]);
                    }
                }
                out.push(ch);
            }
        }
        Op::Rough3D { allowance } => {
            let req = required_grid(model, mat, tool, 1.0, tool.stepover.max(0.2));
            let lowest = req.vals.iter().copied().fold(0.0, f32::min);
            let levels = pass_levels(-lowest, tool.pass_depth);
            let mut prev = 0.0;
            for lv in levels {
                for (row, y) in req.ys.iter().enumerate() {
                    let mut segs: Vec<Vec<[f32; 3]>> = Vec::new();
                    let mut cur = Vec::new();
                    for (col, x) in req.xs.iter().enumerate() {
                        let need = req.vals[row * req.xs.len() + col] + allowance;
                        if need < prev - 1e-3 {
                            cur.push([*x, *y, need.max(lv)]);
                        } else if !cur.is_empty() {
                            segs.push(std::mem::take(&mut cur));
                        }
                    }
                    if !cur.is_empty() {
                        segs.push(cur);
                    }
                    if row % 2 == 1 {
                        segs.reverse();
                        for s in &mut segs {
                            s.reverse();
                        }
                    }
                    out.extend(segs.into_iter().filter(|s| s.len() >= 2));
                }
                prev = lv;
            }
        }
        Op::Finish3D => {
            let req = required_grid(model, mat, tool, 0.5, tool.stepover.max(0.1));
            for (row, y) in req.ys.iter().enumerate() {
                let n = req.xs.len();
                let mut ch: Vec<[f32; 3]> = Vec::with_capacity(n);
                for col in 0..n {
                    let z = req.vals[row * n + col];
                    // drop points that lie on a straight run
                    if ch.len() >= 2 {
                        let a = ch[ch.len() - 2];
                        let b = ch[ch.len() - 1];
                        if (a[2] - b[2]).abs() < 0.002 && (b[2] - z).abs() < 0.002 {
                            ch.pop();
                        }
                    }
                    ch.push([req.xs[col], *y, z]);
                }
                if row % 2 == 1 {
                    ch.reverse();
                }
                out.push(ch);
            }
        }
        Op::Texture { depth, length } => {
            let region = closed_only(polys);
            let spacing = (length * 0.55).max(0.5);
            let (w, h) = (mat.w, mat.h);
            let mut j = 0;
            let mut y = spacing * 0.5;
            while y < h {
                let mut i = 0;
                let mut x = spacing * 0.5;
                while x < w {
                    let rnd = |s: i32| {
                        let mut v = (i * 7919 + j * 104729 + s * 31) as u32;
                        v ^= v >> 13;
                        v = v.wrapping_mul(0x5bd1e995);
                        v ^= v >> 15;
                        (v & 0xffff) as f32 / 65535.0
                    };
                    let c = pos2(x + (rnd(1) - 0.5) * spacing * 0.6, y + (rnd(2) - 0.5) * spacing * 0.6);
                    if region.is_empty() || geom::point_inside(c, &region) {
                        let a = (rnd(3) - 0.5) * 0.9;
                        let len = length * (0.7 + 0.3 * rnd(4));
                        let d = depth * (0.6 + 0.4 * rnd(5));
                        let dir = eframe::egui::vec2(a.cos(), a.sin());
                        let ch: Vec<[f32; 3]> = (0..=8)
                            .map(|k| {
                                let t = k as f32 / 8.0;
                                let p = c + dir * (t - 0.5) * len;
                                [p.x, p.y, -d * (std::f32::consts::PI * t).sin()]
                            })
                            .collect();
                        out.push(ch);
                    }
                    x += spacing;
                    i += 1;
                }
                y += spacing * 0.8;
                j += 1;
            }
        }
    }
    out
}

fn closed_only(polys: &[Poly]) -> Vec<Poly> {
    polys.iter().filter(|p| p.closed && p.pts.len() >= 3).cloned().collect()
}

/// Lowest tip height that does not gouge the model, sampled on a raster.
struct ReqGrid {
    xs: Vec<f32>,
    ys: Vec<f32>,
    vals: Vec<f32>,
}

pub fn model_z<'a>(model: &'a Model, mat: &'a Material) -> impl Fn(Pos2) -> f32 + 'a {
    let top = model.max_height();
    let thick = mat.thickness;
    move |p: Pos2| (model.height_at(p) - top).max(-thick)
}

fn required_grid(model: &Model, mat: &Material, tool: &Tool, dx: f32, dy: f32) -> ReqGrid {
    let zf = model_z(model, mat);
    let r = tool.radius();
    let mut offsets = vec![(0.0f32, 0.0f32, 0.0f32)];
    for (ring, n) in [(0.35f32, 6), (0.7, 10), (1.0, 14)] {
        for k in 0..n {
            let a = k as f32 / n as f32 * std::f32::consts::TAU;
            let d = r * ring;
            offsets.push((d * a.cos(), d * a.sin(), tool.surface(d)));
        }
    }
    let xs: Vec<f32> = {
        let n = (mat.w / dx).ceil() as usize;
        (0..=n).map(|i| (i as f32 * dx).min(mat.w)).collect()
    };
    let ys: Vec<f32> = {
        let n = ((mat.h) / dy).ceil() as usize;
        (0..=n).map(|j| (j as f32 * dy).min(mat.h)).collect()
    };
    let mut vals = Vec::with_capacity(xs.len() * ys.len());
    for y in &ys {
        for x in &xs {
            let mut best = f32::MIN;
            for (ox, oy, s) in &offsets {
                let q = pos2(x + ox, y + oy);
                if q.x < 0.0 || q.y < 0.0 || q.x > mat.w || q.y > mat.h {
                    continue;
                }
                best = best.max(zf(q) - s);
            }
            vals.push(best.min(0.0));
        }
    }
    ReqGrid { xs, ys, vals }
}

// ------------------------------------------------------------ simulation

#[derive(Clone)]
pub struct Stock {
    pub nx: usize,
    pub ny: usize,
    pub cell: f32,
    pub z: Vec<f32>,
    pub thickness: f32,
}

impl Stock {
    pub fn new(mat: &Material) -> Stock {
        let cell = (mat.w.max(mat.h) / 640.0).max(0.05);
        let nx = (mat.w / cell).round().max(2.0) as usize;
        let ny = (mat.h / cell).round().max(2.0) as usize;
        Stock { nx, ny, cell, z: vec![0.0; nx * ny], thickness: mat.thickness }
    }
    pub fn grid(&self) -> model::Grid {
        model::Grid { nx: self.nx, ny: self.ny, cell: self.cell }
    }
}

struct Kernel {
    offs: Vec<(i32, i32, f32)>,
}

impl Kernel {
    fn new(tool: &Tool, cell: f32) -> Kernel {
        let r = tool.radius().max(cell * 0.5);
        let n = (r / cell).ceil() as i32;
        let mut offs = Vec::new();
        for dj in -n..=n {
            for di in -n..=n {
                let d = ((di * di + dj * dj) as f32).sqrt() * cell;
                if d <= r + cell * 0.3 {
                    offs.push((di, dj, tool.surface(d.min(r))));
                }
            }
        }
        Kernel { offs }
    }
}

pub struct Sim {
    pub stock: Stock,
    items: Vec<(Kernel, Arc<Chains>)>,
    ti: usize,
    ci: usize,
    pi: usize,
    pub done: bool,
    pub revision: u64,
    pub total_points: usize,
    pub done_points: usize,
}

impl Sim {
    pub fn new(mat: &Material, toolpaths: &[&Toolpath]) -> Sim {
        let stock = Stock::new(mat);
        let items: Vec<(Kernel, Arc<Chains>)> = toolpaths.iter().map(|t| (Kernel::new(&t.tool, stock.cell), t.chains.clone())).collect();
        let total_points = items.iter().map(|(_, c)| c.iter().map(|v| v.len()).sum::<usize>()).sum();
        Sim { stock, items, ti: 0, ci: 0, pi: 0, done: false, revision: 1, total_points, done_points: 0 }
    }

    pub fn progress(&self) -> f32 {
        if self.done {
            1.0
        } else {
            self.done_points as f32 / self.total_points.max(1) as f32
        }
    }

    pub fn finish(&mut self) {
        while !self.done {
            self.advance(usize::MAX / 4);
        }
    }

    /// Cut up to `budget` cell updates worth of moves.
    pub fn advance(&mut self, budget: usize) {
        let mut used = 0usize;
        let cell = self.stock.cell;
        while used < budget {
            let Some((kernel, chains)) = self.items.get(self.ti) else {
                self.done = true;
                break;
            };
            let Some(chain) = chains.get(self.ci) else {
                self.ti += 1;
                self.ci = 0;
                self.pi = 0;
                continue;
            };
            if self.pi + 1 >= chain.len() {
                if chain.len() == 1 && self.pi == 0 {
                    stamp(&mut self.stock, kernel, chain[0]);
                }
                self.ci += 1;
                self.pi = 0;
                self.done_points += chain.len();
                continue;
            }
            let a = chain[self.pi];
            let b = chain[self.pi + 1];
            let len = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
            let n = (len / (cell * 0.7)).ceil().max(1.0) as usize;
            for k in 0..=n {
                let t = k as f32 / n as f32;
                stamp(&mut self.stock, kernel, [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]);
            }
            used += n * kernel.offs.len();
            self.pi += 1;
        }
        self.revision += 1;
    }
}

fn stamp(s: &mut Stock, k: &Kernel, p: [f32; 3]) {
    let ci = (p[0] / s.cell - 0.5).round() as i32;
    let cj = (p[1] / s.cell - 0.5).round() as i32;
    let floor = -s.thickness;
    for &(di, dj, dz) in &k.offs {
        let (i, j) = (ci + di, cj + dj);
        if i < 0 || j < 0 || i as usize >= s.nx || j as usize >= s.ny {
            continue;
        }
        let idx = j as usize * s.nx + i as usize;
        let z = (p[2] + dz).max(floor);
        if z < s.z[idx] {
            s.z[idx] = z;
        }
    }
}

// ---------------------------------------------------------------- G-code

/// Write the toolpaths as G-code for the chosen machine (the material's post-processor).
///
/// Every chain becomes: retract to safe Z, rapid over its start, feed down at the plunge rate,
/// then cut at the feed rate. Level runs that follow a circle are written as G2/G3 arcs.
pub fn gcode(paths: &[&Toolpath], mat: &Material) -> String {
    use std::fmt::Write;
    let post = mat.post;
    let inch = post.inch();
    let k = if inch { 1.0 / 25.4 } else { 1.0 };
    let dp = if inch { 4 } else { 3 };
    let (ox, oy) = match mat.origin {
        Origin::BottomLeft => (0.0, 0.0),
        Origin::Center => (mat.w * 0.5, mat.h * 0.5),
    };
    // Z values are generated relative to the material top; shift them when Z zero is the bed.
    let oz = if mat.z_bed { mat.thickness } else { 0.0 };
    let n = |v: f32| format!("{:.*}", dp, v * k);
    let xy = |x: f32, y: f32| format!("X{} Y{}", n(x - ox), n(y - oy));
    let zz = |z: f32| format!("Z{}", n(z + oz));
    let safe = zz(mat.safe_z);
    let feed = |f: f32| format!("F{:.1}", f * k);
    let mut s = String::new();
    let _ = writeln!(s, "( CarveCraft Studio · {} )", post.label());
    let _ = writeln!(s, "( Material {:.2} x {:.2} x {:.2} mm, Z zero on the {} )", mat.w, mat.h, mat.thickness, if mat.z_bed { "machine bed" } else { "material surface" });
    for tp in paths {
        let _ = writeln!(s, "( {} – {} Ø{} mm, tool #{} )", tp.name, tp.tool.name, tp.tool.diameter, tp.tool.number);
    }
    let _ = writeln!(s, "{} G90 G17 G94", if inch { "G20" } else { "G21" });
    if !matches!(post, Post::GrblMm | Post::GrblInch) {
        let _ = writeln!(s, "G40 G49 G80");
    }
    if post == Post::LinuxCncMm {
        // path blending tolerance: stay within 0.01 mm of the programmed path
        let _ = writeln!(s, "G64 P0.01");
    }
    let _ = writeln!(s, "G0 {safe}");
    let mut last_tool = u32::MAX;
    for tp in paths {
        let _ = writeln!(s, "( Toolpath: {} )", tp.name);
        if tp.tool.number != last_tool {
            let _ = writeln!(s, "M5");
            if post.tool_change() {
                let _ = writeln!(s, "T{} M6", tp.tool.number);
            } else {
                let _ = writeln!(s, "( Change to tool #{}: {} – resume when ready )\nM0", tp.tool.number, tp.tool.name);
            }
            let _ = writeln!(s, "S{:.0} M3", tp.tool.rpm);
            last_tool = tp.tool.number;
        }
        for ch in tp.chains.iter() {
            let Some(f) = ch.first() else { continue };
            let _ = writeln!(s, "G0 {safe}");
            let _ = writeln!(s, "G0 {}", xy(f[0], f[1]));
            let _ = writeln!(s, "G1 {} {}", zz(f[2]), feed(tp.tool.plunge));
            let mut first = true;
            let mut at = *f;
            for m in cam::fit_arcs(ch, 0.005) {
                let f_word = if first { format!(" {}", feed(tp.tool.feed)) } else { String::new() };
                match m {
                    cam::Move::Line(p) => {
                        let _ = writeln!(s, "G1 {} {}{f_word}", xy(p[0], p[1]), zz(p[2]));
                        at = p;
                    }
                    cam::Move::Arc { end, centre, cw } => {
                        // I and J are measured from the arc's start point.
                        let _ = writeln!(s, "{} {} {} I{} J{}{f_word}", if cw { "G2" } else { "G3" }, xy(end[0], end[1]), zz(end[2]), n(centre.x - at[0]), n(centre.y - at[1]));
                        at = end;
                    }
                }
                first = false;
            }
        }
        let _ = writeln!(s, "G0 {safe}");
    }
    let _ = writeln!(s, "M5\nG0 {}\nM30", xy(ox, oy));
    s
}

pub fn material_rect(mat: &Material) -> Rect {
    Rect::from_min_max(pos2(0.0, 0.0), pos2(mat.w, mat.h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ops_produce_moves() {
        let mat = Material::default();
        let mut model = Model::new(mat.w, mat.h);
        model.rebuild();
        let text: Vec<Poly> = geom::text_outlines("CARVE", 30.0)
            .into_iter()
            .map(|p| Poly { pts: p.pts.iter().map(|q| pos2(q.x + 150.0, q.y + 100.0)).collect(), closed: p.closed })
            .collect();
        let circle = vec![Poly {
            pts: (0..64)
                .map(|k| {
                    let a = k as f32 / 64.0 * std::f32::consts::TAU;
                    pos2(60.0 + 20.0 * a.cos(), 60.0 + 20.0 * a.sin())
                })
                .collect(),
            closed: true,
        }];
        let tools = Tool::defaults();
        let cases = [
            (Op::VCarve { flat_depth: 0.0 }, &tools[4], &text),
            (Op::pocket(3.0, true), &tools[1], &circle),
            (Op::pocket(3.0, false), &tools[1], &circle),
            (Op::profile(6.0, Side::Inside), &tools[0], &circle),
            (Op::Drill { depth: 6.0 }, &tools[0], &circle),
            (Op::Fluting { depth: 2.0 }, &tools[2], &circle),
        ];
        for (op, tool, polys) in cases {
            let t0 = std::time::Instant::now();
            let ch = generate(&op, tool, polys, &model, &mat);
            let pts: usize = ch.iter().map(|c| c.len()).sum();
            eprintln!("{:<14} chains {:5} points {:7} in {:?}", op.label(), ch.len(), pts, t0.elapsed());
            assert!(!ch.is_empty(), "{} produced nothing", op.label());
            let deepest = ch.iter().flatten().map(|p| p[2]).fold(0.0f32, f32::min);
            assert!(deepest < 0.0, "{} never cuts", op.label());
        }
    }
}

#[cfg(test)]
mod accuracy_tests {
    use super::*;

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Vec<Poly> {
        vec![Poly { pts: vec![pos2(x, y), pos2(x + w, y), pos2(x + w, y + h), pos2(x, y + h)], closed: true }]
    }

    fn tool(dia: f32) -> Tool {
        let mut t = Tool::defaults()[0].clone();
        t.diameter = dia;
        t.pass_depth = 3.0;
        t.stepover = dia * 0.4;
        t
    }

    fn empty_model(mat: &Material) -> Model {
        let mut m = Model::new(mat.w, mat.h);
        m.rebuild();
        m
    }

    #[test]
    fn outside_profile_is_exactly_one_radius_off_and_climbs() {
        let mat = Material::default();
        let model = empty_model(&mat);
        let part = rect(50.0, 50.0, 100.0, 50.0);
        let ch = generate(&Op::profile(6.0, Side::Outside), &tool(6.0), &part, &model, &mat);
        assert_eq!(ch.len(), 2, "two 3 mm passes");
        for c in &ch {
            for p in c {
                let d = geom::dist_to_polys(pos2(p[0], p[1]), &part);
                assert!((d - 3.0).abs() < 0.003, "tool centre {d} mm from the part");
            }
        }
        let loop_pts: Vec<Pos2> = ch[0].iter().map(|p| pos2(p[0], p[1])).collect();
        assert!(geom::signed_area(&loop_pts) < 0.0, "climb on an outside cut runs clockwise");
        assert_eq!(ch[1].iter().map(|p| p[2]).fold(0.0, f32::min), -6.0);
    }

    #[test]
    fn inside_profile_with_allowance() {
        let mat = Material::default();
        let model = empty_model(&mat);
        let hole = rect(50.0, 50.0, 60.0, 40.0);
        let mut op = Op::profile(3.0, Side::Inside);
        if let Op::Profile { allowance, .. } = &mut op {
            *allowance = 0.5;
        }
        let ch = generate(&op, &tool(6.0), &hole, &model, &mat);
        for p in &ch[0] {
            let d = geom::dist_to_polys(pos2(p[0], p[1]), &hole);
            assert!((d - 3.5).abs() < 0.003, "3 mm radius + 0.5 mm allowance, got {d}");
        }
    }

    #[test]
    fn tabs_only_on_passes_below_the_tab() {
        let mat = Material::default();
        let model = empty_model(&mat);
        let part = rect(50.0, 50.0, 100.0, 50.0);
        let mut op = Op::profile(9.0, Side::Outside);
        if let Op::Profile { tabs, .. } = &mut op {
            *tabs = cam::Tabs { on: true, count: 4, length: 8.0, thickness: 4.0 };
        }
        let ch = generate(&op, &tool(6.0), &part, &model, &mat);
        assert_eq!(ch.len(), 3);
        let top = |c: &Vec<[f32; 3]>| c.iter().map(|p| p[2]).fold(f32::MIN, f32::max);
        assert!((top(&ch[0]) + 3.0).abs() < 1e-4, "first pass (-3) is above the tab top (-5): no lift");
        assert!((top(&ch[2]) + 5.0).abs() < 1e-3, "last pass rides up to the tab top");
    }

    #[test]
    fn offset_pocket_stays_inside_and_leaves_nothing() {
        let mat = Material::default();
        let model = empty_model(&mat);
        let shape = rect(50.0, 50.0, 60.0, 40.0);
        let t = tool(6.0);
        for raster in [false, true] {
            let ch = generate(&Op::pocket(3.0, raster), &t, &shape, &model, &mat);
            // never closer to the wall than the tool radius
            for c in &ch {
                for p in c {
                    let q = pos2(p[0], p[1]);
                    assert!(geom::point_inside(q, &shape));
                    assert!(geom::dist_to_polys(q, &shape) > 3.0 - 0.003);
                }
            }
            // every spot of the pocket is within the cutter's reach of some move
            let mut worst: f32 = 0.0;
            for i in 0..=60 {
                for j in 0..=40 {
                    let q = pos2(50.0 + i as f32, 50.0 + j as f32);
                    let d = ch.iter().flat_map(|c| c.windows(2)).map(|w| geom::seg_dist(q, pos2(w[0][0], w[0][1]), pos2(w[1][0], w[1][1]))).fold(f32::MAX, f32::min);
                    // A round cutter can't reach the very corners: only check spots within one
                    // radius of where its centre may go (the pocket shrunk by the radius).
                    let centre_area = Rect::from_min_max(pos2(53.0, 53.0), pos2(107.0, 87.0));
                    let reach = (q - centre_area.clamp(q)).length();
                    if reach <= 3.0 - 1e-3 {
                        worst = worst.max(d);
                    }
                }
            }
            assert!(worst <= 3.0 + 1e-3, "an uncut spot {worst} mm from the nearest move (raster={raster})");
            assert!(ch.len() <= 2, "passes are linked without lifting (raster={raster}, {} chains)", ch.len());
        }
    }

    /// Replay G-code (G0/G1/G2/G3, absolute, relative I/J) and return every position reached.
    fn replay(code: &str) -> Vec<[f32; 3]> {
        let mut at = [0.0f32; 3];
        let mut out = Vec::new();
        for line in code.lines() {
            let words: Vec<&str> = line.split_whitespace().filter(|w| !w.starts_with('(')).collect();
            if line.trim_start().starts_with('(') || words.is_empty() {
                continue;
            }
            let get = |c: char| words.iter().find(|w| w.starts_with(c)).and_then(|w| w[1..].parse::<f32>().ok());
            let g = words[0];
            if !["G0", "G1", "G2", "G3"].contains(&g) {
                continue;
            }
            let end = [get('X').unwrap_or(at[0]), get('Y').unwrap_or(at[1]), get('Z').unwrap_or(at[2])];
            if g == "G2" || g == "G3" {
                let c = (at[0] + get('I').unwrap(), at[1] + get('J').unwrap());
                let r0 = ((at[0] - c.0).powi(2) + (at[1] - c.1).powi(2)).sqrt();
                let r1 = ((end[0] - c.0).powi(2) + (end[1] - c.1).powi(2)).sqrt();
                assert!((r0 - r1).abs() < 0.003, "arc start and end are on one circle: {line}");
            }
            at = end;
            out.push(at);
        }
        out
    }

    #[test]
    fn gcode_arcs_land_on_the_path_in_mm_and_inch() {
        let mut mat = Material::default();
        let model = empty_model(&mat);
        let part = rect(50.0, 50.0, 100.0, 50.0);
        let mut tp = Toolpath { id: 1, name: "Profile".into(), op: Op::profile(3.0, Side::Outside), tool: tool(6.0), polys: part.clone(), visible: true, chains: Arc::new(Vec::new()), length: 0.0, minutes: 0.0 };
        tp.calculate(&model, &mat);
        let code = gcode(&[&tp], &mat);
        assert!(code.contains("G21"));
        assert!(code.lines().any(|l| l.starts_with("G2 ")), "rounded corners come out as arcs");
        let lines_mm = code.lines().count();
        // every replayed cutting position is exactly one radius from the part
        for p in replay(&code).iter().filter(|p| p[2] < 0.0) {
            let d = geom::dist_to_polys(pos2(p[0], p[1]), &part);
            assert!((d - 3.0).abs() < 0.01, "{p:?} is {d} mm from the part");
        }
        mat.post = Post::Mach3Inch;
        let inch = gcode(&[&tp], &mat);
        assert!(inch.contains("G20") && inch.contains("T1 M6"));
        let far = replay(&inch).iter().map(|p| p[0]).fold(0.0, f32::max);
        assert!((far - 153.0 / 25.4).abs() < 0.001, "right edge 153 mm = {} in, got {far}", 153.0 / 25.4);
        assert!(lines_mm < 60, "arcs keep the file short ({lines_mm} lines)");
    }
}
