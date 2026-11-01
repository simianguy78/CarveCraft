//! The 2D design canvas and the 3D view.

use crate::app::{App, Drag, DrawTool};
use crate::geom::{self, ShapeKind};
use crate::render3d::{self, Projector, Surface};
use crate::theme::{self, ACCENT};
use crate::toolpath::ToolKind;
use egui::{pos2, vec2, Align2, Color32, CornerRadius, FontId, Mesh, PointerButton, Pos2, Rect, Sense, Shape, Stroke, Ui};

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
        if let Some(b) = self.selection_bounds() {
            let sr = Rect::from_two_pos(to_s(b.min), to_s(b.max)).expand(5.0);
            let dash = Shape::dashed_line(&[sr.left_top(), sr.right_top(), sr.right_bottom(), sr.left_bottom(), sr.left_top()], Stroke::new(1.0_f32, ACCENT), 5.0, 4.0);
            painter.extend(dash);
            for p in [sr.left_top(), sr.right_top(), sr.right_bottom(), sr.left_bottom(), sr.center()] {
                painter.rect_filled(Rect::from_center_size(p, vec2(7.0, 7.0)), CornerRadius::same(2), ACCENT);
            }
            painter.text(sr.center_top() - vec2(0.0, 4.0), Align2::CENTER_BOTTOM, format!("{:.1} × {:.1}", b.width(), b.height()), FontId::proportional(11.0), ACCENT);
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
        if !self.poly_pts.is_empty() {
            let mut pts: Vec<Pos2> = self.poly_pts.iter().map(|p| to_s(*p)).collect();
            if let Some(h) = hover_j {
                pts.push(to_s(self.snap_pt(h)));
            }
            painter.add(Shape::line(pts.clone(), preview));
            for p in &pts[..self.poly_pts.len()] {
                painter.circle_filled(*p, 3.5, ACCENT);
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
            DrawTool::Select => "Click or drag a box to select · drag a selection to move it · right-drag to pan · wheel to zoom",
            DrawTool::Rect | DrawTool::Ellipse => "Drag to draw · Esc returns to Select",
            DrawTool::Circle | DrawTool::Polygon | DrawTool::Star => "Drag out from the centre · Esc returns to Select",
            DrawTool::Polyline => "Click to add points · click the first point to close · double-click or Enter to finish",
            DrawTool::Freehand => "Drag to sketch · finish near the start to close the shape",
            DrawTool::Text => "Click where the text should go",
            DrawTool::Sculpt => "Paint on the model · brush size and strength are in the Sculpt card",
        };
        let hr = Rect::from_min_size(rect.left_bottom() + vec2(12.0, -34.0), vec2(rect.width() - 24.0, 24.0));
        painter.text(hr.left_center(), Align2::LEFT_CENTER, hint, FontId::proportional(12.0), pal.weak);
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

    fn handle_2d_input(&mut self, resp: &egui::Response, to_j: impl Fn(Pos2) -> Pos2, tol: f32) {
        let shift = resp.ctx.input(|i| i.modifiers.shift || i.modifiers.command);
        let pos = resp.interact_pointer_pos().map(&to_j);
        match self.tool {
            DrawTool::Select => {
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
                if resp.clicked() {
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
                if resp.double_clicked() {
                    self.poly_pts.pop();
                    self.finish_polyline(false);
                } else if resp.clicked() {
                    if let Some(p) = pos {
                        let p = self.snap_pt(p);
                        if self.poly_pts.len() >= 3 && (self.poly_pts[0] - p).length() < tol * 1.5 {
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
