//! 2D vector shapes, text outlines, distance fields and contour tracing.

use eframe::egui::{pos2, vec2, Pos2, Rect, Vec2};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ShapeKind {
    Rect { w: f32, h: f32, r: f32 },
    Ellipse { rx: f32, ry: f32 },
    Polygon { r: f32, sides: u32 },
    Star { r1: f32, r2: f32, points: u32 },
    Path { pts: Vec<Pos2>, closed: bool },
    Text { text: String, size: f32 },
}

impl ShapeKind {
    pub fn label(&self) -> &'static str {
        match self {
            ShapeKind::Rect { .. } => "Rectangle",
            ShapeKind::Ellipse { rx, ry } if (rx - ry).abs() < 1e-4 => "Circle",
            ShapeKind::Ellipse { .. } => "Ellipse",
            ShapeKind::Polygon { .. } => "Polygon",
            ShapeKind::Star { .. } => "Star",
            ShapeKind::Path { closed: true, .. } => "Closed curve",
            ShapeKind::Path { .. } => "Open curve",
            ShapeKind::Text { .. } => "Text",
        }
    }
}

/// One closed or open polyline in job coordinates (mm, y up).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Poly {
    pub pts: Vec<Pos2>,
    pub closed: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Vector {
    pub id: u64,
    pub kind: ShapeKind,
    pub center: Pos2,
    pub angle: f32,
    pub scale: Vec2,
    pub layer: usize,
}

impl Vector {
    pub fn new(id: u64, kind: ShapeKind, center: Pos2, layer: usize) -> Self {
        Self { id, kind, center, angle: 0.0, scale: vec2(1.0, 1.0), layer }
    }

    fn local_polys(&self) -> Vec<Poly> {
        match &self.kind {
            ShapeKind::Rect { w, h, r } => {
                let r = r.min(w * 0.5).min(h * 0.5).max(0.0);
                let (hw, hh) = (w * 0.5, h * 0.5);
                if r <= 0.001 {
                    return vec![Poly {
                        pts: vec![pos2(-hw, -hh), pos2(hw, -hh), pos2(hw, hh), pos2(-hw, hh)],
                        closed: true,
                    }];
                }
                let mut pts = Vec::new();
                let corners = [(hw - r, -hh + r, -90.0f32), (hw - r, hh - r, 0.0), (-hw + r, hh - r, 90.0), (-hw + r, -hh + r, 180.0)];
                for (cx, cy, a0) in corners {
                    for k in 0..=10 {
                        let a = (a0 + 9.0 * k as f32).to_radians();
                        pts.push(pos2(cx + r * a.cos(), cy + r * a.sin()));
                    }
                }
                vec![Poly { pts, closed: true }]
            }
            ShapeKind::Ellipse { rx, ry } => {
                let n = 128;
                let pts = (0..n)
                    .map(|k| {
                        let a = k as f32 / n as f32 * std::f32::consts::TAU;
                        pos2(rx * a.cos(), ry * a.sin())
                    })
                    .collect();
                vec![Poly { pts, closed: true }]
            }
            ShapeKind::Polygon { r, sides } => {
                let n = (*sides).max(3);
                let pts = (0..n)
                    .map(|k| {
                        let a = std::f32::consts::FRAC_PI_2 + k as f32 / n as f32 * std::f32::consts::TAU;
                        pos2(r * a.cos(), r * a.sin())
                    })
                    .collect();
                vec![Poly { pts, closed: true }]
            }
            ShapeKind::Star { r1, r2, points } => {
                let n = (*points).max(3) * 2;
                let pts = (0..n)
                    .map(|k| {
                        let a = std::f32::consts::FRAC_PI_2 + k as f32 / n as f32 * std::f32::consts::TAU;
                        let r = if k % 2 == 0 { *r1 } else { *r2 };
                        pos2(r * a.cos(), r * a.sin())
                    })
                    .collect();
                vec![Poly { pts, closed: true }]
            }
            ShapeKind::Path { pts, closed } => vec![Poly { pts: pts.clone(), closed: *closed }],
            ShapeKind::Text { text, size } => text_outlines(text, *size),
        }
    }

