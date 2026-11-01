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
