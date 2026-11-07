//! The 2D design canvas and the 3D view.

use crate::app::{App, Drag, DrawTool, Handle};
use crate::geom::{self, ShapeKind, Vector};
use crate::render3d::{self, Projector, Surface};
use crate::theme::{self, ACCENT};
use crate::toolpath::ToolKind;
use egui::{pos2, vec2, Align2, Color32, CornerRadius, FontId, Mesh, PointerButton, Pos2, Rect, Sense, Shape, Stroke, Ui};

fn shift_only(resp: &egui::Response) -> bool {
    resp.ctx.input(|i| i.modifiers.shift)
}

/// Inside angle at `v` between the segments to `a` and to `b`, in degrees (0–180).
fn corner_angle(a: Pos2, v: Pos2, b: Pos2) -> Option<f32> {
    let (u, w) = (a - v, b - v);
    if u.length() < 1e-6 || w.length() < 1e-6 {
        return None;
    }
    Some((u.dot(w) / (u.length() * w.length())).clamp(-1.0, 1.0).acos().to_degrees())
}

/// Scale a shape by `sx`/`sy` along the job axes about `anchor`.
fn scale_vector(v0: &Vector, anchor: Pos2, sx: f32, sy: f32) -> Vector {
    let mut v = v0.clone();
    v.center = pos2(anchor.x + (v0.center.x - anchor.x) * sx, anchor.y + (v0.center.y - anchor.y) * sy);
    let a = v0.angle.rem_euclid(180.0);
    let near = |t: f32| (a - t).abs() < 0.01 || (a - t - 180.0).abs() < 0.01;
    if (sx - sy).abs() < 1e-5 || near(0.0) {
        v.scale.x *= sx;
        v.scale.y *= sy;
    } else if near(90.0) {
        // Turned a quarter: the shape's own axes are swapped relative to the job.
        v.scale.x *= sy;
        v.scale.y *= sx;
    } else {
        // Stretching a turned shape skews it, which only a free curve can hold.
        let polys = v0.polys();
        if polys.len() == 1 {
            let pts: Vec<Pos2> = polys[0].pts.iter().map(|p| pos2(anchor.x + (p.x - anchor.x) * sx, anchor.y + (p.y - anchor.y) * sy)).collect();
            let c = geom::bounds_of(&[geom::Poly { pts: pts.clone(), closed: polys[0].closed }]).center();
            v.kind = ShapeKind::Path { pts: pts.iter().map(|p| pos2(p.x - c.x, p.y - c.y)).collect(), closed: polys[0].closed };
            v.center = c;
            v.angle = 0.0;
            v.scale = vec2(1.0, 1.0);
        } else {
            // Text and other multi-outline shapes keep their form: scale evenly.
            let s = (sx * sy).sqrt();
            v.center = pos2(anchor.x + (v0.center.x - anchor.x) * s, anchor.y + (v0.center.y - anchor.y) * s);
            v.scale *= s;
        }
    }
    v
}

fn path_color(kind: ToolKind, i: usize) -> Color32 {
    let base = match kind {
        ToolKind::EndMill => Color32::from_rgb(255, 107, 107),
        ToolKind::BallNose => Color32::from_rgb(80, 200, 160),
        ToolKind::VBit => Color32::from_rgb(255, 184, 77),
    };
    if i % 2 == 1 {
        base.gamma_multiply(0.85)
    } else {
        base
    }
}