    pub fn transform_point(&self, p: Pos2) -> Pos2 {
        let (s, c) = self.angle.to_radians().sin_cos();
        let x = p.x * self.scale.x;
        let y = p.y * self.scale.y;
        pos2(self.center.x + x * c - y * s, self.center.y + x * s + y * c)
    }

    /// Outlines in job coordinates.
    pub fn polys(&self) -> Vec<Poly> {
        self.local_polys()
            .into_iter()
            .map(|p| Poly { pts: p.pts.iter().map(|&q| self.transform_point(q)).collect(), closed: p.closed })
            .collect()
    }

    pub fn bounds(&self) -> Rect {
        bounds_of(&self.polys())
    }

    pub fn is_closed(&self) -> bool {
        self.polys().iter().all(|p| p.closed)
    }

    /// Distance from `p` to the shape (0 when inside a closed shape).
    pub fn hit_distance(&self, p: Pos2) -> f32 {
        let polys = self.polys();
        if polys.iter().any(|q| q.closed) && point_inside(p, &polys) {
            return 0.0;
        }
        dist_to_polys(p, &polys)
    }
}

pub fn bounds_of(polys: &[Poly]) -> Rect {
    let mut r = Rect::NOTHING;
    for p in polys {
        for q in &p.pts {
            r.extend_with(*q);
        }
    }
    r
}

pub fn segments(poly: &Poly) -> impl Iterator<Item = (Pos2, Pos2)> + '_ {
    let n = poly.pts.len();
    let count = if poly.closed { n } else { n.saturating_sub(1) };
    (0..count).map(move |i| (poly.pts[i], poly.pts[(i + 1) % n]))
}

pub fn seg_dist(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let l2 = ab.length_sq();
    let t = if l2 > 0.0 { ((p - a).dot(ab) / l2).clamp(0.0, 1.0) } else { 0.0 };
    (a + ab * t - p).length()
}

pub fn closest_on_poly(p: Pos2, poly: &Poly) -> Pos2 {
    let mut best = poly.pts.first().copied().unwrap_or(p);
    let mut bd = f32::MAX;
    for (a, b) in segments(poly) {
        let ab = b - a;
        let l2 = ab.length_sq();
        let t = if l2 > 0.0 { ((p - a).dot(ab) / l2).clamp(0.0, 1.0) } else { 0.0 };
        let q = a + ab * t;
        let d = (q - p).length_sq();
        if d < bd {
            bd = d;
            best = q;
        }
    }
    best
}

pub fn dist_to_polys(p: Pos2, polys: &[Poly]) -> f32 {
    let mut d = f32::MAX;
    for poly in polys {
        for (a, b) in segments(poly) {
            d = d.min(seg_dist(p, a, b));
        }
    }
    d
}

/// Even-odd inside test over all closed outlines (holes work naturally).
pub fn point_inside(p: Pos2, polys: &[Poly]) -> bool {
    let mut inside = false;
    for poly in polys.iter().filter(|q| q.closed) {
        let n = poly.pts.len();
        let mut j = n.wrapping_sub(1);
        for i in 0..n {
            let (a, b) = (poly.pts[i], poly.pts[j]);
            if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
                inside = !inside;
            }
            j = i;
        }
    }
    inside
}

pub fn poly_length(pts: &[Pos2]) -> f32 {
    pts.windows(2).map(|w| (w[1] - w[0]).length()).sum()
}

/// A signed distance field on a regular grid: positive inside closed shapes,
/// negative outside. For open-only input it is the plain distance (negative).
pub struct Field {
    pub origin: Pos2,
    pub cell: f32,
    pub nx: usize,
    pub ny: usize,
    pub d: Vec<f32>,
}

impl Field {
    pub fn build(polys: &[Poly], cell: f32, margin: f32) -> Field {
        let b = bounds_of(polys).expand(margin + cell * 2.0);
        let nx = ((b.width() / cell).ceil() as usize + 1).max(2);
        let ny = ((b.height() / cell).ceil() as usize + 1).max(2);
        Self::build_on(polys, b.min, cell, nx, ny, f32::MAX)
    }

