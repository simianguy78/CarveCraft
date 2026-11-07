//! Hand-drawn vector icons and the tile button used in the tool panels.

use crate::theme::{self, ACCENT};
use egui::{pos2, vec2, Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, Ui};

pub fn draw(p: &Painter, r: Rect, c: Color32, name: &str) {
    let s = Stroke::new(1.6_f32, c);
    let cx = r.center();
    let w = r.width().min(r.height());
    let u = |x: f32, y: f32| pos2(cx.x + x * w * 0.5, cx.y + y * w * 0.5);
    let poly = |pts: Vec<Pos2>| Shape::closed_line(pts, s);
    match name {
        "select" => {
            p.add(Shape::convex_polygon(vec![u(-0.45, -0.7), u(0.45, 0.15), u(0.0, 0.2), u(-0.2, 0.7)], c, Stroke::NONE));
        }
        "job" => {
            // material block with a dashed outline, like a job sheet
            p.add(poly(vec![u(-0.7, -0.2), u(0.1, -0.65), u(0.75, -0.3), u(-0.05, 0.15)]));
            p.add(Shape::line(vec![u(-0.7, -0.2), u(-0.7, 0.3), u(-0.05, 0.7), u(0.75, 0.25), u(0.75, -0.3)], s));
            p.line_segment([u(-0.05, 0.15), u(-0.05, 0.7)], s);
            p.circle_filled(u(-0.7, 0.3), 2.5, Color32::from_rgb(235, 87, 87));
        }
        "open" => {
            p.add(Shape::line(vec![u(-0.8, 0.6), u(-0.8, -0.55), u(-0.35, -0.55), u(-0.2, -0.35), u(0.6, -0.35), u(0.6, -0.1)], s));
            p.add(poly(vec![u(-0.8, 0.6), u(-0.5, -0.1), u(0.85, -0.1), u(0.55, 0.6)]));
        }
        "save" => {
            p.rect_stroke(Rect::from_center_size(cx, vec2(w * 0.8, w * 0.8)), 3.0, s, egui::StrokeKind::Middle);
            p.rect_filled(Rect::from_min_max(u(-0.25, -0.4), u(0.25, -0.1)), 1.0, c);
            p.rect_stroke(Rect::from_min_max(u(-0.4, 0.1), u(0.4, 0.4)), 1.0, s, egui::StrokeKind::Middle);
        }
        "tooldb" => {
            for (x, h) in [(-0.5f32, 0.55f32), (0.0, 0.75), (0.5, 0.6)] {
                p.rect_stroke(Rect::from_min_max(u(x - 0.14, -0.75), u(x + 0.14, h - 0.2)), 1.0, s, egui::StrokeKind::Middle);
                p.add(Shape::convex_polygon(vec![u(x - 0.14, h - 0.2), u(x + 0.14, h - 0.2), u(x, h + 0.05)], c, Stroke::NONE));
            }
        }
        "rect" => {
            p.rect_stroke(Rect::from_center_size(cx, vec2(w * 0.85, w * 0.6)), 2.0, s, egui::StrokeKind::Middle);
        }
        "circle" => {
            p.circle_stroke(cx, w * 0.38, s);
        }
        "ellipse" => {
            let pts = (0..40).map(|k| {
                let a = k as f32 / 40.0 * std::f32::consts::TAU;
                u(0.8 * a.cos(), 0.5 * a.sin())
            });
            p.add(poly(pts.collect()));
        }
        "polygon" => {
            let pts = (0..6).map(|k| {
                let a = k as f32 / 6.0 * std::f32::consts::TAU;
                u(0.75 * a.cos(), 0.75 * a.sin())
            });
            p.add(poly(pts.collect()));
        }
        "star" => {
            let pts = (0..10).map(|k| {
                let a = -std::f32::consts::FRAC_PI_2 + k as f32 / 10.0 * std::f32::consts::TAU;
                let rr = if k % 2 == 0 { 0.8 } else { 0.35 };
                u(rr * a.cos(), rr * a.sin())
            });
            p.add(poly(pts.collect()));
        }
        "polyline" => {
            let pts = vec![u(-0.75, 0.5), u(-0.25, -0.5), u(0.25, 0.35), u(0.75, -0.55)];
            p.add(Shape::line(pts.clone(), s));
            for q in pts {
                p.circle_filled(q, 2.5, c);
            }
        }
        "freehand" => {
            let pts = (0..30).map(|k| {
                let t = k as f32 / 29.0;
                u(-0.8 + 1.6 * t, 0.4 * (t * 9.0).sin() * (1.0 - t * 0.4))
            });
            p.add(Shape::line(pts.collect(), s));
        }
        "text" => {
            p.text(cx, Align2::CENTER_CENTER, "Aa", FontId::proportional(w * 0.62), c);
        }
        "duplicate" => {
            p.rect_stroke(Rect::from_min_size(u(-0.7, -0.7), vec2(w * 0.55, w * 0.55)), 2.0, s, egui::StrokeKind::Middle);
            p.rect_filled(Rect::from_min_size(u(-0.2, -0.2), vec2(w * 0.55, w * 0.55)), 2.0, c.gamma_multiply(0.6));
        }
        "delete" => {
            p.rect_stroke(Rect::from_min_max(u(-0.45, -0.35), u(0.45, 0.75)), 2.0, s, egui::StrokeKind::Middle);
            p.line_segment([u(-0.65, -0.5), u(0.65, -0.5)], s);
            p.line_segment([u(-0.2, -0.7), u(0.2, -0.7)], s);
            p.line_segment([u(-0.12, -0.1), u(-0.12, 0.5)], s);
            p.line_segment([u(0.15, -0.1), u(0.15, 0.5)], s);
        }
        "mirror_h" => {
            p.add(Shape::convex_polygon(vec![u(-0.1, -0.6), u(-0.1, 0.6), u(-0.8, 0.6)], c, Stroke::NONE));
            p.add(poly(vec![u(0.1, -0.6), u(0.1, 0.6), u(0.8, 0.6)]));
            p.line_segment([u(0.0, -0.85), u(0.0, 0.85)], Stroke::new(1.0_f32, c.gamma_multiply(0.6)));
        }
        "mirror_v" => {
            p.add(Shape::convex_polygon(vec![u(-0.6, -0.1), u(0.6, -0.1), u(0.6, -0.8)], c, Stroke::NONE));
            p.add(poly(vec![u(-0.6, 0.1), u(0.6, 0.1), u(0.6, 0.8)]));
            p.line_segment([u(-0.85, 0.0), u(0.85, 0.0)], Stroke::new(1.0_f32, c.gamma_multiply(0.6)));
        }
        "rotate" => {
            let pts: Vec<Pos2> = (0..30).map(|k| {
                let a = 0.4 + k as f32 / 29.0 * 4.9;
                u(0.65 * a.cos(), 0.65 * a.sin())
            }).collect();
            let end = *pts.last().unwrap();
            p.add(Shape::line(pts, s));
            p.add(Shape::convex_polygon(vec![end + vec2(-6.0, -2.0), end + vec2(4.0, -5.0), end + vec2(1.0, 5.0)], c, Stroke::NONE));
        }
        "center" => {
            p.rect_stroke(Rect::from_center_size(cx, vec2(w * 0.85, w * 0.85)), 2.0, Stroke::new(1.0_f32, c.gamma_multiply(0.6)), egui::StrokeKind::Middle);
            p.circle_stroke(cx, w * 0.2, s);
            p.line_segment([u(-0.6, 0.0), u(0.6, 0.0)], s);
            p.line_segment([u(0.0, -0.6), u(0.0, 0.6)], s);
        }
        "offset" => {
            p.rect_stroke(Rect::from_center_size(cx, vec2(w * 0.85, w * 0.85)), 7.0, s, egui::StrokeKind::Middle);
            p.rect_stroke(Rect::from_center_size(cx, vec2(w * 0.42, w * 0.42)), 3.0, Stroke::new(1.2_f32, c.gamma_multiply(0.6)), egui::StrokeKind::Middle);
        }
        "circ_array" => {
            for k in 0..8 {
                let a = k as f32 / 8.0 * std::f32::consts::TAU;
                p.circle_filled(u(0.62 * a.cos(), 0.62 * a.sin()), 2.6, c);
            }
            p.circle_stroke(cx, w * 0.12, Stroke::new(1.0_f32, c.gamma_multiply(0.6)));
        }
        "grid_array" => {
            for a in -1..=1 {
                for b in -1..=1 {
                    p.rect_filled(Rect::from_center_size(u(a as f32 * 0.55, b as f32 * 0.55), vec2(w * 0.2, w * 0.2)), 1.5, c);
                }
            }
        }
        "shape" => {
            let pts: Vec<Pos2> = (0..=30).map(|k| {
                let a = std::f32::consts::PI * k as f32 / 30.0;
                u(-0.75 * a.cos(), 0.35 - 0.7 * a.sin())
            }).collect();
            p.add(Shape::convex_polygon(pts, c.gamma_multiply(0.35), s));
            p.line_segment([u(-0.9, 0.35), u(0.9, 0.35)], s);
        }
        "prism" => {
            p.add(poly(vec![u(-0.8, 0.5), u(0.0, -0.6), u(0.8, 0.5)]));
            p.line_segment([u(0.0, -0.6), u(0.0, 0.5)], Stroke::new(1.0_f32, c.gamma_multiply(0.6)));
            p.line_segment([u(-0.8, 0.5), u(0.8, 0.5)], s);
        }
        "extrude" => {
            let pts: Vec<Pos2> = (0..30).map(|k| {
                let t = k as f32 / 29.0;
                u(-0.8 + 1.6 * t, 0.35 * (t * 5.5).sin())
            }).collect();
            p.add(Shape::line(pts.clone(), Stroke::new(w * 0.22, c.gamma_multiply(0.35))));
            p.add(Shape::line(pts, s));
        }
        "two_rail" => {
            let a: Vec<Pos2> = (0..20).map(|k| {
                let t = k as f32 / 19.0;
                u(-0.8 + 1.6 * t, -0.45 + 0.15 * (t * 6.0).sin())
            }).collect();
            let b: Vec<Pos2> = (0..20).map(|k| {
                let t = k as f32 / 19.0;
                u(-0.8 + 1.6 * t, 0.45 + 0.15 * (t * 6.0).sin())
            }).collect();
            for k in (0..20).step_by(4) {
                p.line_segment([a[k], b[k]], Stroke::new(1.0_f32, c.gamma_multiply(0.5)));
            }
            p.add(Shape::line(a, s));
            p.add(Shape::line(b, s));
        }
        "texture" => {
            for row in 0..4 {
                let y = -0.6 + row as f32 * 0.4;
                let pts: Vec<Pos2> = (0..20).map(|k| {
                    let t = k as f32 / 19.0;
                    u(-0.8 + 1.6 * t, y + 0.1 * (t * 12.0 + row as f32).sin())
                }).collect();
                p.add(Shape::line(pts, Stroke::new(1.2_f32, c)));
            }
        }
        "relief" => {
            for k in 0..6 {
                let a = k as f32 / 6.0 * std::f32::consts::TAU;
                p.circle_stroke(u(0.38 * a.cos(), 0.38 * a.sin()), w * 0.2, Stroke::new(1.2_f32, c));
            }
            p.circle_filled(cx, w * 0.13, c);
        }
        "import" => {
            p.line_segment([u(0.0, -0.8), u(0.0, 0.25)], s);
            p.add(Shape::convex_polygon(vec![u(-0.3, 0.0), u(0.3, 0.0), u(0.0, 0.35)], c, Stroke::NONE));
            p.add(Shape::line(vec![u(-0.75, 0.2), u(-0.75, 0.7), u(0.75, 0.7), u(0.75, 0.2)], s));
        }
        "smooth" => {
            let pts: Vec<Pos2> = (0..30).map(|k| {
                let t = k as f32 / 29.0;
                u(-0.8 + 1.6 * t, 0.25 - 0.45 * (t * std::f32::consts::PI).sin())
            }).collect();
            p.add(Shape::line(pts, s));
            p.line_segment([u(-0.8, 0.55), u(0.8, 0.55)], Stroke::new(1.0_f32, c.gamma_multiply(0.5)));
        }
        "deposit" => {
            p.circle_stroke(cx, w * 0.42, Stroke::new(1.0_f32, c.gamma_multiply(0.5)));
            p.line_segment([u(-0.4, 0.0), u(0.4, 0.0)], s);
            p.line_segment([u(0.0, -0.4), u(0.0, 0.4)], s);
        }
        "remove" => {
            p.circle_stroke(cx, w * 0.42, Stroke::new(1.0_f32, c.gamma_multiply(0.5)));
            p.line_segment([u(-0.4, 0.0), u(0.4, 0.0)], s);
        }
        "erase" => {
            p.add(poly(vec![u(-0.7, 0.2), u(0.0, -0.5), u(0.6, 0.1), u(-0.1, 0.8)]));
            p.line_segment([u(-0.35, -0.15), u(0.25, 0.45)], s);
        }
        "flatten" => {
            p.line_segment([u(-0.8, 0.0), u(0.8, 0.0)], Stroke::new(2.4_f32, c));
            p.line_segment([u(-0.5, -0.5), u(-0.5, -0.15)], Stroke::new(1.0_f32, c.gamma_multiply(0.6)));
            p.line_segment([u(0.5, -0.5), u(0.5, -0.15)], Stroke::new(1.0_f32, c.gamma_multiply(0.6)));
        }
        "bake" => {
            for k in 0..3 {
                let y = -0.45 + k as f32 * 0.35;
                p.add(poly(vec![u(-0.7, y), u(0.0, y - 0.25), u(0.7, y), u(0.0, y + 0.25)]));
            }
        }
        "stl" => {
            p.add(poly(vec![u(-0.6, -0.3), u(0.0, -0.65), u(0.6, -0.3), u(0.6, 0.4), u(0.0, 0.75), u(-0.6, 0.4)]));
            p.line_segment([u(-0.6, -0.3), u(0.0, 0.05)], s);
            p.line_segment([u(0.6, -0.3), u(0.0, 0.05)], s);
            p.line_segment([u(0.0, 0.05), u(0.0, 0.75)], s);
        }
        "profile" => {
            p.rect_stroke(Rect::from_center_size(cx, vec2(w * 0.5, w * 0.5)), 2.0, Stroke::new(1.2_f32, c.gamma_multiply(0.6)), egui::StrokeKind::Middle);
            p.rect_stroke(Rect::from_center_size(cx, vec2(w * 0.85, w * 0.85)), 8.0, Stroke::new(2.2_f32, ACCENT), egui::StrokeKind::Middle);
        }
        "pocket" => {
            let rr = Rect::from_center_size(cx, vec2(w * 0.8, w * 0.8));
            p.rect_filled(rr, 3.0, c.gamma_multiply(0.18));
            for k in 0..5 {
                let y = rr.top() + 4.0 + k as f32 * (rr.height() - 8.0) / 4.0;
                p.line_segment([pos2(rr.left() + 3.0, y), pos2(rr.right() - 3.0, y)], Stroke::new(1.3_f32, ACCENT));
            }
            p.rect_stroke(rr, 3.0, s, egui::StrokeKind::Middle);
        }
        "drill" => {
            p.circle_stroke(cx, w * 0.4, s);
            p.circle_filled(cx, w * 0.15, ACCENT);
            p.line_segment([u(-0.8, 0.0), u(0.8, 0.0)], Stroke::new(1.0_f32, c.gamma_multiply(0.5)));
            p.line_segment([u(0.0, -0.8), u(0.0, 0.8)], Stroke::new(1.0_f32, c.gamma_multiply(0.5)));
        }
        "vcarve" => {
            p.add(Shape::convex_polygon(vec![u(-0.7, -0.5), u(0.7, -0.5), u(0.0, 0.6)], ACCENT.gamma_multiply(0.5), s));
        }
        "engrave" => {
            let pts: Vec<Pos2> = (0..30).map(|k| {
                let t = k as f32 / 29.0;
                u(-0.8 + 1.6 * t, 0.35 * (t * 7.0).sin())
            }).collect();
            p.add(Shape::line(pts, Stroke::new(2.0_f32, ACCENT)));
        }
        "fluting" => {
            p.add(Shape::convex_polygon(vec![u(-0.8, 0.0), u(0.0, -0.18), u(0.8, 0.0), u(0.0, 0.18)], ACCENT, Stroke::NONE));
            p.line_segment([u(-0.8, -0.5), u(0.8, -0.5)], Stroke::new(1.0_f32, c.gamma_multiply(0.5)));
            p.line_segment([u(-0.8, 0.5), u(0.8, 0.5)], Stroke::new(1.0_f32, c.gamma_multiply(0.5)));
        }
        "rough3d" => {
            p.add(Shape::line(vec![u(-0.8, -0.6), u(-0.4, -0.6), u(-0.4, -0.2), u(0.0, -0.2), u(0.0, 0.2), u(0.4, 0.2), u(0.4, 0.6), u(0.8, 0.6)], Stroke::new(2.0_f32, ACCENT)));
        }
        "finish3d" => {
            for row in 0..3 {
                let y = -0.4 + row as f32 * 0.4;
                let pts: Vec<Pos2> = (0..24).map(|k| {
                    let t = k as f32 / 23.0;
                    u(-0.8 + 1.6 * t, y - 0.25 * (t * std::f32::consts::PI).sin())
                }).collect();
                p.add(Shape::line(pts, Stroke::new(1.4_f32, ACCENT)));
            }
        }
        "arc" => {
            let pts = (0..=24).map(|k| {
                let a = std::f32::consts::PI * (0.1 + 0.8 * k as f32 / 24.0);
                u(0.8 * a.cos(), 0.45 - 0.9 * a.sin())
            });
            p.add(Shape::line(pts.collect(), s));
        }
        "node" => {
            let pts = vec![u(-0.75, 0.45), u(-0.15, -0.55), u(0.7, 0.3)];
            p.add(Shape::line(pts.clone(), s));
            for q in &pts {
                p.rect_stroke(Rect::from_center_size(*q, vec2(6.0, 6.0)), 0.0, s, egui::StrokeKind::Middle);
            }
            p.rect_filled(Rect::from_center_size(pts[1], vec2(6.0, 6.0)), 0.0, ACCENT);
        }
        "weld" => {
            p.circle_stroke(u(-0.28, 0.0), w * 0.3, Stroke::new(1.0_f32, c.gamma_multiply(0.45)));
            p.circle_stroke(u(0.28, 0.0), w * 0.3, Stroke::new(1.0_f32, c.gamma_multiply(0.45)));
            let a0 = (0.28f32 / 0.6).acos();
            let mut pts = Vec::new();
            for k in 0..=30 {
                let a = a0 + (std::f32::consts::TAU - 2.0 * a0) * k as f32 / 30.0;
                pts.push(u(0.28 + 0.6 * a.cos(), 0.6 * a.sin()));
            }
            for k in 0..=30 {
                let a = std::f32::consts::PI + a0 - (std::f32::consts::TAU - 2.0 * a0) * k as f32 / 30.0 + std::f32::consts::TAU;
                pts.push(u(-0.28 + 0.6 * a.cos(), 0.6 * -a.sin()));
            }
            p.add(Shape::closed_line(pts, s));
        }
        "trim" => {
            p.line_segment([u(-0.8, 0.0), u(-0.25, 0.0)], s);
            p.line_segment([u(0.25, 0.0), u(0.8, 0.0)], s);
            p.line_segment([u(-0.25, -0.7), u(-0.25, 0.7)], Stroke::new(1.0_f32, c.gamma_multiply(0.6)));
            p.line_segment([u(0.25, -0.7), u(0.25, 0.7)], Stroke::new(1.0_f32, c.gamma_multiply(0.6)));
            p.extend(Shape::dashed_line(&[u(-0.25, 0.0), u(0.25, 0.0)], Stroke::new(1.2_f32, ACCENT), 2.0, 2.0));
        }
        "fillet" => {
            let mut pts = vec![u(-0.7, 0.7), u(-0.7, -0.1)];
            for k in 0..=10 {
                let a = std::f32::consts::PI + std::f32::consts::FRAC_PI_2 * k as f32 / 10.0;
                pts.push(u(-0.1 + 0.6 * a.cos(), -0.1 + 0.6 * a.sin()));
            }
            pts.push(u(0.7, -0.7));
            p.add(Shape::line(pts, s));
        }
        "texture_tp" => {
            for (x, y, a) in [(-0.45, -0.4, 0.3f32), (0.3, -0.5, -0.2), (-0.1, 0.05, 0.1), (0.5, 0.25, 0.4), (-0.55, 0.45, -0.3), (0.1, 0.6, 0.2)] {
                let d = vec2(a.cos(), a.sin()) * w * 0.18;
                p.line_segment([u(x, y) - d, u(x, y) + d], Stroke::new(2.0_f32, ACCENT));
            }
        }
        _ => {
            p.circle_stroke(cx, w * 0.3, s);
        }
    }
}