impl App {
    fn hit_vector(&self, p: Pos2, tol: f32) -> Option<u64> {
        let layers = &self.doc.layers;
        let visible = |l: usize| layers.get(l).map(|x| x.visible).unwrap_or(true);
        let mut best: Option<(f32, u64)> = None;
        for v in self.doc.vectors.iter().rev().filter(|v| visible(v.layer)) {
            let d = geom::dist_to_polys(p, &v.polys());
            if d <= tol && best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, v.id));
            }
        }
        if let Some((_, id)) = best {
            return Some(id);
        }
        // otherwise the smallest closed shape containing the point
        let mut inside: Option<(f32, u64)> = None;
        for v in self.doc.vectors.iter().filter(|v| visible(v.layer)) {
            let polys = v.polys();
            if geom::point_inside(p, &polys) {
                let b = v.bounds();
                let area = b.width() * b.height();
                if inside.is_none_or(|(a, _)| area < a) {
                    inside = Some((area, v.id));
                }
            }
        }
        inside.map(|(_, id)| id)
    }

    pub fn view_2d(&mut self, ui: &mut Ui) {
        let pal = theme::palette(self.dark);
        let (rect, resp) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        let mat = self.doc.material.clone();
        if self.fit_2d {
            self.v2_center = pos2(mat.w * 0.5, mat.h * 0.5);
            self.v2_zoom = ((rect.width() - 90.0) / mat.w).min((rect.height() - 90.0) / mat.h).max(0.05);
            self.fit_2d = false;
        }
        // --- navigation
        let rc = rect.center();
        if resp.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                if let Some(s) = resp.hover_pos() {
                    let z = self.v2_zoom;
                    let c = self.v2_center;
                    let jp = pos2(c.x + (s.x - rc.x) / z, c.y - (s.y - rc.y) / z);
                    let nz = (z * (scroll * 0.0015).exp()).clamp(0.05, 200.0);
                    self.v2_center = jp + (c - jp) * (z / nz);
                    self.v2_zoom = nz;
                }
            }
        }
        if resp.dragged_by(PointerButton::Secondary) || resp.dragged_by(PointerButton::Middle) {
            let d = resp.drag_delta();
            self.v2_center.x -= d.x / self.v2_zoom;
            self.v2_center.y += d.y / self.v2_zoom;
        }
        let (c, z) = (self.v2_center, self.v2_zoom);
        let to_s = move |p: Pos2| pos2(rc.x + (p.x - c.x) * z, rc.y - (p.y - c.y) * z);
        let to_j = move |s: Pos2| pos2(c.x + (s.x - rc.x) / z, c.y - (s.y - rc.y) / z);
        let hover_j = resp.hover_pos().map(to_j);
        self.cursor = hover_j.or(self.cursor);
        let tol = 7.0 / z;

        // --- interaction
        self.handle_2d_input(&resp, to_j, tol);
        // Screen spots of the top-right rotate handle and the selection centre, for the
        // scripted drag used when checking this feature.
        self.probe = self.selection_handles().and_then(|(b, hs)| {
            hs.iter().filter(|(h, _)| *h == Handle::Rotate).max_by(|a, b| (a.1.x + a.1.y).total_cmp(&(b.1.x + b.1.y))).map(|(_, q)| (to_s(*q), to_s(b.center())))
        });

        // --- drawing
        let mr = Rect::from_two_pos(to_s(pos2(0.0, 0.0)), to_s(pos2(mat.w, mat.h)));
        painter.rect_filled(mr.translate(vec2(0.0, 6.0)).expand(2.0), CornerRadius::same(4), Color32::from_black_alpha(if self.dark { 90 } else { 30 }));
        painter.rect_filled(mr, CornerRadius::same(2), pal.sheet);
        let show_model = self.show_model2d && !self.doc.model.is_empty();
        if show_model {
            if let Some(t) = &self.model_tex {
                painter.image(t.id(), mr, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE.gamma_multiply(if self.dark { 0.92 } else { 1.0 }));
            }
        }
        if self.show_grid {
            let mut step = 1.0;
            for s in [1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0] {
                step = s;
                if s * z >= 14.0 {
                    break;
                }
            }
            let major = step * 5.0;
            let alpha = if show_model { 0.12 } else { 1.0 };
            let mut x = 0.0;
            while x <= mat.w + 1e-3 {
                let is_major = (x / major).round() * major - x == 0.0;
                let col = if is_major { pal.grid_major } else { pal.grid };
                painter.line_segment([to_s(pos2(x, 0.0)), to_s(pos2(x, mat.h))], Stroke::new(1.0_f32, col.gamma_multiply(alpha)));
                x += step;
            }
            let mut y = 0.0;
            while y <= mat.h + 1e-3 {
                let is_major = (y / major).round() * major - y == 0.0;
                let col = if is_major { pal.grid_major } else { pal.grid };
                painter.line_segment([to_s(pos2(0.0, y)), to_s(pos2(mat.w, y))], Stroke::new(1.0_f32, col.gamma_multiply(alpha)));
                y += step;
            }
        }
        painter.rect_stroke(mr, CornerRadius::same(2), Stroke::new(1.0_f32, pal.border), egui::StrokeKind::Outside);
        // origin marker
        let o = to_s(match mat.origin {
            crate::toolpath::Origin::BottomLeft => pos2(0.0, 0.0),
            crate::toolpath::Origin::Center => pos2(mat.w * 0.5, mat.h * 0.5),
        });
        painter.arrow(o, vec2(28.0, 0.0), Stroke::new(2.0_f32, Color32::from_rgb(235, 87, 87)));
        painter.arrow(o, vec2(0.0, -28.0), Stroke::new(2.0_f32, Color32::from_rgb(76, 200, 120)));

        // toolpaths
        if self.show_paths {
            for (i, t) in self.doc.toolpaths.iter().enumerate().filter(|(_, t)| t.visible) {
                let col = path_color(t.tool.kind, i);
                let total: usize = t.chains.iter().map(|c| c.len()).sum();
                let stride = (total / 60_000).max(1);
                for ch in t.chains.iter() {
                    let mut pts: Vec<Pos2> = ch.iter().step_by(stride).map(|p| to_s(pos2(p[0], p[1]))).collect();
                    if let Some(l) = ch.last() {
                        pts.push(to_s(pos2(l[0], l[1])));
                    }
                    if pts.len() == 2 && (pts[0] - pts[1]).length() < 0.5 {
                        painter.circle_stroke(pts[0], 4.0, Stroke::new(1.5_f32, col));
                    }
                    painter.add(Shape::line(pts, Stroke::new(1.0_f32, col.gamma_multiply(0.8))));
                }
            }
        }

        // vectors
        let layers = self.doc.layers.clone();
        for v in &self.doc.vectors {
            if !layers.get(v.layer).map(|l| l.visible).unwrap_or(true) {
                continue;
            }
            let selected = self.sel.contains(&v.id);
            let stroke = if selected {
                Stroke::new(2.0_f32, ACCENT)
            } else if show_model {
                Stroke::new(1.2_f32, if self.dark { Color32::from_rgb(40, 30, 20) } else { Color32::from_rgb(60, 45, 30) })
            } else {
                Stroke::new(1.3_f32, pal.vector)
            };
            for poly in v.polys() {
                let pts: Vec<Pos2> = poly.pts.iter().map(|p| to_s(*p)).collect();
                if poly.closed {
                    painter.add(Shape::closed_line(pts, stroke));
                } else {
                    painter.add(Shape::line(pts, stroke));
                }
            }
        }
        if let Some((b, handles)) = self.selection_handles() {
            let sr = Rect::from_two_pos(to_s(b.min), to_s(b.max)).expand(5.0);
            let dash = Shape::dashed_line(&[sr.left_top(), sr.right_top(), sr.right_bottom(), sr.left_bottom(), sr.left_top()], Stroke::new(1.0_f32, ACCENT), 5.0, 4.0);
            painter.extend(dash);
            let hover = if self.tool == DrawTool::Select { hover_j.and_then(|p| self.handle_at(p)) } else { None };
            let active = match &self.drag {
                Drag::Scale { hx, hy, .. } => Some(crate::app::Handle::Scale(*hx, *hy)),
                Drag::Rotate { .. } => Some(crate::app::Handle::Rotate),
                _ => None,
            };
            let show_handles = self.tool == DrawTool::Select;
            for (h, q) in handles.iter().filter(|_| show_handles) {
                let s = to_s(*q);
                let hot = hover == Some(*h) && active.is_none() || active == Some(*h);
                match h {
                    crate::app::Handle::Scale(..) => {
                        let r = Rect::from_center_size(s, vec2(9.0, 9.0) * if hot { 1.25 } else { 1.0 });
                        painter.rect_filled(r, CornerRadius::same(2), if hot { ACCENT } else { pal.sheet });
                        painter.rect_stroke(r, CornerRadius::same(2), Stroke::new(1.5_f32, ACCENT), egui::StrokeKind::Middle);
                    }
                    crate::app::Handle::Rotate => {
                        // A small curved arrow tells it rotates.
                        let rr = if hot { 7.0 } else { 5.5 };
                        painter.circle_filled(s, rr, if hot { ACCENT } else { pal.sheet });
                        painter.circle_stroke(s, rr, Stroke::new(1.5_f32, ACCENT));
                        let arc: Vec<Pos2> = (0..=8).map(|k| {
                            let a = 0.6 + k as f32 / 8.0 * 4.0;
                            s + vec2(a.cos(), a.sin()) * (rr * 0.5)
                        }).collect();
                        painter.add(Shape::line(arc, Stroke::new(1.2_f32, if hot { Color32::WHITE } else { ACCENT })));
                    }
                }
            }
            painter.circle_filled(sr.center(), 2.5, ACCENT);
            let label = match &self.drag {
                Drag::Rotate { angle, .. } => format!("{angle:.1}°"),
                _ => format!("{:.1} × {:.1}", b.width(), b.height()),
            };
            painter.text(sr.center_top() - vec2(0.0, 10.0), Align2::CENTER_BOTTOM, label, FontId::proportional(11.0), ACCENT);
        }

        // in-progress previews
        let preview = Stroke::new(1.5_f32, ACCENT);
        match &self.drag {
            Drag::Create { start } => {
                if let Some(h) = hover_j.or(resp.interact_pointer_pos().map(to_j)) {
                    let h = self.snap_pt(h);
                    if let Some((kind, center)) = self.shape_from_drag(*start, h) {
                        let v = geom::Vector::new(0, kind, center, 0);
                        for poly in v.polys() {
                            painter.add(Shape::closed_line(poly.pts.iter().map(|p| to_s(*p)).collect(), preview));
                        }
                    }
                }
            }
            Drag::Freehand { pts } => {
                painter.add(Shape::line(pts.iter().map(|p| to_s(*p)).collect(), preview));
            }
            Drag::Marquee { start } => {
                if let Some(h) = resp.interact_pointer_pos() {
                    let r = Rect::from_two_pos(to_s(*start), h);
                    painter.rect_filled(r, CornerRadius::ZERO, ACCENT.gamma_multiply(0.12));
                    painter.rect_stroke(r, CornerRadius::ZERO, Stroke::new(1.0_f32, ACCENT), egui::StrokeKind::Inside);
                }
            }
            _ => {}
        }
        if self.tool == DrawTool::Arc && !self.poly_pts.is_empty() {
            let h = hover_j.map(|h| self.snap_pt(h));
            let q = &self.poly_pts;
            let line = match (q.len(), h) {
                (1, Some(h)) => vec![q[0], h],
                (2, Some(h)) => geom::arc_points(q[0], h, q[1]),
                _ => q.clone(),
            };
            painter.add(Shape::line(line.iter().map(|p| to_s(*p)).collect(), preview));
            for p in q {
                painter.circle_filled(to_s(*p), 3.5, ACCENT);
            }
        } else if self.tool == DrawTool::Node {
            if let Some((_, pts, _)) = self.node_points() {
                let active = if let Drag::Node { idx, .. } = self.drag { Some(idx) } else { None };
                for (i, p) in pts.iter().enumerate() {
                    let r = Rect::from_center_size(to_s(*p), vec2(7.0, 7.0));
                    painter.rect_filled(r, CornerRadius::ZERO, if active == Some(i) { ACCENT } else { pal.sheet });
                    painter.rect_stroke(r, CornerRadius::ZERO, Stroke::new(1.2_f32, ACCENT), egui::StrokeKind::Middle);
                }
            }
        } else if !self.poly_pts.is_empty() {
            let shift = ui.input(|i| i.modifiers.shift);
            let next = if self.tool == DrawTool::Polyline { hover_j.map(|h| self.polyline_next(h, shift)) } else { hover_j.map(|h| (self.snap_pt(h), false)) };
            let closing = next.is_some_and(|(_, c)| c);
            let close_col = Color32::from_rgb(52, 199, 120);
            let mut job: Vec<Pos2> = self.poly_pts.clone();
            if let Some((p, _)) = next {
                job.push(p);
            }
            let pts: Vec<Pos2> = job.iter().map(|p| to_s(*p)).collect();
            painter.add(Shape::line(pts.clone(), if closing { Stroke::new(2.0_f32, close_col) } else { preview }));
            let tag = |at: Pos2, text: String, col: Color32| {
                let g = painter.layout_no_wrap(text, FontId::proportional(11.0), Color32::WHITE);
                let r = Rect::from_center_size(at, g.size() + vec2(8.0, 4.0));
                painter.rect_filled(r, CornerRadius::same(4), col.gamma_multiply(0.9));
                painter.galley(r.min + vec2(4.0, 2.0), g, Color32::WHITE);
            };
            // Length of every segment, the live one included, on a small pill at its middle.
            for w in job.windows(2) {
                let len = (w[1] - w[0]).length();
                if len < 1e-3 {
                    continue;
                }
                let (a, b) = (to_s(w[0]), to_s(w[1]));
                let n = (b - a).normalized().rot90() * 12.0;
                tag(a + (b - a) * 0.5 - n, format!("{len:.1} mm"), ACCENT);
            }
            // Corner angle at each clicked point that has segments on both sides.
            for i in 1..job.len().saturating_sub(1) {
                if let Some(deg) = corner_angle(job[i - 1], job[i], job[i + 1]) {
                    let (a, v, b) = (to_s(job[i - 1]), to_s(job[i]), to_s(job[i + 1]));
                    // Put the label inside the corner, along the bisector.
                    let bis = ((a - v).normalized() + (b - v).normalized()).normalized();
                    let bis = if bis.x.is_nan() { vec2(0.0, -1.0) } else { bis };
                    let r = 16.0;
                    let a0 = (a - v).angle();
                    let mut sweep = (b - v).angle() - a0;
                    while sweep > std::f32::consts::PI {
                        sweep -= std::f32::consts::TAU;
                    }
                    while sweep < -std::f32::consts::PI {
                        sweep += std::f32::consts::TAU;
                    }
                    let arc: Vec<Pos2> = (0..=12).map(|k| v + vec2((a0 + sweep * k as f32 / 12.0).cos(), (a0 + sweep * k as f32 / 12.0).sin()) * r).collect();
                    painter.add(Shape::line(arc, Stroke::new(1.0_f32, Color32::from_rgb(255, 184, 77))));
                    tag(v + bis * 34.0, format!("{deg:.0}°"), Color32::from_rgb(214, 140, 30));
                }
            }
            for (i, p) in pts[..self.poly_pts.len()].iter().enumerate() {
                if i == 0 && closing {
                    // The start point lights up to show the next click closes the shape.
                    painter.circle_filled(*p, 7.0, close_col);
                    painter.circle_stroke(*p, 10.0, Stroke::new(1.5_f32, close_col));
                } else if i == 0 && self.poly_pts.len() >= 3 && self.tool == DrawTool::Polyline {
                    painter.circle_stroke(*p, 6.0, Stroke::new(1.5_f32, ACCENT));
                    painter.circle_filled(*p, 3.5, ACCENT);
                } else {
                    painter.circle_filled(*p, 3.5, ACCENT);
                }
            }
            if closing {
                tag(pts[0] + vec2(0.0, -24.0), "Click to close".into(), close_col);
            }
        }
        if self.tool == DrawTool::Sculpt {
            if let Some(h) = resp.hover_pos() {
                painter.circle_stroke(h, self.p.brush_size * z, Stroke::new(1.5_f32, ACCENT));
                painter.circle_stroke(h, self.p.brush_size * z * 0.5, Stroke::new(1.0_f32, ACCENT.gamma_multiply(0.5)));
            }
        }

        // rulers
        {
            let rw = 22.0;
            let bar_bg = if self.dark { Color32::from_rgb(28, 31, 38) } else { Color32::from_rgb(244, 245, 248) };
            let top = Rect::from_min_size(rect.min, vec2(rect.width(), rw));
            let left = Rect::from_min_size(rect.min, vec2(rw, rect.height()));
            painter.rect_filled(top, CornerRadius::ZERO, bar_bg);
            painter.rect_filled(left, CornerRadius::ZERO, bar_bg);
            let mut step = 1.0;
            for s in [1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0, 1000.0] {
                step = s;
                if s * z >= 9.0 {
                    break;
                }
            }
            let label_every = step * if step * z * 10.0 >= 60.0 { 10.0 } else { 20.0 };
            let j0 = to_j(rect.min);
            let j1 = to_j(rect.max);
            let tick = Stroke::new(1.0_f32, pal.weak.gamma_multiply(0.7));
            let mut x = (j0.x / step).floor() * step;
            while x <= j1.x {
                let sx = to_s(pos2(x, 0.0)).x;
                if sx > rect.left() + rw {
                    let major = ((x / label_every).round() * label_every - x).abs() < step * 0.01;
                    let mid = ((x / (step * 5.0)).round() * step * 5.0 - x).abs() < step * 0.01;
                    let h = if major { 12.0 } else if mid { 7.0 } else { 4.0 };
                    painter.line_segment([pos2(sx, rect.top() + rw - h), pos2(sx, rect.top() + rw)], tick);
                    if major {
                        painter.text(pos2(sx + 3.0, rect.top() + 2.0), Align2::LEFT_TOP, format!("{x:.0}"), FontId::proportional(10.0), pal.weak);
                    }
                }
                x += step;
            }
            let mut y = (j1.y / step).floor() * step;
            while y <= j0.y {
                let sy = to_s(pos2(0.0, y)).y;
                if sy > rect.top() + rw {
                    let major = ((y / label_every).round() * label_every - y).abs() < step * 0.01;
                    let mid = ((y / (step * 5.0)).round() * step * 5.0 - y).abs() < step * 0.01;
                    let h = if major { 12.0 } else if mid { 7.0 } else { 4.0 };
                    painter.line_segment([pos2(rect.left() + rw - h, sy), pos2(rect.left() + rw, sy)], tick);
                    if major {
                        painter.text(pos2(rect.left() + 2.0, sy - 2.0), Align2::LEFT_BOTTOM, format!("{y:.0}"), FontId::proportional(10.0), pal.weak);
                    }
                }
                y += step;
            }
            if let Some(h) = resp.hover_pos() {
                painter.line_segment([pos2(h.x, rect.top()), pos2(h.x, rect.top() + rw)], Stroke::new(1.0_f32, ACCENT));
                painter.line_segment([pos2(rect.left(), h.y), pos2(rect.left() + rw, h.y)], Stroke::new(1.0_f32, ACCENT));
            }
            painter.rect_filled(Rect::from_min_size(rect.min, vec2(rw, rw)), CornerRadius::ZERO, bar_bg);
        }

        // hint
        let hint = match self.tool {
            DrawTool::Select => "Click or box-select · drag to move · corner squares scale (Shift = free), edge squares stretch · round dots rotate (Shift = 15°)",
            DrawTool::Rect | DrawTool::Ellipse => "Drag to draw · Esc returns to Select",
            DrawTool::Circle | DrawTool::Polygon | DrawTool::Star => "Drag out from the centre · Esc returns to Select",
            DrawTool::Polyline => "Click to add points · Shift locks to 45°/90° · click the green start point to close · double-click or Enter to finish",
            DrawTool::Freehand => "Drag to sketch · finish near the start to close the shape",
            DrawTool::Text => "Click where the text should go",
            DrawTool::Arc => "Click the start, then the end, then a point the arc passes through",
            DrawTool::Node => "Drag nodes to reshape · double-click a line to add a node, a node to delete it · click another shape to edit it",
            DrawTool::Trim => "Click the piece of a vector to cut away between the lines that cross it",
            DrawTool::Fillet => "Click a corner to round it · radius is in the Draw card",
            DrawTool::Sculpt => "Paint on the model · brush size and strength are in the Sculpt card",
        };
        let hr = Rect::from_min_size(rect.left_bottom() + vec2(12.0, -34.0), vec2(rect.width() - 24.0, 24.0));
        painter.text(hr.left_center(), Align2::LEFT_CENTER, hint, FontId::proportional(12.0), pal.weak);
    }

    /// Apply a scale or rotate handle drag, starting from the shapes as they were when it began.
    fn drag_handle(&mut self, p: Pos2, free: bool) {
        match &mut self.drag {
            Drag::Scale { orig, bounds: b, hx, hy } => {
                let (hx, hy) = (*hx, *hy);
                let side = |h: i8, lo: f32, mid: f32, hi: f32| match h {
                    1 => (lo, hi),
                    -1 => (hi, lo),
                    _ => (mid, mid),
                };
                let (ax, hxp) = side(hx, b.min.x, b.center().x, b.max.x);
                let (ay, hyp) = side(hy, b.min.y, b.center().y, b.max.y);
                let ratio = |cur: f32, anchor: f32, handle: f32| {
                    let span = handle - anchor;
                    if span.abs() < 1e-6 {
                        1.0
                    } else {
                        ((cur - anchor) / span).max(0.01)
                    }
                };
                let mut sx = if hx != 0 { ratio(p.x, ax, hxp) } else { 1.0 };
                let mut sy = if hy != 0 { ratio(p.y, ay, hyp) } else { 1.0 };
                if hx != 0 && hy != 0 && !free {
                    // Corners keep proportions unless Shift is held.
                    let s = if (sx - 1.0).abs() > (sy - 1.0).abs() { sx } else { sy };
                    sx = s;
                    sy = s;
                }
                let anchor = pos2(ax, ay);
                let scaled: Vec<Vector> = orig.iter().map(|v0| scale_vector(v0, anchor, sx, sy)).collect();
                for nv in scaled {
                    if let Some(v) = self.doc.vectors.iter_mut().find(|v| v.id == nv.id) {
                        *v = nv;
                    }
                }
                self.status = format!("Scale {:.0}% × {:.0}%", sx * 100.0, sy * 100.0);
            }
            Drag::Rotate { orig, center, a0, angle } => {
                let c = *center;
                let mut da = ((p.y - c.y).atan2(p.x - c.x) - *a0).to_degrees();
                if free {
                    da = (da / 15.0).round() * 15.0;
                }
                *angle = da;
                let (s, co) = da.to_radians().sin_cos();
                let rotated: Vec<Vector> = orig
                    .iter()
                    .map(|v0| {
                        let mut v = v0.clone();
                        let d = v0.center - c;
                        v.center = c + vec2(d.x * co - d.y * s, d.x * s + d.y * co);
                        v.angle = v0.angle + da;
                        v
                    })
                    .collect();
                for nv in rotated {
                    if let Some(v) = self.doc.vectors.iter_mut().find(|v| v.id == nv.id) {
                        *v = nv;
                    }
                }
                self.status = format!("Rotate {da:.1}°  (Shift snaps to 15°)");
            }
            _ => {}
        }
    }

    fn shape_from_drag(&self, a: Pos2, b: Pos2) -> Option<(ShapeKind, Pos2)> {
        let d = (b - a).length();
        if d < 0.5 {
            return None;
        }
        let r = Rect::from_two_pos(a, b);
        Some(match self.tool {
            DrawTool::Rect => (ShapeKind::Rect { w: r.width().max(0.1), h: r.height().max(0.1), r: self.p.rect_r }, r.center()),
            DrawTool::Ellipse => (ShapeKind::Ellipse { rx: (r.width() * 0.5).max(0.05), ry: (r.height() * 0.5).max(0.05) }, r.center()),
            DrawTool::Circle => (ShapeKind::Ellipse { rx: d, ry: d }, a),
            DrawTool::Polygon => (ShapeKind::Polygon { r: d, sides: self.p.sides }, a),
            DrawTool::Star => (ShapeKind::Star { r1: d, r2: d * self.p.star_ratio, points: self.p.star_points }, a),
            _ => return None,
        })
    }

    /// Where the selection handles are, in job units: scale squares on the corners and edge
    /// middles of the (slightly padded) selection box, rotate dots just outside each corner.
    pub fn selection_handles(&self) -> Option<(Rect, Vec<(Handle, Pos2)>)> {
        let b = self.selection_bounds()?;
        let z = self.v2_zoom.max(1e-3);
        let r = b.expand(5.0 / z);
        let mut out = Vec::new();
        for hy in [-1i8, 0, 1] {
            for hx in [-1i8, 0, 1] {
                if hx == 0 && hy == 0 {
                    continue;
                }
                let x = [r.min.x, r.center().x, r.max.x][(hx + 1) as usize];
                let y = [r.min.y, r.center().y, r.max.y][(hy + 1) as usize];
                out.push((Handle::Scale(hx, hy), pos2(x, y)));
                if hx != 0 && hy != 0 {
                    let off = 15.0 / z;
                    out.push((Handle::Rotate, pos2(x + hx as f32 * off, y + hy as f32 * off)));
                }
            }
        }
        Some((b, out))
    }

    /// Where the next polyline point goes for a pointer at `raw`, and whether it closes the
    /// shape: it snaps onto the first point when near it, and with Shift the new segment is
    /// locked to 45° steps (which includes 90°).
    pub fn polyline_next(&self, raw: Pos2, shift: bool) -> (Pos2, bool) {
        let pts = &self.poly_pts;
        let z = self.v2_zoom.max(1e-3);
        if pts.len() >= 3 && (pts[0] - raw).length() <= 12.0 / z {
            return (pts[0], true);
        }
        if let (true, Some(last)) = (shift, pts.last()) {
            let d = raw - *last;
            let a = (d.y.atan2(d.x) / std::f32::consts::FRAC_PI_4).round() * std::f32::consts::FRAC_PI_4;
            let len = d.length();
            // A snapped length too, when the grid snap is on.
            let len = if self.snap { (len / 5.0).round() * 5.0 } else { len };
            return (*last + vec2(a.cos(), a.sin()) * len, false);
        }
        (self.snap_pt(raw), false)
    }

    fn handle_at(&self, p: Pos2) -> Option<Handle> {
        let (b, hs) = self.selection_handles()?;
        let z = self.v2_zoom.max(1e-3);
        // Scale squares win when the pointer is right on them.
        let reach = 9.0 / z;
        let scale = hs
            .iter()
            .filter(|(h, q)| matches!(h, Handle::Scale(..)) && (*q - p).length() <= reach)
            .min_by(|a, c| (a.1 - p).length().total_cmp(&(c.1 - p).length()))
            .map(|(h, _)| *h);
        if scale.is_some() {
            return scale;
        }
        // Rotate anywhere in a generous zone just outside each corner of the box.
        let r = b.expand(5.0 / z);
        let near_corner = [r.left_top(), r.right_top(), r.left_bottom(), r.right_bottom()].iter().any(|c| (*c - p).length() <= 42.0 / z);
        (near_corner && !r.contains(p)).then_some(Handle::Rotate)
    }

    fn handle_2d_input(&mut self, resp: &egui::Response, to_j: impl Fn(Pos2) -> Pos2, tol: f32) {
        let shift = resp.ctx.input(|i| i.modifiers.shift || i.modifiers.command);
        let pos = resp.interact_pointer_pos().map(&to_j);
        match self.tool {
            DrawTool::Select => {
                // Cursor feedback over the handles.
                if let Some(h) = resp.hover_pos().map(&to_j).and_then(|p| self.handle_at(p)) {
                    let icon = match h {
                        Handle::Rotate => egui::CursorIcon::Alias,
                        Handle::Scale(0, _) => egui::CursorIcon::ResizeVertical,
                        Handle::Scale(_, 0) => egui::CursorIcon::ResizeHorizontal,
                        Handle::Scale(hx, hy) if hx == hy => egui::CursorIcon::ResizeNeSw,
                        Handle::Scale(..) => egui::CursorIcon::ResizeNwSe,
                    };
                    resp.ctx.set_cursor_icon(icon);
                }
                if resp.drag_started_by(PointerButton::Primary) {
                    let start = to_j(resp.ctx.input(|i| i.pointer.press_origin()).unwrap_or(resp.rect.center()));
                    if let (Some(h), Some((b, _))) = (self.handle_at(start), self.selection_handles()) {
                        self.push_undo();
                        let orig: Vec<Vector> = self.selected_vectors().into_iter().cloned().collect();
                        self.drag = match h {
                            Handle::Scale(hx, hy) => Drag::Scale { orig, bounds: b, hx, hy },
                            Handle::Rotate => {
                                let c = b.center();
                                Drag::Rotate { orig, center: c, a0: (start.y - c.y).atan2(start.x - c.x), angle: 0.0 }
                            }
                        };
                    }
                }
                if resp.dragged_by(PointerButton::Primary) {
                    if let Some(p) = pos {
                        self.drag_handle(p, shift);
                    }
                }
                if matches!(self.drag, Drag::Scale { .. } | Drag::Rotate { .. }) {
                    if resp.drag_stopped_by(PointerButton::Primary) {
                        self.drag = Drag::None;
                    }
                    return;
                }
                if resp.drag_started_by(PointerButton::Primary) {
                    if let Some(p) = pos {
                        let start = to_j(resp.ctx.input(|i| i.pointer.press_origin()).unwrap_or(resp.rect.center()));
                        let hit = self.hit_vector(start, tol).or_else(|| self.hit_vector(p, tol));
                        match hit {
                            Some(id) => {
                                if !self.sel.contains(&id) {
                                    if !shift {
                                        self.sel.clear();
                                    }
                                    self.sel.push(id);
                                }
                                self.push_undo();
                                self.drag = Drag::Move;
                            }
                            None => self.drag = Drag::Marquee { start },
                        }
                    }
                }
                if resp.dragged_by(PointerButton::Primary) {
                    if let Drag::Move = self.drag {
                        let d = resp.drag_delta();
                        let dj = vec2(d.x / self.v2_zoom, -d.y / self.v2_zoom);
                        let sel = self.sel.clone();
                        for v in self.doc.vectors.iter_mut().filter(|v| sel.contains(&v.id)) {
                            v.center += dj;
                        }
                    }
                }
                if resp.drag_stopped_by(PointerButton::Primary) {
                    if let (Drag::Marquee { start }, Some(p)) = (&self.drag, pos) {
                        let r = Rect::from_two_pos(*start, p);
                        if !shift {
                            self.sel.clear();
                        }
                        let ids: Vec<u64> = self.doc.vectors.iter().filter(|v| r.contains_rect(v.bounds())).map(|v| v.id).collect();
                        for id in ids {
                            if !self.sel.contains(&id) {
                                self.sel.push(id);
                            }
                        }
                    }
                    if let Drag::Move = self.drag {
                        if self.snap {
                            let sel = self.sel.clone();
                            for v in self.doc.vectors.iter_mut().filter(|v| sel.contains(&v.id)) {
                                v.center = pos2((v.center.x * 2.0).round() / 2.0, (v.center.y * 2.0).round() / 2.0);
                            }
                        }
                    }
                    self.drag = Drag::None;
                }
                // A click on a handle keeps the selection rather than clearing it.
                if resp.clicked() && !pos.is_some_and(|p| self.handle_at(p).is_some()) {
                    if let Some(p) = pos {
                        match self.hit_vector(p, tol) {
                            Some(id) => {
                                if shift {
                                    if let Some(k) = self.sel.iter().position(|x| *x == id) {
                                        self.sel.remove(k);
                                    } else {
                                        self.sel.push(id);
                                    }
                                } else {
                                    self.sel = vec![id];
                                }
                            }
                            None => {
                                if !shift {
                                    self.sel.clear();
                                }
                            }
                        }
                    }
                }
            }
            DrawTool::Rect | DrawTool::Circle | DrawTool::Ellipse | DrawTool::Polygon | DrawTool::Star => {
                if resp.drag_started_by(PointerButton::Primary) {
                    let start = to_j(resp.ctx.input(|i| i.pointer.press_origin()).unwrap_or(resp.rect.center()));
                    self.drag = Drag::Create { start: self.snap_pt(start) };
                }
                if resp.drag_stopped_by(PointerButton::Primary) {
                    if let (Drag::Create { start }, Some(p)) = (&self.drag, pos) {
                        let end = self.snap_pt(p);
                        if let Some((kind, center)) = self.shape_from_drag(*start, end) {
                            self.push_undo();
                            let label = kind.label();
                            let id = self.add_vector(kind, center);
                            self.sel = vec![id];
                            self.toast(format!("{label} added"));
                        }
                    }
                    self.drag = Drag::None;
                }
            }
            DrawTool::Freehand => {
                if resp.drag_started_by(PointerButton::Primary) {
                    let start = to_j(resp.ctx.input(|i| i.pointer.press_origin()).unwrap_or(resp.rect.center()));
                    self.drag = Drag::Freehand { pts: vec![start] };
                }
                if resp.dragged_by(PointerButton::Primary) {
                    if let (Drag::Freehand { pts }, Some(p)) = (&mut self.drag, pos) {
                        if pts.last().is_none_or(|l| (*l - p).length() > 3.0 / self.v2_zoom) {
                            pts.push(p);
                        }
                    }
                }
                if resp.drag_stopped_by(PointerButton::Primary) {
                    if let Drag::Freehand { pts } = std::mem::replace(&mut self.drag, Drag::None) {
                        if pts.len() >= 3 {
                            let closed = (pts[0] - *pts.last().unwrap()).length() < 10.0 / self.v2_zoom;
                            let simple = geom::simplify(&pts, 0.6 / self.v2_zoom);
                            self.push_undo();
                            let id = self.add_path(simple, closed);
                            self.sel = vec![id];
                        }
                    }
                }
            }
            DrawTool::Polyline => {
                // A double-click finishes only when both clicks land on the same spot;
                // quick clicks in different places keep adding points.
                let same_spot = pos.zip(self.poly_pts.last().copied()).is_some_and(|(p, last)| (p - last).length() <= tol * 1.5);
                if resp.double_clicked() && same_spot {
                    self.poly_pts.pop();
                    self.finish_polyline(false);
                } else if resp.clicked() {
                    if let Some(p) = pos {
                        let (p, closes) = self.polyline_next(p, shift_only(resp));
                        if closes {
                            self.finish_polyline(true);
                        } else {
                            self.poly_pts.push(p);
                        }
                    }
                }
            }
            DrawTool::Text => {
                if resp.clicked() {
                    if let Some(p) = pos {
                        if !self.p.text.trim().is_empty() {
                            self.push_undo();
                            let id = self.add_vector(ShapeKind::Text { text: self.p.text.clone(), size: self.p.text_size }, self.snap_pt(p));
                            self.sel = vec![id];
                            self.tool = DrawTool::Select;
                        }
                    }
                }
            }
            DrawTool::Arc => {
                if resp.clicked() {
                    if let Some(p) = pos {
                        self.poly_pts.push(self.snap_pt(p));
                        if self.poly_pts.len() == 3 {
                            let q = std::mem::take(&mut self.poly_pts);
                            self.push_undo();
                            let id = self.add_path(geom::arc_points(q[0], q[2], q[1]), false);
                            self.sel = vec![id];
                            self.toast("Arc added");
                        }
                    }
                }
            }
            DrawTool::Node => {
                let press = resp.ctx.input(|i| i.pointer.press_origin()).map(&to_j);
                self.node_input(resp, pos, press, tol)
            }
            DrawTool::Trim => {
                if resp.clicked() {
                    if let Some(p) = pos {
                        self.trim_at(p, tol);
                    }
                }
            }
            DrawTool::Fillet => {
                if resp.clicked() {
                    if let Some(p) = pos {
                        self.fillet_at(p, tol * 1.5);
                    }
                }
            }
            DrawTool::Sculpt => {
                if resp.drag_started_by(PointerButton::Primary) || resp.clicked() {
                    self.push_undo();
                    let t = pos.map(|p| self.doc.model.height_at(p)).unwrap_or(0.0);
                    self.drag = Drag::Sculpt { target: t };
                }
                let down = resp.ctx.input(|i| i.pointer.primary_down());
                if let (Drag::Sculpt { target }, Some(p)) = (&self.drag, pos) {
                    if down || resp.clicked() {
                        let t = *target;
                        let (b, s, st) = (self.p.brush, self.p.brush_size, self.p.brush_strength);
                        self.doc.model.brush(b, p, s, st, t);
                    }
                }
                if resp.drag_stopped() || resp.clicked() {
                    self.drag = Drag::None;
                    self.invalidate_sim();
                }
            }
        }
    }

    fn visible_vector(&self, v: &geom::Vector) -> bool {
        self.doc.layers.get(v.layer).map(|l| l.visible).unwrap_or(true)
    }

    /// The nearest visible vector whose outline passes within `tol` of `p`.
    fn hit_edge(&self, p: Pos2, tol: f32) -> Option<u64> {
        let mut best: Option<(f32, u64)> = None;
        for v in self.doc.vectors.iter().filter(|v| self.visible_vector(v)) {
            let d = geom::dist_to_polys(p, &v.polys());
            if d <= tol && best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, v.id));
            }
        }
        best.map(|(_, id)| id)
    }

    /// World positions of the nodes of the vector being node-edited.
    fn node_points(&self) -> Option<(u64, Vec<Pos2>, bool)> {
        let id = *self.sel.first()?;
        let v = self.doc.vectors.iter().find(|v| v.id == id)?;
        let polys = v.polys();
        (polys.len() == 1).then(|| (id, polys[0].pts.clone(), polys[0].closed))
    }

    fn node_input(&mut self, resp: &egui::Response, pos: Option<Pos2>, press: Option<Pos2>, tol: f32) {
        let near_node = |app: &App, p: Pos2| {
            let (id, pts, _) = app.node_points()?;
            pts.iter().enumerate().map(|(i, q)| ((*q - p).length(), i)).filter(|(d, _)| *d <= tol).min_by(|a, b| a.0.total_cmp(&b.0)).map(|(_, i)| (id, i))
        };
        let with_path = |app: &mut App, id: u64, f: &mut dyn FnMut(&mut Vec<Pos2>, bool, Pos2)| {
            if let Some(v) = app.doc.vectors.iter_mut().find(|v| v.id == id) {
                if v.to_path() {
                    let c = v.center;
                    if let ShapeKind::Path { pts, closed } = &mut v.kind {
                        f(pts, *closed, c);
                    }
                }
            }
        };
        if resp.double_clicked() {
            let Some(p) = pos else { return };
            if let Some((id, i)) = near_node(self, p) {
                self.push_undo();
                with_path(self, id, &mut |pts, closed, _| {
                    if pts.len() > if closed { 3 } else { 2 } {
                        pts.remove(i);
                    }
                });
            } else if let Some((id, pts, closed)) = self.node_points() {
                let poly = geom::Poly { pts, closed };
                if geom::dist_to_polys(p, std::slice::from_ref(&poly)) <= tol {
                    let q = geom::closest_on_poly(p, &poly);
                    let seg = geom::segments(&poly).enumerate().min_by(|a, b| geom::seg_dist(q, a.1 .0, a.1 .1).total_cmp(&geom::seg_dist(q, b.1 .0, b.1 .1))).map(|(k, _)| k);
                    if let Some(k) = seg {
                        self.push_undo();
                        with_path(self, id, &mut |pts, _, c| pts.insert(k + 1, pos2(q.x - c.x, q.y - c.y)));
                    }
                }
            }
            return;
        }
        if resp.drag_started_by(PointerButton::Primary) {
            if let Some(p) = press {
                if let Some((id, idx)) = near_node(self, p) {
                    self.push_undo();
                    with_path(self, id, &mut |_, _, _| {});
                    self.drag = Drag::Node { id, idx };
                }
            }
        }
        if resp.dragged_by(PointerButton::Primary) {
            if let (Drag::Node { id, idx }, Some(p)) = (&self.drag, pos) {
                let (id, idx, p) = (*id, *idx, self.snap_pt(p));
                with_path(self, id, &mut |pts, _, c| {
                    if let Some(q) = pts.get_mut(idx) {
                        *q = pos2(p.x - c.x, p.y - c.y);
                    }
                });
            }
        }
        if resp.drag_stopped_by(PointerButton::Primary) {
            self.drag = Drag::None;
        }
        if resp.clicked() {
            if let Some(p) = pos {
                if near_node(self, p).is_none() {
                    self.sel = self.hit_vector(p, tol).into_iter().collect();
                }
            }
        }
    }

    fn trim_at(&mut self, p: Pos2, tol: f32) {
        let Some(id) = self.hit_edge(p, tol) else { return };
        let Some(v) = self.doc.vectors.iter().find(|v| v.id == id).cloned() else { return };
        let polys = v.polys();
        if polys.len() != 1 {
            self.toast("Trim works on single outlines; text can't be trimmed");
            return;
        }
        let others: Vec<geom::Poly> = self.doc.vectors.iter().filter(|o| o.id != id && self.visible_vector(o)).flat_map(|o| o.polys()).collect();
        match geom::trim(&polys[0], &others, p) {
            None => self.toast("Nothing crosses this vector here"),
            Some(pieces) => {
                self.push_undo();
                self.doc.vectors.retain(|x| x.id != id);
                self.sel.clear();
                for piece in pieces {
                    let nid = self.add_path(piece.pts, false);
                    if let Some(n) = self.doc.vectors.iter_mut().find(|x| x.id == nid) {
                        n.layer = v.layer;
                    }
                }
            }
        }
    }

    fn fillet_at(&mut self, p: Pos2, tol: f32) {
        let mut best: Option<(f32, u64, usize)> = None;
        for v in self.doc.vectors.iter().filter(|v| self.visible_vector(v)) {
            let polys = v.polys();
            if polys.len() != 1 {
                continue;
            }
            for (i, q) in polys[0].pts.iter().enumerate() {
                let d = (*q - p).length();
                if d <= tol && best.is_none_or(|(bd, _, _)| d < bd) {
                    best = Some((d, v.id, i));
                }
            }
        }
        let Some((_, id, i)) = best else {
            self.toast("Click on a corner");
            return;
        };
        self.push_undo();
        let r = self.p.fillet_r;
        let mut ok = false;
        if let Some(v) = self.doc.vectors.iter_mut().find(|v| v.id == id) {
            if v.to_path() {
                if let ShapeKind::Path { pts, closed } = &mut v.kind {
                    ok = geom::fillet_corner(pts, *closed, i, r);
                }
            }
        }
        if !ok {
            self.undo.pop();
            self.toast("That point isn't a corner");
        }
    }

    pub fn view_3d(&mut self, ui: &mut Ui) {
        let pal = theme::palette(self.dark);
        let (rect, resp) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        // background gradient
        let (top, bottom) = if self.dark { (Color32::from_rgb(34, 38, 47), Color32::from_rgb(14, 15, 19)) } else { (Color32::from_rgb(244, 246, 250), Color32::from_rgb(208, 214, 223)) };
        let mut bg = Mesh::default();
        bg.colored_vertex(rect.left_top(), top);
        bg.colored_vertex(rect.right_top(), top);
        bg.colored_vertex(rect.right_bottom(), bottom);
        bg.colored_vertex(rect.left_bottom(), bottom);
        bg.add_triangle(0, 1, 2);
        bg.add_triangle(0, 2, 3);
        painter.add(Shape::mesh(bg));

        if resp.dragged_by(PointerButton::Primary) {
            let d = resp.drag_delta();
            self.cam.yaw += d.x * 0.35;
            self.cam.pitch = (self.cam.pitch + d.y * 0.3).clamp(5.0, 90.0);
        }
        if resp.dragged_by(PointerButton::Secondary) || resp.dragged_by(PointerButton::Middle) {
            self.cam.pan += resp.drag_delta();
        }
        if resp.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                self.cam.zoom = (self.cam.zoom * (scroll * 0.0015).exp()).clamp(0.2, 20.0);
            }
        }
        let mat = self.doc.material.clone();
        let proj = Projector::new(&self.cam, rect, mat.w, mat.h);
        let showing_sim = self.show_sim && self.sim.is_some();
        // soft floor shadow
        {
            let corners = [(0.0, 0.0), (mat.w, 0.0), (mat.w, mat.h), (0.0, mat.h)];
            let pts: Vec<Pos2> = corners.iter().map(|(x, y)| proj.project(*x, *y, -mat.thickness).0 + vec2(0.0, 10.0)).collect();
            painter.add(Shape::convex_polygon(pts, Color32::from_black_alpha(if self.dark { 70 } else { 28 }), Stroke::NONE));
        }
        let side = Color32::from_rgb(196, 150, 100);
        if showing_sim {
            if let (Some(sim), Some(tex)) = (&self.sim, &self.sim_tex) {
                let s = &sim.stock;
                render3d::draw_surface(&painter, &proj, &Surface { heights: &s.z, nx: s.nx, ny: s.ny, cell: s.cell, texture: tex.id(), side, bottom: -mat.thickness });
            }
        } else if let Some(tex) = &self.model_tex {
            let g = self.doc.model.grid;
            if self.model_z.len() == g.len() {
                render3d::draw_surface(&painter, &proj, &Surface { heights: &self.model_z, nx: g.nx, ny: g.ny, cell: g.cell, texture: tex.id(), side, bottom: -mat.thickness });
            }
        }
        render3d::draw_box_edges(&painter, &proj, mat.w, mat.h, -mat.thickness, Stroke::new(1.0_f32, pal.weak.gamma_multiply(0.5)));
        if self.show_paths && !showing_sim {
            for (i, t) in self.doc.toolpaths.iter().enumerate().filter(|(_, t)| t.visible) {
                render3d::draw_chains(&painter, &proj, &t.chains, path_color(t.tool.kind, i), 40_000);
            }
        }
        // axis gizmo
        let g0 = rect.left_bottom() + vec2(54.0, -54.0);
        let (o, _) = proj.project(0.0, 0.0, 0.0);
        for (dx, dy, dz, col, l) in [(1.0, 0.0, 0.0, Color32::from_rgb(235, 87, 87), "X"), (0.0, 1.0, 0.0, Color32::from_rgb(76, 200, 120), "Y"), (0.0, 0.0, 1.0, ACCENT, "Z")] {
            let (p, _) = proj.project(dx * 10.0, dy * 10.0, dz * 10.0);
            let d = (p - o).normalized() * 30.0;
            let d = if d.x.is_nan() { vec2(0.0, -30.0) } else { d };
            painter.arrow(g0, d, Stroke::new(2.0_f32, col));
            painter.text(g0 + d * 1.25, Align2::CENTER_CENTER, l, FontId::proportional(11.0), col);
        }
        let info = if showing_sim {
            match &self.sim {
                Some(s) if !s.done => format!("Simulating… {:.0}%", s.progress() * 100.0),
                _ => "Cut preview · untick “Cut result” to see the design model".to_string(),
            }
        } else {
            let m = &self.doc.model;
            format!("Model height {:.1} mm · {} components · drag to orbit, right-drag to pan, wheel to zoom", (m.max_height() - m.min_height()).max(0.0), m.components.len())
        };
        painter.text(rect.right_bottom() + vec2(-14.0, -14.0), Align2::RIGHT_BOTTOM, info, FontId::proportional(12.0), pal.weak);
    }
}