    /// Build on an explicit grid. `max_d` caps the expensive distance search.
    pub fn build_on(polys: &[Poly], origin: Pos2, cell: f32, nx: usize, ny: usize, max_d: f32) -> Field {
        let mut d = vec![0.0f32; nx * ny];
        let segs: Vec<(Pos2, Pos2)> = polys.iter().flat_map(|p| segments(p).collect::<Vec<_>>()).collect();
        let closed: Vec<&Poly> = polys.iter().filter(|p| p.closed).collect();
        // Bucket segments by row band for a cheap distance cut-off.
        for j in 0..ny {
            let y = origin.y + j as f32 * cell;
            // even-odd crossings for this row
            let mut xs: Vec<f32> = Vec::new();
            for poly in &closed {
                for (a, b) in segments(poly) {
                    if (a.y > y) != (b.y > y) {
                        xs.push(a.x + (y - a.y) / (b.y - a.y) * (b.x - a.x));
                    }
                }
            }
            xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
            // segments near this row
            let near: Vec<(Pos2, Pos2)> = if max_d < f32::MAX {
                segs.iter().copied().filter(|(a, b)| a.y.min(b.y) - max_d <= y && a.y.max(b.y) + max_d >= y).collect()
            } else {
                Vec::new()
            };
            let list = if max_d < f32::MAX { &near } else { &segs };
            let mut k = 0;
            for i in 0..nx {
                let x = origin.x + i as f32 * cell;
                while k < xs.len() && xs[k] < x {
                    k += 1;
                }
                let inside = k % 2 == 1;
                let p = pos2(x, y);
                let mut m = max_d;
                for &(a, b) in list.iter() {
                    let dd = seg_dist(p, a, b);
                    if dd < m {
                        m = dd;
                    }
                }
                d[j * nx + i] = if inside { m } else { -m };
            }
        }
        Field { origin, cell, nx, ny, d }
    }

    #[inline]
    pub fn at(&self, i: usize, j: usize) -> f32 {
        self.d[j * self.nx + i]
    }

    pub fn pos(&self, i: usize, j: usize) -> Pos2 {
        pos2(self.origin.x + i as f32 * self.cell, self.origin.y + j as f32 * self.cell)
    }

    pub fn sample(&self, p: Pos2) -> f32 {
        let fx = ((p.x - self.origin.x) / self.cell).clamp(0.0, (self.nx - 1) as f32);
        let fy = ((p.y - self.origin.y) / self.cell).clamp(0.0, (self.ny - 1) as f32);
        let i = (fx as usize).min(self.nx - 2);
        let j = (fy as usize).min(self.ny - 2);
        let (tx, ty) = (fx - i as f32, fy - j as f32);
        let a = self.at(i, j) * (1.0 - tx) + self.at(i + 1, j) * tx;
        let b = self.at(i, j + 1) * (1.0 - tx) + self.at(i + 1, j + 1) * tx;
        a * (1.0 - ty) + b * ty
    }

    pub fn max(&self) -> f32 {
        self.d.iter().copied().fold(f32::MIN, f32::max)
    }

