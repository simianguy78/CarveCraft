//! Right panel (tools, toolpaths, preview) and the sample project.

use crate::app::{App, Doc};
use crate::geom::{Poly, ShapeKind, Vector};
use crate::icons;
use crate::model::{Combine, Component, Profile, ReliefKind, Source};
use crate::panels::{accent_card, card, num, primary};
use crate::theme;
use crate::toolpath::{Material, Op, Side, Tool, ToolKind, Toolpath};
use egui::{pos2, vec2, Color32, Rect, RichText, Ui};
use std::sync::Arc;

/// Pick a tool from the list and edit its size, feeds and speeds.
fn tool_editor(ui: &mut Ui, pal: &theme::Palette, tools: &mut [Tool], idx: &mut usize) {
    let names: Vec<String> = tools.iter().map(|t| t.name.clone()).collect();
    egui::ComboBox::from_id_salt(ui.id().with("tool")).width(ui.available_width() - 8.0).selected_text(&names[*idx]).show_ui(ui, |ui| {
        for (i, n) in names.iter().enumerate() {
            ui.selectable_value(idx, i, n);
        }
    });
    let t = &mut tools[*idx];
    ui.label(RichText::new(format!("{} · tool #{}", t.kind.label(), t.number)).color(pal.weak));
    egui::Grid::new(ui.id().with("toolgrid")).num_columns(4).spacing(vec2(6.0, 4.0)).show(ui, |ui| {
        ui.label("Ø");
        ui.add(egui::DragValue::new(&mut t.diameter).range(0.1..=50.0).speed(0.05).suffix(" mm"));
        if t.kind == ToolKind::VBit {
            ui.label("Angle");
            ui.add(egui::DragValue::new(&mut t.angle).range(10.0..=150.0).suffix("°"));
        } else {
            ui.label("");
            ui.label("");
        }
        ui.end_row();
        ui.label("Step");
        ui.add(egui::DragValue::new(&mut t.stepover).range(0.05..=50.0).speed(0.02).suffix(" mm"));
        ui.label("Pass");
        ui.add(egui::DragValue::new(&mut t.pass_depth).range(0.1..=50.0).speed(0.05).suffix(" mm"));
        ui.end_row();
        ui.label("Feed");
        ui.add(egui::DragValue::new(&mut t.feed).range(10.0..=20000.0).speed(5.0));
        ui.label("RPM");
        ui.add(egui::DragValue::new(&mut t.rpm).range(1000.0..=40000.0).speed(50.0));
        ui.end_row();
        ui.label("Plunge");
        ui.add(egui::DragValue::new(&mut t.plunge).range(10.0..=20000.0).speed(5.0));
        ui.label("Flutes");
        ui.add(egui::DragValue::new(&mut t.flutes).range(1..=8));
        ui.end_row();
    });
    let chip = t.feed / (t.rpm.max(1.0) * t.flutes.max(1) as f32);
    ui.label(RichText::new(format!("Chip load {chip:.3} mm per tooth")).small().color(pal.weak));
}

