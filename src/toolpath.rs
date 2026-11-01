//! Tools, toolpath generation, cutting simulation and G-code output.

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
}

impl Default for Material {
    fn default() -> Self {
        Self { w: 300.0, h: 200.0, thickness: 20.0, origin: Origin::BottomLeft, safe_z: 5.0, z_bed: false }
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
    Profile { depth: f32, side: Side },
    Pocket { depth: f32, raster: bool },
    Drill { depth: f32 },
    VCarve { flat_depth: f32 },
    Engrave { depth: f32 },
    Fluting { depth: f32 },
    Rough3D { allowance: f32 },
    Finish3D,
    Texture { depth: f32, length: f32 },
}

impl Op {
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
        Op::Profile { depth, side } => {
            let loops: Vec<Poly> = match side {
                Side::On => polys.to_vec(),
                Side::Outside => geom::offset_polys(&closed_only(polys), r),
                Side::Inside => geom::offset_polys(&closed_only(polys), -r),
            };
            for z in pass_levels(*depth, tool.pass_depth) {
                for l in &loops {
                    let pts = if l.closed { close(&l.pts) } else { l.pts.clone() };
                    out.push(chain_at(&pts, z));
                }
            }
        }
        Op::Pocket { depth, raster } => {
            let closed = closed_only(polys);
            if closed.is_empty() {
                return out;
            }
            let cell = field_cell(&closed, 600.0, 0.05, 0.5);
            let f = Field::build(&closed, cell, cell * 3.0);
            let so = tool.stepover.max(0.05);
            let finish = f.contours(r);
            for z in pass_levels(*depth, tool.pass_depth) {
                if *raster {
                    let b = geom::bounds_of(&closed);
                    let mut y = b.min.y + r + so * 0.5;
                    let mut flip = false;
                    while y < b.max.y - r {
                        let mut segs: Vec<Vec<[f32; 3]>> = Vec::new();
                        let mut cur: Vec<[f32; 3]> = Vec::new();
                        let mut x = b.min.x;
                        let step = cell * 0.5;
                        while x <= b.max.x {
                            let ins = f.sample(pos2(x, y)) >= r;
                            if ins {
                                if cur.is_empty() {
                                    cur.push([x, y, z]);
                                }
                            } else if !cur.is_empty() {
                                cur.push([x - step, y, z]);
                                segs.push(std::mem::take(&mut cur));
                            }
                            x += step;
                        }
                        if !cur.is_empty() {
                            cur.push([b.max.x, y, z]);
                            segs.push(cur);
                        }
                        if flip {
                            segs.reverse();
                            for s in &mut segs {
                                s.reverse();
                            }
                        }
                        out.extend(segs.into_iter().filter(|s| s.len() >= 2));
                        flip = !flip;
                        y += so;
                    }
                } else {
                    let mut level = r + so;
                    let mut rings = Vec::new();
                    for _ in 0..400 {
                        let c = f.contours(level);
                        if c.is_empty() {
                            break;
                        }
                        rings.push(c);
                        level += so;
                    }
                    for ring in rings.iter().rev() {
                        for l in ring {
                            out.push(chain_at(&close(l), z));
                        }
                    }
                }
                for l in &finish {
                    out.push(chain_at(&close(l), z));
                }
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

pub fn gcode(paths: &[&Toolpath], mat: &Material) -> String {
    use std::fmt::Write;
    let (ox, oy) = match mat.origin {
        Origin::BottomLeft => (0.0, 0.0),
        Origin::Center => (mat.w * 0.5, mat.h * 0.5),
    };
    // Z values are generated relative to the material top; shift them when Z zero is the bed.
    let oz = if mat.z_bed { mat.thickness } else { 0.0 };
    let safe = mat.safe_z + oz;
    let mut s = String::new();
    let _ = writeln!(s, "( CarveCraft G-code, units mm )");
    let zero = if mat.z_bed { "machine bed" } else { "material surface" };
    let _ = writeln!(s, "( Material {:.2} x {:.2} x {:.2}, Z zero on {zero} )", mat.w, mat.h, mat.thickness);
    let _ = writeln!(s, "G21 G90 G17 G94");
    let _ = writeln!(s, "G0 Z{:.3}", safe);
    let mut last_tool = u32::MAX;
    for tp in paths {
        let _ = writeln!(s, "( Toolpath: {} - {} )", tp.name, tp.tool.name);
        if tp.tool.number != last_tool {
            let _ = writeln!(s, "M5\nT{} M6\nS{:.0} M3", tp.tool.number, tp.tool.rpm);
            last_tool = tp.tool.number;
        }
        for ch in tp.chains.iter() {
            let Some(f) = ch.first() else { continue };
            let _ = writeln!(s, "G0 Z{:.3}", safe);
            let _ = writeln!(s, "G0 X{:.3} Y{:.3}", f[0] - ox, f[1] - oy);
            let _ = writeln!(s, "G1 Z{:.3} F{:.0}", f[2] + oz, tp.tool.plunge);
            let mut first = true;
            for p in ch.iter().skip(1) {
                if first {
                    let _ = writeln!(s, "G1 X{:.3} Y{:.3} Z{:.3} F{:.0}", p[0] - ox, p[1] - oy, p[2] + oz, tp.tool.feed);
                    first = false;
                } else {
                    let _ = writeln!(s, "X{:.3} Y{:.3} Z{:.3}", p[0] - ox, p[1] - oy, p[2] + oz);
                }
            }
        }
        let _ = writeln!(s, "G0 Z{:.3}", safe);
    }
    let _ = writeln!(s, "M5\nG0 X{:.3} Y{:.3}\nM30", -ox, -oy);
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
            (Op::Pocket { depth: 3.0, raster: true }, &tools[1], &circle),
            (Op::Pocket { depth: 3.0, raster: false }, &tools[1], &circle),
            (Op::Profile { depth: 6.0, side: Side::Inside }, &tools[0], &circle),
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