    /// Closed contour loops where the field equals `level` (marching squares).
    pub fn contours(&self, level: f32) -> Vec<Vec<Pos2>> {
        let nx = self.nx;
        let key_h = |i: usize, j: usize| ((j * nx + i) * 2) as u64;
        let key_v = |i: usize, j: usize| ((j * nx + i) * 2 + 1) as u64;
        let mut pts: HashMap<u64, Pos2> = HashMap::new();
        let mut adj: HashMap<u64, Vec<u64>> = HashMap::new();
        let interp = |a: Pos2, b: Pos2, va: f32, vb: f32| {
            let t = if (vb - va).abs() > 1e-9 { ((level - va) / (vb - va)).clamp(0.0, 1.0) } else { 0.5 };
            a + (b - a) * t
        };
        for j in 0..self.ny - 1 {
            for i in 0..nx - 1 {
                let v00 = self.at(i, j);
                let v10 = self.at(i + 1, j);
                let v11 = self.at(i + 1, j + 1);
                let v01 = self.at(i, j + 1);
                let c = ((v00 > level) as u8) | (((v10 > level) as u8) << 1) | (((v11 > level) as u8) << 2) | (((v01 > level) as u8) << 3);
                if c == 0 || c == 15 {
                    continue;
                }
                let p00 = self.pos(i, j);
                let p10 = self.pos(i + 1, j);
                let p11 = self.pos(i + 1, j + 1);
                let p01 = self.pos(i, j + 1);
                // edges: bottom(0) right(1) top(2) left(3)
                let ek = [key_h(i, j), key_v(i + 1, j), key_h(i, j + 1), key_v(i, j)];
                let ep = |e: usize| match e {
                    0 => interp(p00, p10, v00, v10),
                    1 => interp(p10, p11, v10, v11),
                    2 => interp(p01, p11, v01, v11),
                    _ => interp(p00, p01, v00, v01),
                };
                let center = (v00 + v10 + v11 + v01) * 0.25 > level;
                let pairs: &[(usize, usize)] = match c {
                    1 | 14 => &[(3, 0)],
                    2 | 13 => &[(0, 1)],
                    3 | 12 => &[(3, 1)],
                    4 | 11 => &[(1, 2)],
                    6 | 9 => &[(0, 2)],
                    7 | 8 => &[(3, 2)],
                    5 => {
                        if center {
                            &[(3, 2), (0, 1)]
                        } else {
                            &[(3, 0), (1, 2)]
                        }
                    }
                    10 => {
                        if center {
                            &[(3, 0), (1, 2)]
                        } else {
                            &[(3, 2), (0, 1)]
                        }
                    }
                    _ => &[],
                };
                for &(a, b) in pairs {
                    let (ka, kb) = (ek[a], ek[b]);
                    pts.entry(ka).or_insert_with(|| ep(a));
                    pts.entry(kb).or_insert_with(|| ep(b));
                    adj.entry(ka).or_default().push(kb);
                    adj.entry(kb).or_default().push(ka);
                }
            }
        }
        let mut visited: std::collections::HashSet<u64> = std::collections::HashSet::new();
        let mut loops = Vec::new();
        let mut keys: Vec<u64> = adj.keys().copied().collect();
        keys.sort_unstable();
        for start in keys {
            if visited.contains(&start) {
                continue;
            }
            let mut chain = vec![pts[&start]];
            visited.insert(start);
            let mut cur = start;
            loop {
                let next = adj[&cur].iter().copied().find(|k| !visited.contains(k));
                match next {
                    Some(n) => {
                        visited.insert(n);
                        chain.push(pts[&n]);
                        cur = n;
                    }
                    None => break,
                }
            }
            if chain.len() >= 3 {
                loops.push(simplify(&chain, self.cell * 0.05));
            }
        }
        loops
    }
}

// ---------------------------------------------------------------- vector editing

impl Vector {
    /// Bake the shape into an editable path (one outline only) with no rotation or scale.
    pub fn to_path(&mut self) -> bool {
        let polys = self.polys();
        if polys.len() != 1 {
            return false;
        }
        let poly = &polys[0];
        let c = self.center;
        self.kind = ShapeKind::Path { pts: poly.pts.iter().map(|p| pos2(p.x - c.x, p.y - c.y)).collect(), closed: poly.closed };
        self.angle = 0.0;
        self.scale = vec2(1.0, 1.0);
        true
    }
}

/// Points of the circular arc from `a` through `m` to `b` (a straight line when they are in a row).
pub fn arc_points(a: Pos2, m: Pos2, b: Pos2) -> Vec<Pos2> {
    let d = 2.0 * (a.x * (m.y - b.y) + m.x * (b.y - a.y) + b.x * (a.y - m.y));
    if d.abs() < 1e-6 {
        return vec![a, b];
    }
    let (a2, m2, b2) = (a.to_vec2().length_sq(), m.to_vec2().length_sq(), b.to_vec2().length_sq());
    let c = pos2(
        (a2 * (m.y - b.y) + m2 * (b.y - a.y) + b2 * (a.y - m.y)) / d,
        (a2 * (b.x - m.x) + m2 * (a.x - b.x) + b2 * (m.x - a.x)) / d,
    );
    let r = (a - c).length();
    let ang = |p: Pos2| (p.y - c.y).atan2(p.x - c.x);
    let tau = std::f32::consts::TAU;
    let (a0, am, ab) = (ang(a), ang(m), ang(b));
    let ccw = |x: f32| (x - a0).rem_euclid(tau);
    // sweep counter-clockwise if m lies between a and b going that way, else clockwise
    let sweep = if ccw(am) < ccw(ab) { ccw(ab) } else { ccw(ab) - tau };
    let n = ((sweep.abs() * r / 0.5).ceil() as usize).clamp(8, 720);
    (0..=n).map(|k| {
        let t = a0 + sweep * k as f32 / n as f32;
        pos2(c.x + r * t.cos(), c.y + r * t.sin())
    })
    .collect()
}

