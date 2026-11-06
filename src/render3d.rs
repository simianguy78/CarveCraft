//! A small software 3D view for height fields (orbit camera, painter's sort).

use eframe::egui::{self, pos2, vec2, Color32, ColorImage, Mesh, Painter, Pos2, Rect, Shape, Stroke, TextureId, Vec2};

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub yaw: f32,
    pub pitch: f32,
    pub zoom: f32,
    pub pan: Vec2,
    pub exaggerate: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self { yaw: -25.0, pitch: 48.0, zoom: 1.0, pan: Vec2::ZERO, exaggerate: 1.0 }
    }
}

pub struct Projector {
    cx: f32,
    cy: f32,
    cyaw: f32,
    syaw: f32,
    cp: f32,
    sp: f32,
    scale: f32,
    origin: Pos2,
    exag: f32,
}

impl Projector {
    pub fn new(cam: &Camera, rect: Rect, w: f32, h: f32) -> Projector {
        let scale = rect.width().min(rect.height()) / (w.max(h) * 1.2) * cam.zoom;
        Projector {
            cx: w * 0.5,
            cy: h * 0.5,
            cyaw: cam.yaw.to_radians().cos(),
            syaw: cam.yaw.to_radians().sin(),
            cp: cam.pitch.to_radians().cos(),
            sp: cam.pitch.to_radians().sin(),
            scale,
            origin: rect.center() + cam.pan + vec2(0.0, rect.height() * 0.04),
            exag: cam.exaggerate,
        }
    }

    /// Returns screen position and depth (larger = closer to the viewer).
    #[inline]
    pub fn project(&self, x: f32, y: f32, z: f32) -> (Pos2, f32) {
        let (x0, y0, z0) = (x - self.cx, y - self.cy, z * self.exag);
        let x1 = x0 * self.cyaw - y0 * self.syaw;
        let y1 = x0 * self.syaw + y0 * self.cyaw;
        let up = y1 * self.sp + z0 * self.cp;
        let depth = -y1 * self.cp + z0 * self.sp;
        (pos2(self.origin.x + x1 * self.scale, self.origin.y - up * self.scale), depth)
    }

    /// Depth direction of a horizontal normal (positive = faces viewer).
    fn faces(&self, nx: f32, ny: f32) -> f32 {
        let y1 = nx * self.syaw + ny * self.cyaw;
        -y1 * self.cp
    }
}

/// Hill-shaded colour image (row 0 = top edge, max y) of a height field.
pub fn shade(heights: &[f32], nx: usize, ny: usize, cell: f32, color: impl Fn(f32) -> [f32; 3]) -> ColorImage {
    let mut img = ColorImage::new([nx, ny], Color32::BLACK);
    let l = {
        let v = [-0.45f32, 0.55, 0.70];
        let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        [v[0] / n, v[1] / n, v[2] / n]
    };
    let at = |i: usize, j: usize| heights[j * nx + i];
    for j in 0..ny {
        for i in 0..nx {
            let h = at(i, j);
            let dzdx = (at((i + 1).min(nx - 1), j) - at(i.saturating_sub(1), j)) / (2.0 * cell);
            let dzdy = (at(i, (j + 1).min(ny - 1)) - at(i, j.saturating_sub(1))) / (2.0 * cell);
            let n = {
                let v = [-dzdx, -dzdy, 1.0];
                let m = (v[0] * v[0] + v[1] * v[1] + 1.0).sqrt();
                [v[0] / m, v[1] / m, v[2] / m]
            };
            let diff = (n[0] * l[0] + n[1] * l[1] + n[2] * l[2]).max(0.0);
            // half-vector specular toward a viewer above
            let hv = {
                let v = [l[0], l[1], l[2] + 1.0];
                let m = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                [v[0] / m, v[1] / m, v[2] / m]
            };
            let spec = (n[0] * hv[0] + n[1] * hv[1] + n[2] * hv[2]).max(0.0).powf(40.0) * 0.25;
            let c = color(h);
            let k = 0.38 + 0.68 * diff;
            let px = [(c[0] * k + spec).min(1.0), (c[1] * k + spec).min(1.0), (c[2] * k + spec).min(1.0)];
            img.pixels[(ny - 1 - j) * nx + i] = Color32::from_rgb((px[0] * 255.0) as u8, (px[1] * 255.0) as u8, (px[2] * 255.0) as u8);
        }
    }
    img
}

pub struct Surface<'a> {
    pub heights: &'a [f32],
    pub nx: usize,
    pub ny: usize,
    pub cell: f32,
    pub texture: TextureId,
    pub side: Color32,
    pub bottom: f32,
}