/// A square tool tile with an icon and a short caption.
pub fn tile(ui: &mut Ui, dark: bool, icon: &str, label: &str, selected: bool, tip: &str) -> Response {
    let pal = theme::palette(dark);
    let size = vec2(60.0, 56.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    if ui.is_rect_visible(rect) {
        let bg = if selected {
            ACCENT.gamma_multiply(if dark { 0.35 } else { 0.22 })
        } else if resp.hovered() {
            pal.tile_hover
        } else {
            pal.tile
        };
        let p = ui.painter();
        p.rect_filled(rect, CornerRadius::same(8), bg);
        if selected {
            p.rect_stroke(rect, CornerRadius::same(8), Stroke::new(1.5_f32, ACCENT), egui::StrokeKind::Inside);
        }
        let col = if selected { if dark { Color32::WHITE } else { ACCENT } } else { pal.text };
        let ir = Rect::from_center_size(rect.center() - vec2(0.0, 8.0), vec2(26.0, 22.0));
        draw(p, ir, col, icon);
        p.text(pos2(rect.center().x, rect.bottom() - 9.0), Align2::CENTER_CENTER, label, FontId::proportional(10.5), pal.weak);
    }
    resp.on_hover_text(tip)
}

/// A compact icon-only tile (the original's dense tool grids); the name shows on hover.
pub fn icon_tile(ui: &mut Ui, dark: bool, icon: &str, selected: bool, tip: &str) -> Response {
    let pal = theme::palette(dark);
    let (rect, resp) = ui.allocate_exact_size(vec2(40.0, 38.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let enabled = ui.is_enabled();
        let bg = if selected {
            ACCENT.gamma_multiply(if dark { 0.35 } else { 0.22 })
        } else if resp.hovered() {
            pal.tile_hover
        } else {
            Color32::TRANSPARENT
        };
        let p = ui.painter();
        p.rect_filled(rect, CornerRadius::same(7), bg);
        if selected {
            p.rect_stroke(rect, CornerRadius::same(7), Stroke::new(1.5_f32, ACCENT), egui::StrokeKind::Inside);
        }
        let col = if selected {
            if dark { Color32::WHITE } else { ACCENT }
        } else if !enabled {
            pal.weak.gamma_multiply(0.6)
        } else {
            pal.text
        };
        draw(p, Rect::from_center_size(rect.center(), vec2(24.0, 22.0)), col, icon);
    }
    resp.on_hover_text(tip)
}

/// Shades of the app's blue for the pictures: light top, mid front, dark side.
pub const BLUE_TOP: Color32 = Color32::from_rgb(120, 178, 255);
pub const BLUE_FRONT: Color32 = Color32::from_rgb(61, 139, 253);
pub const BLUE_SIDE: Color32 = Color32::from_rgb(34, 92, 196);
const GLOW: Color32 = Color32::from_rgb(255, 196, 64);

/// A rounded isometric slab: flat-shaded blue faces with a soft white rim on the top.
fn slab(p: &Painter, c: Pos2, w: f32, d: f32, h: f32, top: Color32) -> [Pos2; 4] {
    // isometric axes: x goes right-down, y goes right-up
    let ax = vec2(0.866, 0.5) * w * 0.5;
    let ay = vec2(0.866, -0.5) * d * 0.5;
    let up = vec2(0.0, -h);
    let t = [c - ax - ay + up, c + ax - ay + up, c + ax + ay + up, c - ax + ay + up];
    let b = [c - ax - ay, c + ax - ay, c + ax + ay];
    p.add(Shape::convex_polygon(vec![b[0], b[1], t[1], t[0]], BLUE_FRONT, Stroke::NONE));
    p.add(Shape::convex_polygon(vec![b[1], b[2], t[2], t[1]], BLUE_SIDE, Stroke::NONE));
    p.add(Shape::convex_polygon(t.to_vec(), top, Stroke::new(1.0_f32, Color32::from_white_alpha(140))));
    t
}

fn arrow_line(p: &Painter, a: Pos2, b: Pos2, col: Color32) {
    let s = Stroke::new(1.3_f32, col);
    p.line_segment([a, b], s);
    let d = (b - a).normalized() * 4.0;
    for (q, dir) in [(a, d), (b, -d)] {
        p.line_segment([q, q + dir + dir.rot90() * 0.7], s);
        p.line_segment([q, q + dir - dir.rot90() * 0.7], s);
    }
}

/// Pictures for the Job Setup sections, in CarveCraft's own blue style: "type", "size",
/// "zzero" (on = machine bed), "datum", "resolution".
pub fn job_art(p: &Painter, r: Rect, which: &str, on: bool) {
    // every picture sits on a soft rounded blue tile
    p.rect_filled(r.shrink(1.0), CornerRadius::same(10), BLUE_FRONT.gamma_multiply(0.14));
    let c = r.center() + vec2(-2.0, 8.0);
    let ink = Color32::from_rgb(170, 200, 245);
    let font = FontId::proportional(9.0);
    match which {
        "type" => {
            // one slab with a downward arrow: cut from the top side
            slab(p, c + vec2(0.0, 4.0), 30.0, 22.0, 9.0, BLUE_TOP);
            let a = c + vec2(0.0, -24.0);
            p.line_segment([a, a + vec2(0.0, 12.0)], Stroke::new(2.0_f32, GLOW));
            p.add(Shape::convex_polygon(vec![a + vec2(-4.0, 10.0), a + vec2(4.0, 10.0), a + vec2(0.0, 15.0)], GLOW, Stroke::NONE));
        }
        "size" => {
            let t = slab(p, c + vec2(-3.0, 2.0), 28.0, 20.0, 16.0, BLUE_TOP);
            // dimension arrows for X, Y and Z
            let x0 = t[0] + vec2(0.0, 16.0 + 5.0);
            arrow_line(p, x0, x0 + (t[1] - t[0]), ink);
            p.text(x0 + (t[1] - t[0]) * 0.5 + vec2(-3.0, 3.0), Align2::CENTER_TOP, "X", font.clone(), ink);
            let y0 = t[1] + vec2(5.0, 16.0 + 2.0);
            arrow_line(p, y0, y0 + (t[2] - t[1]), ink);
            p.text(y0 + (t[2] - t[1]) * 0.5 + vec2(5.0, 2.0), Align2::LEFT_CENTER, "Y", font.clone(), ink);
            let z0 = t[2] + vec2(5.0, 0.0);
            arrow_line(p, z0, z0 + vec2(0.0, 16.0), ink);
            p.text(z0 + vec2(4.0, 8.0), Align2::LEFT_CENTER, "Z", font, ink);
        }
        "zzero" => {
            let t = slab(p, c + vec2(0.0, 2.0), 30.0, 22.0, 14.0, BLUE_TOP);
            // a glowing plane marks where Z0 is: the top face or the bed under the slab
            let plane: Vec<Pos2> = if on { t.iter().map(|q| *q + vec2(0.0, 14.0)).collect() } else { t.to_vec() };
            p.add(Shape::closed_line(plane.clone(), Stroke::new(2.0_f32, GLOW)));
            // label it on the plane itself, so it never runs off the picture
            let mid = plane.iter().fold(vec2(0.0, 0.0), |a, q| a + q.to_vec2()) / 4.0;
            p.text(mid.to_pos2(), Align2::CENTER_CENTER, "Z0", FontId::proportional(9.5), if on { GLOW } else { BLUE_SIDE });
        }
        "datum" => {
            // seen from above: a sheet with a crosshair at its X0 Y0 corner
            let sheet = Rect::from_center_size(c + vec2(2.0, -6.0), vec2(36.0, 26.0));
            p.rect_filled(sheet, CornerRadius::same(3), BLUE_TOP);
            p.rect_stroke(sheet, CornerRadius::same(3), Stroke::new(1.0_f32, Color32::from_white_alpha(140)), egui::StrokeKind::Inside);
            let o = sheet.left_bottom();
            p.circle_stroke(o, 5.0, Stroke::new(1.6_f32, GLOW));
            p.line_segment([o + vec2(-8.0, 0.0), o + vec2(8.0, 0.0)], Stroke::new(1.3_f32, GLOW));
            p.line_segment([o + vec2(0.0, -8.0), o + vec2(0.0, 8.0)], Stroke::new(1.3_f32, GLOW));
        }
        _ => {
            // resolution: the same curve drawn coarse, then fine
            let base = c + vec2(-24.0, 6.0);
            for (k, (cells, x0)) in [(5, 0.0), (14, 26.0)].iter().enumerate() {
                let w = 22.0 / *cells as f32;
                for i in 0..*cells {
                    let t = (i as f32 + 0.5) / *cells as f32;
                    let h = 4.0 + 18.0 * (t * std::f32::consts::PI).sin();
                    let col = if k == 0 { BLUE_SIDE } else { BLUE_FRONT };
                    p.rect_filled(Rect::from_min_max(base + vec2(x0 + i as f32 * w, -h), base + vec2(x0 + (i + 1) as f32 * w - 0.6, 0.0)), 0.0, col);
                }
            }
        }
    }
}