/// Union of closed shapes; each group is one shape (its own holes kept).
pub fn weld(groups: &[Vec<Poly>]) -> Vec<Poly> {
    let all: Vec<Poly> = groups.iter().flatten().cloned().collect();
    let b = bounds_of(&all);
    let cell = (b.width().max(b.height()) / 1500.0).clamp(0.02, 0.5);
    let b = b.expand(cell * 3.0);
    let nx = (b.width() / cell).ceil() as usize + 1;
    let ny = (b.height() / cell).ceil() as usize + 1;
    let mut field: Option<Field> = None;
    for g in groups {
        let f = Field::build_on(g, b.min, cell, nx, ny, cell * 4.0);
        match &mut field {
            None => field = Some(f),
            Some(acc) => acc.d.iter_mut().zip(f.d).for_each(|(a, v)| *a = a.max(v)),
        }
    }
    let Some(field) = field else { return Vec::new() };
    field.contours(0.0).into_iter().map(|pts| Poly { pts: simplify(&pts, cell * 0.3), closed: true }).collect()
}

fn seg_cross(a: Pos2, b: Pos2, c: Pos2, d: Pos2) -> Option<f32> {
    let r = b - a;
    let s = d - c;
    let den = r.x * s.y - r.y * s.x;
    if den.abs() < 1e-9 {
        return None;
    }
    let q = c - a;
    let t = (q.x * s.y - q.y * s.x) / den;
    let u = (q.x * r.y - q.y * r.x) / den;
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then_some(t)
}

/// Distance along a polyline (from its first point) of the point nearest `p`.
fn param_of(pts: &[Pos2], closed: bool, p: Pos2) -> f32 {
    let poly = Poly { pts: pts.to_vec(), closed };
    let (mut acc, mut best, mut bd) = (0.0, 0.0, f32::MAX);
    for (a, b) in segments(&poly) {
        let ab = b - a;
        let l = ab.length();
        let t = if l > 0.0 { ((p - a).dot(ab) / (l * l)).clamp(0.0, 1.0) } else { 0.0 };
        let d = (a + ab * t - p).length();
        if d < bd {
            bd = d;
            best = acc + t * l;
        }
        acc += l;
    }
    best
}

/// The part of a polyline between two distances along it (s0 < s1).
fn sub_path(pts: &[Pos2], closed: bool, s0: f32, s1: f32) -> Vec<Pos2> {
    let poly = Poly { pts: pts.to_vec(), closed };
    let mut out = Vec::new();
    let mut acc = 0.0;
    for (a, b) in segments(&poly) {
        let l = (b - a).length();
        let (e0, e1) = (acc, acc + l);
        if e1 >= s0 && e0 <= s1 && l > 0.0 {
            if out.is_empty() {
                out.push(a + (b - a) * ((s0 - e0) / l).clamp(0.0, 1.0));
            }
            if e1 <= s1 {
                out.push(b);
            } else {
                out.push(a + (b - a) * ((s1 - e0) / l));
                break;
            }
        }
        acc = e1;
    }
    out
}