impl App {
    pub fn right_panel(&mut self, ui: &mut Ui) {
        let dark = self.dark;
        ui.label(RichText::new("Toolpaths").strong().size(16.0));
        ui.add_space(4.0);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            self.material_setup(ui);
            let pending = self.pending_op.clone();
            let mut pick: Option<(Op, (ToolKind, f32))> = None;
            let mut toggle_tools = false;
            let show_tools = self.show_tool_db;
            let is_on = |op: &Op| pending.as_ref().is_some_and(|p| std::mem::discriminant(p) == std::mem::discriminant(op));
            ui.label(RichText::new("Toolpath Operations").strong().size(13.5));
            ui.add_space(2.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                let ops = [
                    ("profile", Op::profile(6.0, Side::Outside), (ToolKind::EndMill, 6.0), "Profile – cut around or along vectors"),
                    ("pocket", Op::pocket(3.0, true), (ToolKind::EndMill, 6.0), "Pocket – clear out the inside of shapes"),
                    ("drill", Op::Drill { depth: 8.0 }, (ToolKind::EndMill, 6.0), "Drilling – holes at the centre of shapes"),
                    ("vcarve", Op::VCarve { flat_depth: 0.0 }, (ToolKind::VBit, 60.0), "V-Carve – sharp V-grooves, ideal for lettering"),
                    ("engrave", Op::Engrave { depth: 1.0 }, (ToolKind::VBit, 30.0), "Quick engrave – trace vectors at a fixed depth"),
                    ("fluting", Op::Fluting { depth: 3.0 }, (ToolKind::BallNose, 3.0), "Fluting – grooves that ramp in and out"),
                    ("rough3d", Op::Rough3D { allowance: 0.5 }, (ToolKind::EndMill, 6.0), "3D roughing – remove bulk material in steps"),
                    ("finish3d", Op::Finish3D, (ToolKind::BallNose, 3.0), "3D finishing – follow the surface with a ball-nose"),
                    ("texture_tp", Op::Texture { depth: 1.0, length: 12.0 }, (ToolKind::BallNose, 6.0), "Texture – hand-tooled strokes"),
                ];
                for (icon, op, tool, tip) in ops {
                    if icons::icon_tile(ui, dark, icon, is_on(&op), tip).clicked() {
                        pick = Some((op, tool));
                    }
                }
                if icons::icon_tile(ui, dark, "tooldb", show_tools, "Tool database – sizes, feeds and speeds").clicked() {
                    toggle_tools = true;
                }
            });
            ui.add_space(6.0);
            if let Some((op, (kind, size))) = pick {
                self.pending_op = Some(op);
                self.tool_idx = crate::tooldb::pick(&self.tools, kind, size);
            }
            if toggle_tools {
                self.show_tool_db = !self.show_tool_db;
                self.db_sel = self.tool_idx;
            }
            if self.pending_op.is_some() {
                self.op_form(ui);
            }
            self.toolpath_list(ui);
            self.preview_card(ui);
        });
    }

    /// The original's Material Setup strip: thickness, Z zero, XY datum and a Set button.
    fn material_setup(&mut self, ui: &mut Ui) {
        let dark = self.dark;
        let pal = theme::palette(dark);
        let m = self.doc.material.clone();
        let mut set = false;
        card(ui, dark, "Material Setup", |ui| {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(46.0, 40.0), egui::Sense::hover());
                let p = ui.painter();
                let wood = Color32::from_rgb(214, 170, 122);
                let top = [r.left_top() + vec2(4.0, 12.0), r.left_top() + vec2(26.0, 4.0), r.right_top() + vec2(-2.0, 10.0), r.left_top() + vec2(22.0, 18.0)];
                let drop = vec2(0.0, 14.0);
                p.add(egui::Shape::convex_polygon(vec![top[0], top[3], top[3] + drop, top[0] + drop], wood.gamma_multiply(0.8), egui::Stroke::NONE));
                p.add(egui::Shape::convex_polygon(vec![top[3], top[2], top[2] + drop, top[3] + drop], wood.gamma_multiply(0.6), egui::Stroke::NONE));
                p.add(egui::Shape::convex_polygon(top.to_vec(), wood, egui::Stroke::NONE));
                // The red outline marks where Z zero is: the top face or the bed.
                let zero: Vec<_> = if m.z_bed { top.iter().map(|q| *q + drop).collect() } else { top.to_vec() };
                p.add(egui::Shape::closed_line(zero, egui::Stroke::new(1.5_f32, Color32::from_rgb(235, 87, 87))));
                ui.vertical(|ui| {
                    ui.label(RichText::new(format!("Thickness {} mm", m.thickness)).strong());
                    let z = if m.z_bed { "Z zero: machine bed" } else { "Z zero: material top" };
                    ui.label(RichText::new(z).small().color(pal.weak));
                    let datum = match m.origin {
                        crate::toolpath::Origin::BottomLeft => "XY datum: bottom-left",
                        crate::toolpath::Origin::Center => "XY datum: centre",
                    };
                    ui.label(RichText::new(format!("{datum} · safe Z {} mm", m.safe_z)).small().color(pal.weak));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Set…").clicked() {
                        set = true;
                    }
                });
            });
        });
        if set {
            self.left_tab = crate::app::LeftTab::Design;
            self.form = Some(crate::app::Form::Material);
        }
    }

    fn op_form(&mut self, ui: &mut Ui) {
        let dark = self.dark;
        let pal = theme::palette(dark);
        let nsel = self.sel.len();
        let mut open_db = false;
        let mut calc = false;
        let mut cancel = false;
        let Some(op) = &mut self.pending_op else { return };
        accent_card(ui, dark, |ui| {
            ui.label(RichText::new(format!("{} toolpath", op.label())).strong().size(15.0));
            match op {
                Op::Profile { depth, side, start, allowance, climb, tabs, ramp } => {
                    ui.label(RichText::new("Cutting depths").strong());
                    num(ui, "Start depth", start, 0.0..=200.0, " mm");
                    num(ui, "Cut depth", depth, 0.1..=200.0, " mm");
                    ui.label(RichText::new("Machine vectors").strong());
                    ui.horizontal(|ui| {
                        ui.radio_value(side, Side::Outside, "Outside");
                        ui.radio_value(side, Side::Inside, "Inside");
                        ui.radio_value(side, Side::On, "On");
                    });
                    ui.horizontal(|ui| {
                        ui.radio_value(climb, true, "Climb");
                        ui.radio_value(climb, false, "Conventional");
                    });
                    num(ui, "Allowance", allowance, -20.0..=20.0, " mm").on_hover_text("Leave this much extra material on the wall for a finishing pass");
                    ui.horizontal(|ui| {
                        let mut on = *ramp > 0.0;
                        if ui.checkbox(&mut on, "Ramp plunge moves").changed() {
                            *ramp = if on { 10.0 } else { 0.0 };
                        }
                        if on {
                            ui.add(egui::DragValue::new(ramp).range(0.5..=200.0).speed(0.2).suffix(" mm"));
                        }
                    });
                    ui.checkbox(&mut tabs.on, "Add tabs");
                    if tabs.on {
                        egui::Grid::new("tabs").num_columns(2).show(ui, |ui| {
                            ui.label("Number");
                            ui.add(egui::DragValue::new(&mut tabs.count).range(1..=50));
                            ui.end_row();
                            ui.label("Length");
                            ui.add(egui::DragValue::new(&mut tabs.length).range(1.0..=100.0).speed(0.1).suffix(" mm"));
                            ui.end_row();
                            ui.label("Thickness");
                            ui.add(egui::DragValue::new(&mut tabs.thickness).range(0.2..=50.0).speed(0.1).suffix(" mm"));
                            ui.end_row();
                        });
                    }
                }
                Op::Pocket { depth, raster, start, allowance, climb } => {
                    ui.label(RichText::new("Cutting depths").strong());
                    num(ui, "Start depth", start, 0.0..=200.0, " mm");
                    num(ui, "Cut depth", depth, 0.1..=200.0, " mm");
                    ui.horizontal(|ui| {
                        ui.radio_value(raster, false, "Offset");
                        ui.radio_value(raster, true, "Raster");
                    });
                    ui.horizontal(|ui| {
                        ui.radio_value(climb, true, "Climb");
                        ui.radio_value(climb, false, "Conventional");
                    });
                    num(ui, "Pocket allowance", allowance, -20.0..=20.0, " mm");
                }
                Op::Drill { depth } => {
                    num(ui, "Hole depth", depth, 0.1..=200.0, " mm");
                }
                Op::VCarve { flat_depth } => {
                    num(ui, "Flat depth (0 = none)", flat_depth, 0.0..=50.0, " mm");
                }
                Op::Engrave { depth } | Op::Fluting { depth } => {
                    num(ui, "Depth", depth, 0.05..=50.0, " mm");
                }
                Op::Rough3D { allowance } => {
                    num(ui, "Leave allowance", allowance, 0.0..=5.0, " mm");
                }
                Op::Finish3D => {
                    ui.label(RichText::new("Raster along X, following the model surface.").color(pal.weak));
                }
                Op::Texture { depth, length } => {
                    num(ui, "Max depth", depth, 0.1..=10.0, " mm");
                    num(ui, "Stroke length", length, 1.0..=100.0, " mm");
                }
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Tool").strong());
                if ui.small_button("Tool database…").on_hover_text("Pick or edit tools, feeds and speeds per material").clicked() {
                    open_db = true;
                }
            });
            tool_editor(ui, &pal, &mut self.tools, &mut self.tool_idx);
            if op.needs_vectors() {
                let msg = if nsel == 0 { "Select vectors in the 2D view first".to_string() } else { format!("{nsel} vector(s) selected") };
                ui.label(RichText::new(msg).color(if nsel == 0 { Color32::from_rgb(240, 160, 60) } else { pal.weak }));
            } else if matches!(op, Op::Texture { .. }) {
                ui.label(RichText::new("Uses selected closed vectors as the area, or the whole job.").small().color(pal.weak));
            }
            ui.horizontal(|ui| {
                if primary(ui, "Calculate").clicked() {
                    calc = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        });
        if open_db {
            self.db_sel = self.tool_idx;
            self.show_tool_db = true;
        }
        if calc {
            self.calculate_pending();
        }
        if cancel {
            self.pending_op = None;
        }
    }

    fn toolpath_list(&mut self, ui: &mut Ui) {
        let dark = self.dark;
        let pal = theme::palette(dark);
        let mut action: Option<(&str, usize)> = None;
        let mut vis_changed = false;
        let mut sel_tp = self.sel_tp;
        let n = self.doc.toolpaths.len();
        card(ui, dark, "Toolpaths", |ui| {
            if n > 1 {
                if let Some(i) = sel_tp.filter(|i| *i < n) {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Order").small().color(pal.weak));
                        if ui.add_enabled(i > 0, egui::Button::new("▲").small()).on_hover_text("Cut earlier").clicked() {
                            action = Some(("up", i));
                        }
                        if ui.add_enabled(i + 1 < n, egui::Button::new("▼").small()).on_hover_text("Cut later").clicked() {
                            action = Some(("down", i));
                        }
                    });
                }
            }
            if self.doc.toolpaths.is_empty() {
                ui.add_space(18.0);
                ui.vertical_centered(|ui| ui.label(RichText::new("No Toolpaths Created Yet").size(16.0).color(pal.weak)));
                ui.add_space(18.0);
            }
            for (i, t) in self.doc.toolpaths.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    if ui.checkbox(&mut t.visible, "").changed() {
                        vis_changed = true;
                    }
                    if ui.selectable_label(sel_tp == Some(i), &t.name).clicked() {
                        sel_tp = Some(i);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(format!("{:.0} min", t.minutes.max(1.0))).small().color(pal.weak));
                    });
                });
            }
            if let Some(i) = sel_tp.filter(|i| *i < self.doc.toolpaths.len()) {
                let t = &self.doc.toolpaths[i];
                ui.separator();
                ui.label(RichText::new(format!("{} · {}", t.op.label(), t.tool.name)).color(pal.weak));
                ui.label(RichText::new(format!("{:.1} m of cutting, {} moves", t.length / 1000.0, t.chains.len())).small().color(pal.weak));
                ui.horizontal(|ui| {
                    if ui.button("Recalculate").clicked() {
                        action = Some(("recalc", i));
                    }
                    if ui.button("Save G-code").clicked() {
                        action = Some(("save", i));
                    }
                    if ui.button("🗑").on_hover_text("Delete toolpath").clicked() {
                        action = Some(("delete", i));
                    }
                });
            }
        });
        self.sel_tp = sel_tp;
        if vis_changed {
            self.invalidate_sim();
        }
        if let Some((a, i)) = action {
            match a {
                "recalc" => {
                    self.push_undo();
                    let (m, mat) = (&self.doc.model, &self.doc.material);
                    self.doc.toolpaths[i].calculate(m, mat);
                    self.invalidate_sim();
                }
                "up" | "down" => {
                    let j = if a == "up" { i - 1 } else { i + 1 };
                    self.push_undo();
                    self.doc.toolpaths.swap(i, j);
                    self.sel_tp = Some(j);
                    self.invalidate_sim();
                }
                "save" => self.export_gcode(Some(i)),
                "delete" => {
                    self.push_undo();
                    self.doc.toolpaths.remove(i);
                    self.sel_tp = None;
                    self.invalidate_sim();
                }
                _ => {}
            }
        }
    }

    fn preview_card(&mut self, ui: &mut Ui) {
        let dark = self.dark;
        let pal = theme::palette(dark);
        let total: f32 = self.doc.toolpaths.iter().filter(|t| t.visible).map(|t| t.minutes).sum();
        let mut cmd: Option<&str> = None;
        card(ui, dark, "Preview & output", |ui| {
            ui.label(RichText::new(format!("Ticked toolpaths: about {total:.0} min of machining")).color(pal.weak));
            ui.horizontal(|ui| {
                if primary(ui, "▶ Simulate").on_hover_text("Watch the material being cut").clicked() {
                    cmd = Some("animate");
                }
                if ui.button("Instant").clicked() {
                    cmd = Some("instant");
                }
                if ui.button("Reset").clicked() {
                    cmd = Some("reset");
                }
            });
            ui.add(egui::Slider::new(&mut self.sim_speed, 1.0..=10.0).show_value(false).text("Speed"));
            if let Some(sim) = &self.sim {
                ui.add(egui::ProgressBar::new(sim.progress()).show_percentage());
                if self.sim_running && ui.button("⏸ Pause").clicked() {
                    self.sim_running = false;
                } else if !self.sim_running && !sim.done && ui.button("▶ Resume").clicked() {
                    self.sim_running = true;
                }
                ui.checkbox(&mut self.show_sim, "Show cut material in 3D view");
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Machine");
                egui::ComboBox::from_id_salt("post").width(170.0).selected_text(self.doc.material.post.label()).show_ui(ui, |ui| {
                    for p in crate::cam::Post::ALL {
                        ui.selectable_value(&mut self.doc.material.post, p, p.label());
                    }
                });
            });
            if ui.add(egui::Button::new("💾 Save Toolpaths…").min_size(vec2(ui.available_width(), 28.0))).clicked() {
                cmd = Some("save");
            }
        });
        match cmd {
            Some("animate") => self.start_sim(true),
            Some("instant") => self.start_sim(false),
            Some("reset") => {
                self.invalidate_sim();
                self.show_sim = false;
            }
            Some("save") => self.export_gcode(None),
            _ => {}
        }
    }
}

