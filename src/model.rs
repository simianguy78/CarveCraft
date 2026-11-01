//! 3D relief model: a stack of height-map components plus a sculpt layer.

use crate::geom::{self, Field, Poly};
use eframe::egui::{pos2, vec2, Pos2, Rect};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Profile {
    Round,
    Angle,
    Flat,
    Ogee,
}

impl Profile {
    pub const ALL: [Profile; 4] = [Profile::Round, Profile::Angle, Profile::Flat, Profile::Ogee];
    pub fn label(self) -> &'static str {
        match self {
            Profile::Round => "Round",
            Profile::Angle => "Angular",
            Profile::Flat => "Flat",
            Profile::Ogee => "Ogee",
        }
    }
    /// Height factor for normalised position `t` (0 at edge, 1 at middle/top).
    pub fn eval(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Profile::Round => (1.0 - (1.0 - t) * (1.0 - t)).sqrt(),
            Profile::Angle => t,
            Profile::Flat => 1.0,
            Profile::Ogee => t * t * (3.0 - 2.0 * t),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Combine {
    Add,
    Subtract,
    Merge,
    Low,
}

impl Combine {
    pub const ALL: [Combine; 4] = [Combine::Add, Combine::Subtract, Combine::Merge, Combine::Low];
    pub fn label(self) -> &'static str {
        match self {
            Combine::Add => "Add",
            Combine::Subtract => "Subtract",
            Combine::Merge => "Merge",
            Combine::Low => "Low",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextureKind {
    Waves,
    Ripple,
    Hammered,
    WoodGrain,
    Basket,
}

impl TextureKind {
    pub const ALL: [TextureKind; 5] = [TextureKind::Waves, TextureKind::Ripple, TextureKind::Hammered, TextureKind::WoodGrain, TextureKind::Basket];
    pub fn label(self) -> &'static str {
        match self {
            TextureKind::Waves => "Waves",
            TextureKind::Ripple => "Ripple",
            TextureKind::Hammered => "Hammered",
            TextureKind::WoodGrain => "Wood grain",
            TextureKind::Basket => "Basket weave",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReliefKind {
    Rosette,
    Shell,
    Leaf,
    Heart,
    WavePanel,
    Button,
}

impl ReliefKind {
    pub const ALL: [ReliefKind; 6] = [ReliefKind::Rosette, ReliefKind::Shell, ReliefKind::Leaf, ReliefKind::Heart, ReliefKind::WavePanel, ReliefKind::Button];
    pub fn label(self) -> &'static str {
        match self {
            ReliefKind::Rosette => "Rosette",
            ReliefKind::Shell => "Shell",
            ReliefKind::Leaf => "Leaf",
            ReliefKind::Heart => "Heart",
            ReliefKind::WavePanel => "Wave panel",
            ReliefKind::Button => "Dome button",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Source {
    Shape { polys: Vec<Poly>, profile: Profile, height: f32, angle: f32, base: f32, limit: bool },
    Extrude { polys: Vec<Poly>, profile: Profile, width: f32, height: f32 },
    TwoRail { a: Poly, b: Poly, profile: Profile, height: f32 },
    Texture { polys: Vec<Poly>, kind: TextureKind, depth: f32, size: f32 },
    Relief { kind: ReliefKind, rect: Rect, height: f32 },
    Image { w: usize, h: usize, data: Vec<f32>, rect: Rect, height: f32 },
}

impl Source {
    pub fn kind_label(&self) -> &'static str {
        match self {
            Source::Shape { profile: Profile::Angle, .. } => "Prism",
            Source::Shape { .. } => "Shape",
            Source::Extrude { .. } => "Extrusion",
            Source::TwoRail { .. } => "Two-rail sweep",
            Source::Texture { .. } => "Texture",
            Source::Relief { .. } => "Relief",
            Source::Image { .. } => "Height map",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Component {
    pub id: u64,
    pub name: String,
    pub source: Source,
    pub mode: Combine,
    pub visible: bool,
    pub scale: f32,
    pub offset: f32,
    pub tilt: f32,
    pub tilt_dir: f32,
    pub fade: f32,
    pub fade_dir: f32,
    pub invert: bool,
    #[serde(skip)]
    pub cache: Arc<Vec<f32>>,
    #[serde(skip)]
    pub bounds: Option<Rect>,
}

impl Component {
    pub fn new(id: u64, name: impl Into<String>, source: Source, mode: Combine) -> Self {
        Self {
            id,
            name: name.into(),
            source,
            mode,
            visible: true,
            scale: 1.0,
            offset: 0.0,
            tilt: 0.0,
            tilt_dir: 90.0,
            fade: 1.0,
            fade_dir: 0.0,
            invert: false,
            cache: Arc::new(Vec::new()),
            bounds: None,
        }
    }
}

/// The model grid covers the material from (0,0) to (w,h).
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Grid {
    pub nx: usize,
    pub ny: usize,
    pub cell: f32,
}

impl Grid {
    pub fn for_material(w: f32, h: f32) -> Grid {
        let cell = (w.max(h) / 512.0).max(0.05);
        Grid { nx: (w / cell).round().max(2.0) as usize, ny: (h / cell).round().max(2.0) as usize, cell }
    }
    pub fn pos(&self, i: usize, j: usize) -> Pos2 {
        pos2((i as f32 + 0.5) * self.cell, (j as f32 + 0.5) * self.cell)
    }
    pub fn len(&self) -> usize {
        self.nx * self.ny
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Model {
    pub grid: Grid,
    pub components: Vec<Component>,
    pub sculpt: Vec<f32>,
    #[serde(skip)]
    pub composite: Arc<Vec<f32>>,
    #[serde(skip)]
    pub revision: u64,
}

impl Model {
    pub fn new(w: f32, h: f32) -> Self {
        let grid = Grid::for_material(w, h);
        Self { grid, components: Vec::new(), sculpt: vec![0.0; grid.len()], composite: Arc::new(vec![0.0; grid.len()]), revision: 1 }
    }

    pub fn resize(&mut self, w: f32, h: f32) {
        let grid = Grid::for_material(w, h);
        if grid != self.grid {
            self.grid = grid;
            self.sculpt = vec![0.0; grid.len()];
            for c in &mut self.components {
                c.cache = Arc::new(Vec::new());
            }
            self.rebuild();
        }
    }

    pub fn max_height(&self) -> f32 {
        self.composite.iter().copied().fold(0.0, f32::max)
    }

    pub fn min_height(&self) -> f32 {
        self.composite.iter().copied().fold(0.0, f32::min)
    }

    pub fn is_empty(&self) -> bool {
        self.max_height() - self.min_height() < 1e-4
    }

    /// Regenerate any component caches that are missing, then composite.
    pub fn rebuild(&mut self) {
        let grid = self.grid;
        if self.sculpt.len() != grid.len() {
            self.sculpt = vec![0.0; grid.len()];
        }
        for c in &mut self.components {
            if c.cache.len() != grid.len() {
                let (data, b) = generate(&c.source, grid);
                c.cache = Arc::new(data);
                c.bounds = b;
            }
        }
        self.composite_only();
    }

    pub fn invalidate(&mut self, idx: usize) {
        if let Some(c) = self.components.get_mut(idx) {
            c.cache = Arc::new(Vec::new());
        }
        self.rebuild();
    }

    pub fn composite_only(&mut self) {
        let g = self.grid;
        let mut z = vec![0.0f32; g.len()];
        for c in self.components.iter().filter(|c| c.visible) {
            let b = c.bounds.unwrap_or(Rect::from_min_max(pos2(0.0, 0.0), pos2(g.nx as f32 * g.cell, g.ny as f32 * g.cell)));
            let center = b.center();
            let (ts, tc) = c.tilt_dir.to_radians().sin_cos();
            let tilt = c.tilt.to_radians().tan();
            let (fs, fc) = c.fade_dir.to_radians().sin_cos();
            // projection extent for fade
            let half = (b.width() * fc.abs() + b.height() * fs.abs()) * 0.5;
            for j in 0..g.ny {
                for i in 0..g.nx {
                    let k = j * g.nx + i;
                    let h = c.cache[k];
                    if h.is_nan() {
                        continue;
                    }
                    let p = g.pos(i, j);
                    let mut v = h * c.scale;
                    if (c.fade - 1.0).abs() > 1e-4 && half > 0.0 {
                        let t = (((p - center).dot(vec2(fc, fs)) / half) * 0.5 + 0.5).clamp(0.0, 1.0);
                        v *= 1.0 + (c.fade - 1.0) * t;
                    }
                    if c.invert {
                        v = -v;
                    }
                    v += c.offset;
                    if tilt != 0.0 {
                        v += (p - center).dot(vec2(tc, ts)) * tilt;
                    }
                    z[k] = match c.mode {
                        Combine::Add => z[k] + v,
                        Combine::Subtract => z[k] - v.abs(),
                        Combine::Merge => z[k].max(v),
                        Combine::Low => z[k].min(v),
                    };
                }
            }
        }
        for (a, s) in z.iter_mut().zip(&self.sculpt) {
            *a += *s;
        }
        self.composite = Arc::new(z);
        self.revision += 1;
    }

    pub fn height_at(&self, p: Pos2) -> f32 {
        sample_grid(&self.composite, self.grid, p)
    }

    /// Bake everything (including sculpting) into one height-map component.
    pub fn bake(&mut self, id: u64) {
        let g = self.grid;
        let data = self.composite.as_ref().clone();
        let rect = Rect::from_min_max(pos2(0.0, 0.0), pos2(g.nx as f32 * g.cell, g.ny as f32 * g.cell));
        let mut c = Component::new(id, "Baked model", Source::Image { w: g.nx, h: g.ny, data, rect, height: 1.0 }, Combine::Add);
        c.cache = Arc::new(Vec::new());
        self.components = vec![c];
        self.sculpt = vec![0.0; g.len()];
        self.rebuild();
    }

    pub fn brush(&mut self, kind: Brush, at: Pos2, radius: f32, strength: f32, target: f32) {
        let g = self.grid;
        let r = radius.max(g.cell);
        let i0 = ((at.x - r) / g.cell).floor().max(0.0) as usize;
        let i1 = (((at.x + r) / g.cell).ceil() as usize).min(g.nx.saturating_sub(1));
        let j0 = ((at.y - r) / g.cell).floor().max(0.0) as usize;
        let j1 = (((at.y + r) / g.cell).ceil() as usize).min(g.ny.saturating_sub(1));
        if i0 > i1 || j0 > j1 {
            return;
        }
        let comp = self.composite.clone();
        for j in j0..=j1 {
            for i in i0..=i1 {
                let d = (g.pos(i, j) - at).length();
                if d > r {
                    continue;
                }
                let f = {
                    let t = d / r;
                    (1.0 - t * t).powi(2) * strength
                };
                let k = j * g.nx + i;
                match kind {
                    Brush::Deposit => self.sculpt[k] += 0.12 * f,
                    Brush::Remove => self.sculpt[k] -= 0.12 * f,
                    Brush::Erase => self.sculpt[k] *= 1.0 - 0.5 * f,
                    Brush::Smooth => {
                        let mut s = 0.0;
                        let mut n: f32 = 0.0;
                        for (di, dj) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1), (-1, -1), (1, 1), (-1, 1), (1, -1)] {
                            let (ii, jj) = (i as i32 + di, j as i32 + dj);
                            if ii >= 0 && jj >= 0 && (ii as usize) < g.nx && (jj as usize) < g.ny {
                                s += comp[jj as usize * g.nx + ii as usize];
                                n += 1.0;
                            }
                        }
                        let avg = s / n.max(1.0);
                        self.sculpt[k] += (avg - comp[k]) * 0.6 * f;
                    }
                    Brush::Flatten => self.sculpt[k] += (target - comp[k]) * 0.3 * f,
                }
            }
        }
        self.composite_only();
    }

    pub fn smooth_all(&mut self, passes: usize) {
        let g = self.grid;
        let mut z = self.composite.as_ref().clone();
        for _ in 0..passes {
            let src = z.clone();
            for j in 0..g.ny {
                for i in 0..g.nx {
                    let mut s = 0.0;
                    let mut n = 0.0;
                    for dj in -1i32..=1 {
                        for di in -1i32..=1 {
                            let (ii, jj) = (i as i32 + di, j as i32 + dj);
                            if ii >= 0 && jj >= 0 && (ii as usize) < g.nx && (jj as usize) < g.ny {
                                s += src[jj as usize * g.nx + ii as usize];
                                n += 1.0;
                            }
                        }
                    }
                    z[j * g.nx + i] = s / n;
                }
            }
        }
        for k in 0..g.len() {
            self.sculpt[k] += z[k] - self.composite[k];
        }
        self.composite_only();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Brush {
    Smooth,
    Deposit,
    Remove,
    Erase,
    Flatten,
}

impl Brush {
    pub const ALL: [Brush; 5] = [Brush::Smooth, Brush::Deposit, Brush::Remove, Brush::Erase, Brush::Flatten];
    pub fn label(self) -> &'static str {
        match self {
            Brush::Smooth => "Smooth",
            Brush::Deposit => "Deposit",
            Brush::Remove => "Remove",
            Brush::Erase => "Erase",
            Brush::Flatten => "Flatten",
        }
    }
}

pub fn sample_grid(data: &[f32], g: Grid, p: Pos2) -> f32 {
    if data.len() != g.len() {
        return 0.0;
    }
    let fx = (p.x / g.cell - 0.5).clamp(0.0, (g.nx - 1) as f32);
    let fy = (p.y / g.cell - 0.5).clamp(0.0, (g.ny - 1) as f32);
    let i = (fx as usize).min(g.nx - 2);
    let j = (fy as usize).min(g.ny - 2);
    let (tx, ty) = (fx - i as f32, fy - j as f32);
    let v = |i: usize, j: usize| {
        let x = data[j * g.nx + i];
        if x.is_nan() {
            0.0
        } else {
            x
        }
    };
    let a = v(i, j) * (1.0 - tx) + v(i + 1, j) * tx;
    let b = v(i, j + 1) * (1.0 - tx) + v(i + 1, j + 1) * tx;
    a * (1.0 - ty) + b * ty
}

fn field_on_grid(polys: &[Poly], g: Grid) -> Field {
    // grid points are cell centres
    Field::build_on(polys, pos2(g.cell * 0.5, g.cell * 0.5), g.cell, g.nx, g.ny, f32::MAX)
}

fn hash2(i: i32, j: i32) -> f32 {
    let mut h = (i as u32).wrapping_mul(374761393) ^ (j as u32).wrapping_mul(668265263);
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    ((h ^ (h >> 16)) & 0xffff) as f32 / 65535.0
}

fn value_noise(x: f32, y: f32) -> f32 {
    let (i, j) = (x.floor() as i32, y.floor() as i32);
    let (fx, fy) = (x - x.floor(), y - y.floor());
    let s = |t: f32| t * t * (3.0 - 2.0 * t);
    let a = hash2(i, j) + (hash2(i + 1, j) - hash2(i, j)) * s(fx);
    let b = hash2(i, j + 1) + (hash2(i + 1, j + 1) - hash2(i, j + 1)) * s(fx);
    a + (b - a) * s(fy)
}

pub fn texture_value(kind: TextureKind, p: Pos2, size: f32) -> f32 {
    let size = size.max(0.5);
    let (x, y) = (p.x / size, p.y / size);
    match kind {
        TextureKind::Waves => 0.5 + 0.5 * (std::f32::consts::TAU * (x + 0.15 * (y * 0.8).sin())).sin(),
        TextureKind::Ripple => 0.5 + 0.5 * (std::f32::consts::TAU * (x * x + y * y).sqrt()).cos(),
        TextureKind::Hammered => {
            let (ci, cj) = (x.floor() as i32, y.floor() as i32);
            let mut best = f32::MAX;
            for dj in -1..=1 {
                for di in -1..=1 {
                    let (a, b) = (ci + di, cj + dj);
                    let fx = a as f32 + hash2(a, b);
                    let fy = b as f32 + hash2(b, a + 17);
                    best = best.min(((x - fx).powi(2) + (y - fy).powi(2)).sqrt());
                }
            }
            (best * 1.4).min(1.0).powf(0.7)
        }
        TextureKind::WoodGrain => {
            let n = value_noise(x * 0.6, y * 3.0) * 0.6 + value_noise(x * 1.7, y * 7.0) * 0.4;
            0.5 + 0.5 * (std::f32::consts::TAU * (y * 1.3 + n * 2.5)).sin()
        }
        TextureKind::Basket => {
            let (ci, cj) = (x.floor() as i32, y.floor() as i32);
            let (fx, fy) = (x - x.floor(), y - y.floor());
            if (ci + cj) % 2 == 0 {
                (std::f32::consts::PI * fy).sin() * (0.85 + 0.15 * (std::f32::consts::TAU * fx * 3.0).cos())
            } else {
                (std::f32::consts::PI * fx).sin() * (0.85 + 0.15 * (std::f32::consts::TAU * fy * 3.0).cos())
            }
        }
    }
}

pub fn relief_value(kind: ReliefKind, u: f32, v: f32) -> f32 {
    use std::f32::consts::*;
    let r = (u * u + v * v).sqrt();
    let th = v.atan2(u);
    match kind {
        ReliefKind::Rosette => {
            if r > 1.0 {
                return f32::NAN;
            }
            let petals = 0.5 + 0.5 * (8.0 * th).cos();
            let inner = 0.5 + 0.5 * (8.0 * th + PI).cos();
            let outer_ring = (1.0 - r * r).sqrt() * (0.45 + 0.55 * petals.powf(0.6));
            let inner_ring = if r < 0.55 { (1.0 - (r / 0.55).powi(2)).sqrt() * (0.5 + 0.5 * inner.powf(0.6)) * 0.55 } else { 0.0 };
            let bead = if r < 0.18 { (1.0 - (r / 0.18).powi(2)).sqrt() * 0.35 } else { 0.0 };
            outer_ring * 0.7 + inner_ring + bead
        }
        ReliefKind::Shell => {
            // fan in the upper half, hinge at (0,-0.9)
            let (x, y) = (u, v + 0.9);
            let rr = (x * x + (y / 1.9).powi(2)).sqrt();
            if y < 0.0 || rr > 1.0 {
                return f32::NAN;
            }
            let a = x.atan2(y);
            let ribs = 0.5 + 0.5 * (13.0 * a).cos();
            (1.0 - rr * rr).sqrt() * (0.7 + 0.3 * ribs) * (0.6 + 0.4 * (y / 1.9))
        }
        ReliefKind::Leaf => {
            let w = 0.48 * (1.0 - u * u).max(0.0) * (1.0 + 0.25 * u);
            if v.abs() > w || w <= 0.0 {
                return f32::NAN;
            }
            let across = (1.0 - (v / w).powi(2)).sqrt();
            let along = (1.0 - u * u).max(0.0).sqrt();
            let vein = (-(v / 0.03).powi(2)).exp() * 0.2;
            let side = {
                let s = (u * 6.0 - v.abs() * 5.0) * PI;
                (-(s.sin() / 0.25).powi(2)).exp() * 0.08 * across
            };
            (across * along * 0.85 + 0.15 * along - vein - side).max(0.0)
        }
        ReliefKind::Heart => {
            let x = u * 1.25;
            let y = v * 1.25 + 0.15;
            let f = (x * x + y * y - 1.0).powi(3) - x * x * y * y * y;
            if f > 0.0 {
                return f32::NAN;
            }
            (-f).powf(0.33).min(1.0)
        }
        ReliefKind::WavePanel => {
            if u.abs() > 1.0 || v.abs() > 1.0 {
                return f32::NAN;
            }
            let edge = ((1.0 - u.abs()).min(1.0 - v.abs()) / 0.08).min(1.0);
            (0.5 + 0.5 * (3.0 * PI * u + 1.5 * (PI * v).sin()).sin()) * edge
        }
        ReliefKind::Button => {
            if r > 1.0 {
                return f32::NAN;
            }
            let dome = (1.0 - r * r).sqrt();
            let rim = if r > 0.82 { 0.15 } else { 0.0 };
            dome * 0.85 + rim
        }
    }
}

/// Produce heights for one component (NaN where it does not cover).
pub fn generate(src: &Source, g: Grid) -> (Vec<f32>, Option<Rect>) {
    let mut out = vec![f32::NAN; g.len()];
    match src {
        Source::Shape { polys, profile, height, angle, base, limit } => {
            let f = field_on_grid(polys, g);
            let dmax = f.max().max(1e-3);
            let tan = angle.to_radians().tan();
            for k in 0..g.len() {
                let d = f.d[k];
                if d <= 0.0 {
                    continue;
                }
                let h = match profile {
                    Profile::Angle => {
                        let v = d * tan;
                        if *limit {
                            v.min(*height)
                        } else {
                            v
                        }
                    }
                    p => *height * p.eval(d / dmax),
                };
                out[k] = base + h;
            }
        }
        Source::Extrude { polys, profile, width, height } => {
            let hw = (width * 0.5).max(0.1);
            for j in 0..g.ny {
                for i in 0..g.nx {
                    let p = g.pos(i, j);
                    let d = geom::dist_to_polys(p, polys);
                    if d < hw {
                        let h = match profile {
                            Profile::Round => (1.0 - (d / hw).powi(2)).sqrt(),
                            p => p.eval(1.0 - d / hw),
                        };
                        out[j * g.nx + i] = height * h;
                    }
                }
            }
        }
        Source::TwoRail { a, b, profile, height } => {
            let ra = std::slice::from_ref(a);
            let rb = std::slice::from_ref(b);
            for j in 0..g.ny {
                for i in 0..g.nx {
                    let p = g.pos(i, j);
                    let da = geom::dist_to_polys(p, ra);
                    let db = geom::dist_to_polys(p, rb);
                    let ca = geom::closest_on_poly(p, a);
                    let gap = geom::dist_to_polys(ca, rb).max(0.1);
                    if da + db <= gap * 1.02 + g.cell {
                        let t = da / (da + db).max(1e-4);
                        let u = 1.0 - (2.0 * t - 1.0).abs();
                        let h = match profile {
                            Profile::Round => (1.0 - (1.0 - u).powi(2)).sqrt(),
                            p => p.eval(u),
                        };
                        out[j * g.nx + i] = height * h * (gap / gap.max(1.0)).min(1.0);
                    }
                }
            }
        }
        Source::Texture { polys, kind, depth, size } => {
            for j in 0..g.ny {
                for i in 0..g.nx {
                    let p = g.pos(i, j);
                    if polys.is_empty() || geom::point_inside(p, polys) {
                        out[j * g.nx + i] = texture_value(*kind, p, *size) * depth;
                    }
                }
            }
        }
        Source::Relief { kind, rect, height } => {
            for j in 0..g.ny {
                for i in 0..g.nx {
                    let p = g.pos(i, j);
                    if !rect.contains(p) {
                        continue;
                    }
                    let u = (p.x - rect.center().x) / (rect.width() * 0.5);
                    let v = (p.y - rect.center().y) / (rect.height() * 0.5);
                    let h = relief_value(*kind, u, v);
                    if !h.is_nan() {
                        out[j * g.nx + i] = h * height;
                    }
                }
            }
        }
        Source::Image { w, h, data, rect, height } => {
            for j in 0..g.ny {
                for i in 0..g.nx {
                    let p = g.pos(i, j);
                    if !rect.contains(p) || *w < 2 || *h < 2 {
                        continue;
                    }
                    let fx = ((p.x - rect.min.x) / rect.width() * (*w as f32) - 0.5).clamp(0.0, (*w - 1) as f32);
                    let fy = ((p.y - rect.min.y) / rect.height() * (*h as f32) - 0.5).clamp(0.0, (*h - 1) as f32);
                    let (x0, y0) = ((fx as usize).min(w - 2), (fy as usize).min(h - 2));
                    let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
                    let s = |x: usize, y: usize| data[y * w + x];
                    let a = s(x0, y0) * (1.0 - tx) + s(x0 + 1, y0) * tx;
                    let b = s(x0, y0 + 1) * (1.0 - tx) + s(x0 + 1, y0 + 1) * tx;
                    out[j * g.nx + i] = (a * (1.0 - ty) + b * ty) * height;
                }
            }
        }
    }
    let mut b = Rect::NOTHING;
    for j in 0..g.ny {
        for i in 0..g.nx {
            if !out[j * g.nx + i].is_nan() {
                b.extend_with(g.pos(i, j));
            }
        }
    }
    (out, if b.is_positive() { Some(b) } else { None })
}