/// Remove the piece of `poly` around `click` that lies between its nearest crossings with
/// `others`. Returns the pieces that remain, or None when nothing crosses it.
pub fn trim(poly: &Poly, others: &[Poly], click: Pos2) -> Option<Vec<Poly>> {
    let total: f32 = segments(poly).map(|(a, b)| (b - a).length()).sum();
    let mut cuts = Vec::new();
    let mut acc = 0.0;
    for (a, b) in segments(poly) {
        let l = (b - a).length();
        for o in others {
            for (c, d) in segments(o) {
                if let Some(t) = seg_cross(a, b, c, d) {
                    cuts.push(acc + t * l);
                }
            }
        }
        acc += l;
    }
    cuts.sort_by(|a, b| a.partial_cmp(b).unwrap());
    cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-3);
    if cuts.is_empty() {
        return None;
    }
    let s = param_of(&poly.pts, poly.closed, click);
    let before = cuts.iter().copied().filter(|c| *c < s).last();
    let after = cuts.iter().copied().find(|c| *c > s);
    let piece = |a: f32, b: f32| Poly { pts: sub_path(&poly.pts, poly.closed, a, b), closed: false };
    let mut out = Vec::new();
    if poly.closed {
        if cuts.len() < 2 {
            return None;
        }
        let (lo, hi) = (before.unwrap_or(*cuts.last().unwrap() - total), after.unwrap_or(cuts[0] + total));
        // keep the rest of the loop: from hi round to lo
        let mut doubled = poly.pts.clone();
        doubled.extend(poly.pts.iter().copied());
        doubled.push(poly.pts[0]);
        let (a, b) = if hi >= total { (hi - total, lo + total - total) } else { (hi, lo + total) };
        let b = if b <= a { b + total } else { b };
        out.push(Poly { pts: sub_path(&doubled, false, a, b), closed: false });
    } else {
        if let Some(b) = before {
            out.push(piece(0.0, b));
        }
        if let Some(a) = after {
            out.push(piece(a, total));
        }
    }
    out.retain(|p| p.pts.len() >= 2 && poly_length(&p.pts) > 1e-3);
    Some(out)
}

/// Round the corner at vertex `i` with radius `r`. Returns false if it is not a corner.
pub fn fillet_corner(pts: &mut Vec<Pos2>, closed: bool, i: usize, r: f32) -> bool {
    let n = pts.len();
    if n < 3 || (!closed && (i == 0 || i == n - 1)) {
        return false;
    }
    let p = pts[i];
    let (a, b) = (pts[(i + n - 1) % n], pts[(i + 1) % n]);
    let (u, v) = ((a - p).normalized(), (b - p).normalized());
    let cos = u.dot(v).clamp(-1.0, 1.0);
    let theta = cos.acos();
    if theta > std::f32::consts::PI - 0.02 || theta < 0.01 {
        return false;
    }
    let t = (r / (theta * 0.5).tan()).min((a - p).length() * 0.5).min((b - p).length() * 0.5);
    let r = t * (theta * 0.5).tan();
    let (s, e) = (p + u * t, p + v * t);
    let bis = (u + v).normalized();
    let mid = p + bis * (t / (theta * 0.5).cos() - r);
    let arc = arc_points(s, mid, e);
    pts.splice(i..=i, arc);
    true
}

/// Round every corner of a path sharper than about 170°.
pub fn fillet_all(pts: &[Pos2], closed: bool, r: f32) -> Vec<Pos2> {
    let mut out = pts.to_vec();
    let mut i = if closed { 0 } else { 1 };
    while i < out.len() {
        let before = out.len();
        let n = out.len();
        let (a, p, b) = (out[(i + n - 1) % n], out[i], out[(i + 1) % n]);
        let sharp = (a - p).normalized().dot((b - p).normalized()) > -0.985;
        if sharp && fillet_corner(&mut out, closed, i, r) {
            i += out.len() - before + 1;
        } else {
            i += 1;
        }
        if !closed && i >= out.len() - 1 {
            break;
        }
    }
    out
}

/// Ramer–Douglas–Peucker simplification.
pub fn simplify(pts: &[Pos2], tol: f32) -> Vec<Pos2> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    let mut keep = vec![false; pts.len()];
    keep[0] = true;
    keep[pts.len() - 1] = true;
    let mut stack = vec![(0usize, pts.len() - 1)];
    while let Some((a, b)) = stack.pop() {
        let mut best = 0.0;
        let mut idx = 0;
        for i in a + 1..b {
            let d = seg_dist(pts[i], pts[a], pts[b]);
            if d > best {
                best = d;
                idx = i;
            }
        }
        if best > tol {
            keep[idx] = true;
            stack.push((a, idx));
            stack.push((idx, b));
        }
    }
    pts.iter().zip(keep).filter(|(_, k)| *k).map(|(p, _)| *p).collect()
}