// ---------------------------------------------------------------- sample

pub fn sample_doc() -> Doc {
    let mut d = Doc::new(Material::default());
    d.name = "Welcome plaque".into();
    let c = pos2(150.0, 100.0);
    let add = |d: &mut Doc, kind: ShapeKind, at: egui::Pos2| -> Vector {
        let id = d.id();
        let v = Vector::new(id, kind, at, 0);
        d.vectors.push(v.clone());
        v
    };
    let plate = add(&mut d, ShapeKind::Rect { w: 284.0, h: 184.0, r: 16.0 }, c);
    let rope = add(&mut d, ShapeKind::Rect { w: 262.0, h: 162.0, r: 12.0 }, c);
    let oval = add(&mut d, ShapeKind::Ellipse { rx: 84.0, ry: 52.0 }, pos2(150.0, 112.0));
    let text = add(&mut d, ShapeKind::Text { text: "WELCOME".into(), size: 18.0 }, pos2(150.0, 38.0));
    let s1 = add(&mut d, ShapeKind::Star { r1: 13.0, r2: 6.0, points: 5 }, pos2(40.0, 112.0));
    let s2 = add(&mut d, ShapeKind::Star { r1: 13.0, r2: 6.0, points: 5 }, pos2(260.0, 112.0));
    let push = |d: &mut Doc, name: &str, src: Source, mode: Combine| {
        let id = d.id();
        d.model.components.push(Component::new(id, name, src, mode));
    };
    push(&mut d, "Plaque", Source::Shape { polys: plate.polys(), profile: Profile::Angle, height: 3.0, angle: 35.0, base: 0.0, limit: true }, Combine::Add);
    push(&mut d, "Rope border", Source::Extrude { polys: rope.polys(), profile: Profile::Round, width: 7.0, height: 3.0 }, Combine::Add);
    push(&mut d, "Oval dome", Source::Shape { polys: oval.polys(), profile: Profile::Round, height: 4.0, angle: 45.0, base: 0.0, limit: false }, Combine::Add);
    push(&mut d, "Rosette", Source::Relief { kind: ReliefKind::Rosette, rect: Rect::from_center_size(pos2(150.0, 112.0), vec2(86.0, 86.0)), height: 6.0 }, Combine::Add);
    push(&mut d, "Lettering", Source::Shape { polys: text.polys(), profile: Profile::Angle, height: 2.5, angle: 40.0, base: 0.0, limit: true }, Combine::Add);
    let mut stars: Vec<Poly> = s1.polys();
    stars.extend(s2.polys());
    push(&mut d, "Stars", Source::Shape { polys: stars, profile: Profile::Angle, height: 3.0, angle: 30.0, base: 0.0, limit: true }, Combine::Add);
    d.model.rebuild();
    let tools = Tool::defaults();
    let tp = |d: &mut Doc, name: &str, op: Op, tool: &Tool, polys: Vec<Poly>, visible: bool| {
        let id = d.id();
        let mut t = Toolpath { id, name: name.into(), op, tool: tool.clone(), polys, visible, chains: Arc::new(Vec::new()), length: 0.0, minutes: 0.0 };
        t.calculate(&d.model, &d.material);
        d.toolpaths.push(t);
    };
    tp(&mut d, "3D Finishing", Op::Finish3D, &tools[2], Vec::new(), false);
    tp(&mut d, "Profile cut-out", Op::profile(20.0, Side::Outside), &tools[0], plate.polys(), true);
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_3d_paths_gcode_and_save_roundtrip() {
        let mut d = sample_doc();
        let tools = Tool::defaults();
        for (op, tool) in [
            (Op::Rough3D { allowance: 0.5 }, &tools[0]),
            (Op::Finish3D, &tools[2]),
            (Op::Texture { depth: 1.0, length: 12.0 }, &tools[3]),
        ] {
            let ch = crate::toolpath::generate(&op, tool, &[], &d.model, &d.material);
            let pts: usize = ch.iter().map(|c| c.len()).sum();
            eprintln!("{:<12} chains {:5} points {:7}", op.label(), ch.len(), pts);
            assert!(!ch.is_empty(), "{} produced nothing", op.label());
            assert!(ch.iter().flatten().all(|p| p[2] >= -d.material.thickness - 1e-3), "{} cuts below the material", op.label());
        }
        let paths: Vec<&Toolpath> = d.toolpaths.iter().collect();
        let code = crate::toolpath::gcode(&paths, &d.material);
        assert!(code.contains("G21") && code.contains("M30") && code.lines().count() > 100);
        let json = serde_json::to_string(&d).unwrap();
        let mut back: Doc = serde_json::from_str(&json).unwrap();
        back.after_load();
        assert_eq!(back.vectors.len(), d.vectors.len());
        assert_eq!(back.model.components.len(), d.model.components.len());
        assert!((back.model.max_height() - d.model.max_height()).abs() < 1e-3);
        let mut sim = crate::toolpath::Sim::new(&d.material, &paths);
        sim.finish();
        assert!(sim.stock.z.iter().any(|z| *z < -1.0), "simulation removed nothing");
        d.name.clear();
    }
}