#[cfg(test)]
mod handle_tests {
    use super::*;

    fn rect(angle: f32) -> Vector {
        let mut v = Vector::new(1, ShapeKind::Rect { w: 40.0, h: 20.0, r: 0.0 }, pos2(100.0, 50.0), 0);
        v.angle = angle;
        v
    }

    #[test]
    fn corner_scale_keeps_opposite_corner_fixed() {
        let v = rect(0.0);
        let b = v.bounds();
        let s = scale_vector(&v, b.min, 2.0, 1.5);
        let nb = s.bounds();
        assert!((nb.min - b.min).length() < 1e-3);
        assert!((nb.width() - 80.0).abs() < 1e-3 && (nb.height() - 30.0).abs() < 1e-3);
    }

    #[test]
    fn quarter_turned_shape_stretches_along_job_axes() {
        let v = rect(90.0);
        let b = v.bounds();
        let s = scale_vector(&v, b.center(), 2.0, 1.0);
        let nb = s.bounds();
        assert!((nb.width() - b.width() * 2.0).abs() < 1e-2, "{:?} vs {:?}", nb, b);
        assert!((nb.height() - b.height()).abs() < 1e-2);
    }

    #[test]
    fn skewed_shape_becomes_curve_with_right_size() {
        let v = rect(30.0);
        let b = v.bounds();
        let s = scale_vector(&v, b.min, 1.5, 1.0);
        let nb = s.bounds();
        assert!(matches!(s.kind, ShapeKind::Path { .. }));
        assert!((nb.width() - b.width() * 1.5).abs() < 1e-2 && (nb.height() - b.height()).abs() < 1e-2);
    }
}

#[cfg(test)]
mod polyline_tests {
    use super::*;

    #[test]
    fn corner_angles() {
        let a = corner_angle(pos2(0.0, 0.0), pos2(10.0, 0.0), pos2(10.0, 10.0)).unwrap();
        assert!((a - 90.0).abs() < 1e-3);
        let b = corner_angle(pos2(0.0, 0.0), pos2(10.0, 0.0), pos2(20.0, 0.0)).unwrap();
        assert!((b - 180.0).abs() < 1e-3);
        assert!(corner_angle(pos2(1.0, 1.0), pos2(1.0, 1.0), pos2(2.0, 2.0)).is_none());
    }
}