/// Offset closed outlines: positive = outward, negative = inward.
pub fn offset_polys(polys: &[Poly], offset: f32) -> Vec<Poly> {
    let b = bounds_of(polys);
    let cell = ((b.width().max(b.height()) + 2.0 * offset.abs()) / 500.0).clamp(0.05, 1.0);
    let field = Field::build(polys, cell, offset.abs() + cell * 3.0);
    field.contours(-offset).into_iter().map(|pts| Poly { pts, closed: true }).collect()
}

// ---------------------------------------------------------------- text

fn font() -> &'static ab_glyph::FontRef<'static> {
    static FONT: OnceLock<ab_glyph::FontRef<'static>> = OnceLock::new();
    FONT.get_or_init(|| {
        let defs = eframe::egui::FontDefinitions::default();
        let data = defs
            .font_data
            .get("Ubuntu-Light")
            .or_else(|| defs.font_data.values().next())
            .expect("a default font");
        let bytes: &'static [u8] = Box::leak(data.font.to_vec().into_boxed_slice());
        ab_glyph::FontRef::try_from_slice(bytes).expect("font parses")
    })
}

/// Glyph outlines for a line of text, centred on the origin, cap height ≈ size.
pub fn text_outlines(text: &str, size: f32) -> Vec<Poly> {
    use ab_glyph::{Font, OutlineCurve};
    let f = font();
    // Scale so capital letters are about `size` tall.
    let cap = f
        .outline(f.glyph_id('H'))
        .map(|o| {
            let ys = o.curves.iter().flat_map(|c| match c {
                OutlineCurve::Line(a, b) => vec![a.y, b.y],
                OutlineCurve::Quad(a, b, c) => vec![a.y, b.y, c.y],
                OutlineCurve::Cubic(a, b, c, d) => vec![a.y, b.y, c.y, d.y],
            });
            let (lo, hi) = ys.fold((f32::MAX, f32::MIN), |(lo, hi), y| (lo.min(y), hi.max(y)));
            hi - lo
        })
        .filter(|h| *h > 0.0)
        .unwrap_or(f.ascent_unscaled() * 0.7);
    let s = size / cap.max(1e-3);
    let mut polys = Vec::new();
    let mut x = 0.0f32;
    let mut prev: Option<ab_glyph::GlyphId> = None;
    for ch in text.chars() {
        let gid = f.glyph_id(ch);
        if let Some(p) = prev {
            x += f.kern_unscaled(p, gid);
        }
        if let Some(outline) = f.outline(gid) {
            let mut cur: Vec<Pos2> = Vec::new();
            let mut last: Option<ab_glyph::Point> = None;
            let tp = |p: ab_glyph::Point| pos2((x + p.x) * s, p.y * s);
            for curve in &outline.curves {
                let (start, end) = match curve {
                    OutlineCurve::Line(a, b) => (*a, *b),
                    OutlineCurve::Quad(a, _, c) => (*a, *c),
                    OutlineCurve::Cubic(a, _, _, d) => (*a, *d),
                };
                let continues = last.map(|l| (l.x - start.x).abs() < 1e-3 && (l.y - start.y).abs() < 1e-3).unwrap_or(false);
                if !continues {
                    if cur.len() > 2 {
                        polys.push(Poly { pts: std::mem::take(&mut cur), closed: true });
                    }
                    cur.clear();
                    cur.push(tp(start));
                }
                match curve {
                    OutlineCurve::Line(_, b) => cur.push(tp(*b)),
                    OutlineCurve::Quad(a, b, c) => {
                        for k in 1..=6 {
                            let t = k as f32 / 6.0;
                            let u = 1.0 - t;
                            let px = u * u * a.x + 2.0 * u * t * b.x + t * t * c.x;
                            let py = u * u * a.y + 2.0 * u * t * b.y + t * t * c.y;
                            cur.push(tp(ab_glyph::point(px, py)));
                        }
                    }
                    OutlineCurve::Cubic(a, b, c, d) => {
                        for k in 1..=8 {
                            let t = k as f32 / 8.0;
                            let u = 1.0 - t;
                            let px = u * u * u * a.x + 3.0 * u * u * t * b.x + 3.0 * u * t * t * c.x + t * t * t * d.x;
                            let py = u * u * u * a.y + 3.0 * u * u * t * b.y + 3.0 * u * t * t * c.y + t * t * t * d.y;
                            cur.push(tp(ab_glyph::point(px, py)));
                        }
                    }
                }
                last = Some(end);
            }
            if cur.len() > 2 {
                // drop duplicated closing point
                if (cur[0] - *cur.last().unwrap()).length() < 1e-4 {
                    cur.pop();
                }
                polys.push(Poly { pts: cur, closed: true });
            }
        }
        x += f.h_advance_unscaled(gid);
        prev = Some(gid);
    }
    // centre
    let b = bounds_of(&polys);
    if b.is_positive() {
        let c = b.center();
        let off = vec2(-c.x, -size * 0.5);
        for p in &mut polys {
            for q in &mut p.pts {
                *q += off;
            }
        }
    }
    polys
}