/// Draw a height field with sides down to `bottom`.
pub fn draw_surface(painter: &Painter, proj: &Projector, s: &Surface) {
    let (nx, ny) = (s.nx, s.ny);
    if nx < 2 || ny < 2 || s.heights.len() != nx * ny {
        return;
    }
    let step = ((nx.max(ny) as f32) / 330.0).ceil().max(1.0) as usize;
    let xs: Vec<usize> = (0..nx).step_by(step).chain(std::iter::once(nx - 1)).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
    let ys: Vec<usize> = (0..ny).step_by(step).chain(std::iter::once(ny - 1)).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
    let (vw, vh) = (xs.len(), ys.len());
    let mut mesh = Mesh::with_texture(s.texture);
    let mut depth = Vec::with_capacity(vw * vh);
    let mut zs = Vec::with_capacity(vw * vh);
    let wx = |i: usize| (i as f32 + 0.5) * s.cell;
    for &j in &ys {
        for &i in &xs {
            let z = s.heights[j * nx + i];
            let (p, d) = proj.project(wx(i), wx(j), z);
            depth.push(d);
            zs.push(z);
            mesh.vertices.push(egui::epaint::Vertex {
                pos: p,
                uv: pos2((i as f32 + 0.5) / nx as f32, 1.0 - (j as f32 + 0.5) / ny as f32),
                color: Color32::WHITE,
            });
        }
    }
    let mut tris: Vec<(f32, [u32; 3])> = Vec::with_capacity((vw - 1) * (vh - 1) * 2);
    for b in 0..vh - 1 {
        for a in 0..vw - 1 {
            let i0 = (b * vw + a) as u32;
            let i1 = i0 + 1;
            let i2 = i0 + vw as u32;
            let i3 = i2 + 1;
            // Split along the diagonal whose ends are closest in height, so steep
            // walls (cut edges, groove sides) don't turn into long spiky triangles.
            let z = |k: u32| zs[k as usize];
            let d = |a: u32, b: u32, c: u32| depth[a as usize] + depth[b as usize] + depth[c as usize];
            let pair = if (z(i0) - z(i3)).abs() <= (z(i1) - z(i2)).abs() { [[i0, i1, i3], [i0, i3, i2]] } else { [[i0, i1, i2], [i1, i3, i2]] };
            for t in pair {
                tris.push((d(t[0], t[1], t[2]), t));
            }
        }
    }
    tris.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    mesh.indices.reserve(tris.len() * 3);
    for (_, t) in tris {
        mesh.indices.extend_from_slice(&t);
    }
    painter.add(Shape::mesh(mesh));

    // Sides facing the viewer.
    let mut walls = Mesh::default();
    let edges: [(Vec<(usize, usize)>, (f32, f32)); 4] = [
        (xs.iter().map(|&i| (i, 0)).collect(), (0.0, -1.0)),
        (xs.iter().map(|&i| (i, ny - 1)).collect(), (0.0, 1.0)),
        (ys.iter().map(|&j| (0, j)).collect(), (-1.0, 0.0)),
        (ys.iter().map(|&j| (nx - 1, j)).collect(), (1.0, 0.0)),
    ];
    for (pts, (nxn, nyn)) in edges.iter() {
        let f = proj.faces(*nxn, *nyn);
        if f <= 0.0 {
            continue;
        }
        let k = 0.55 + 0.35 * (nxn * -0.45 + nyn * 0.55 + 0.6).clamp(0.0, 1.0);
        let col = Color32::from_rgb((s.side.r() as f32 * k) as u8, (s.side.g() as f32 * k) as u8, (s.side.b() as f32 * k) as u8);
        for w in pts.windows(2) {
            let (i0, j0) = w[0];
            let (i1, j1) = w[1];
            let (x0, x1, y0, y1) = (wx(i0), wx(i1), wx(j0), wx(j1));
            let (a, _) = proj.project(x0, y0, s.heights[j0 * nx + i0]);
            let (b, _) = proj.project(x1, y1, s.heights[j1 * nx + i1]);
            let (c, _) = proj.project(x1, y1, s.bottom);
            let (d, _) = proj.project(x0, y0, s.bottom);
            let base = walls.vertices.len() as u32;
            for p in [a, b, c, d] {
                walls.colored_vertex(p, col);
            }
            walls.add_triangle(base, base + 1, base + 2);
            walls.add_triangle(base, base + 2, base + 3);
        }
    }
    painter.add(Shape::mesh(walls));
}

/// Plain box for the material outline when nothing else is shown.
pub fn draw_box_edges(painter: &Painter, proj: &Projector, w: f32, h: f32, bottom: f32, stroke: Stroke) {
    let c = [(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)];
    for k in 0..4 {
        let (a, b) = (c[k], c[(k + 1) % 4]);
        let (pa, _) = proj.project(a.0, a.1, 0.0);
        let (pb, _) = proj.project(b.0, b.1, 0.0);
        painter.line_segment([pa, pb], stroke);
        let (qa, _) = proj.project(a.0, a.1, bottom);
        let (qb, _) = proj.project(b.0, b.1, bottom);
        painter.line_segment([qa, qb], Stroke::new(stroke.width * 0.6, stroke.color.gamma_multiply(0.5)));
        painter.line_segment([pa, qa], Stroke::new(stroke.width * 0.6, stroke.color.gamma_multiply(0.5)));
    }
}

/// Toolpath chains as 3D polylines.
pub fn draw_chains(painter: &Painter, proj: &Projector, chains: &[Vec<[f32; 3]>], color: Color32, max_points: usize) {
    let total: usize = chains.iter().map(|c| c.len()).sum();
    let stride = (total / max_points.max(1)).max(1);
    let stroke = Stroke::new(1.0_f32, color);
    let mut prev_end: Option<Pos2> = None;
    for ch in chains {
        if ch.is_empty() {
            continue;
        }
        let mut pts: Vec<Pos2> = ch.iter().step_by(stride).map(|p| proj.project(p[0], p[1], p[2]).0).collect();
        if let Some(l) = ch.last() {
            pts.push(proj.project(l[0], l[1], l[2]).0);
        }
        if let (Some(pe), Some(first)) = (prev_end, pts.first()) {
            painter.line_segment([pe, *first], Stroke::new(0.5_f32, Color32::from_rgba_unmultiplied(255, 80, 80, 60)));
        }
        prev_end = pts.last().copied();
        painter.add(Shape::line(pts, stroke));
    }
}