#[cfg(test)]
mod tests {
    #[test]
    fn text_has_outlines() {
        let defs = eframe::egui::FontDefinitions::default();
        eprintln!("fonts: {:?}", defs.font_data.keys().collect::<Vec<_>>());
        let p = super::text_outlines("WELCOME", 18.0);
        eprintln!("polys {} bounds {:?}", p.len(), super::bounds_of(&p));
        assert!(!p.is_empty());
    }
}

#[cfg(test)]
mod edit_tests {
    use super::*;

    fn square(c: Pos2, h: f32) -> Poly {
        Poly { pts: vec![pos2(c.x - h, c.y - h), pos2(c.x + h, c.y - h), pos2(c.x + h, c.y + h), pos2(c.x - h, c.y + h)], closed: true }
    }

    #[test]
    fn arc_through_three_points() {
        let pts = arc_points(pos2(-10.0, 0.0), pos2(0.0, 10.0), pos2(10.0, 0.0));
        assert!(pts.iter().all(|p| (p.to_vec2().length() - 10.0).abs() < 1e-3));
        assert!(pts.iter().all(|p| p.y >= -1e-3), "arc should pass over the top");
    }

    #[test]
    fn weld_two_overlapping_squares() {
        let out = weld(&[vec![square(pos2(0.0, 0.0), 10.0)], vec![square(pos2(10.0, 0.0), 10.0)]]);
        assert_eq!(out.len(), 1);
        let b = bounds_of(&out);
        assert!((b.width() - 30.0).abs() < 0.3 && (b.height() - 20.0).abs() < 0.3, "{b:?}");
    }

    #[test]
    fn trim_open_line_between_crossings() {
        let line = Poly { pts: vec![pos2(0.0, 0.0), pos2(30.0, 0.0)], closed: false };
        let cutters = [Poly { pts: vec![pos2(10.0, -5.0), pos2(10.0, 5.0)], closed: false }, Poly { pts: vec![pos2(20.0, -5.0), pos2(20.0, 5.0)], closed: false }];
        let out = trim(&line, &cutters, pos2(15.0, 0.0)).unwrap();
        assert_eq!(out.len(), 2);
        assert!((poly_length(&out[0].pts) - 10.0).abs() < 1e-3 && (poly_length(&out[1].pts) - 10.0).abs() < 1e-3);
    }

    #[test]
    fn trim_closed_square_keeps_the_rest() {
        let sq = square(pos2(0.0, 0.0), 10.0);
        let cutter = [Poly { pts: vec![pos2(5.0, -20.0), pos2(5.0, 20.0)], closed: false }];
        let out = trim(&sq, &cutter, pos2(10.0, 0.0)).unwrap();
        assert_eq!(out.len(), 1);
        assert!((poly_length(&out[0].pts) - 50.0).abs() < 1e-2, "{}", poly_length(&out[0].pts));
    }

    #[test]
    fn fillet_rounds_square_corner() {
        let mut pts = square(pos2(0.0, 0.0), 10.0).pts;
        assert!(fillet_corner(&mut pts, true, 2, 4.0));
        assert!(pts.iter().all(|p| p.x <= 10.0 + 1e-3 && p.y <= 10.0 + 1e-3));
        let far = pts.iter().filter(|p| p.x > 0.0 && p.y > 0.0).map(|p| p.to_vec2().length()).fold(0.0, f32::max);
        assert!(far < 10.0 * 2f32.sqrt() - 1.0, "corner should be cut");
        let all = fillet_all(&square(pos2(0.0, 0.0), 10.0).pts, true, 3.0);
        assert!(all.len() > 20);
    }
}
