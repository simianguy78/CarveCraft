//! Top bar, status bar, view toolbar and the left (design / 3D model) panel.

use crate::app::{App, DrawTool, Form, LeftTab, ModelColor, View};
use crate::geom::ShapeKind;
use crate::icons;
use crate::model::{Brush, Combine, Profile, ReliefKind, TextureKind};
use crate::render3d::Camera;
use crate::theme::{self, ACCENT};
use crate::toolpath::Origin;
use egui::{pos2, vec2, Color32, CornerRadius, FontId, RichText, Ui};

pub fn card(ui: &mut Ui, dark: bool, title: &str, add: impl FnOnce(&mut Ui)) {
    let pal = theme::palette(dark);
    egui::Frame::new()
        .fill(pal.card)
        .corner_radius(CornerRadius::same(10))
        .stroke(egui::Stroke::new(1.0_f32, pal.border))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(title.to_uppercase()).size(11.0).strong().color(pal.weak));
            ui.add_space(4.0);
            add(ui);
        });
    ui.add_space(8.0);
}

pub fn accent_card(ui: &mut Ui, dark: bool, add: impl FnOnce(&mut Ui)) {
    let pal = theme::palette(dark);
    egui::Frame::new()
        .fill(pal.card)
        .corner_radius(CornerRadius::same(10))
        .stroke(egui::Stroke::new(1.5_f32, ACCENT))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
    ui.add_space(8.0);
}

pub fn segmented<T: PartialEq + Copy>(ui: &mut Ui, dark: bool, value: &mut T, items: &[(T, &str)]) -> bool {
    let pal = theme::palette(dark);
    let mut changed = false;
    egui::Frame::new().fill(pal.tile).corner_radius(CornerRadius::same(8)).inner_margin(egui::Margin::same(3)).show(ui, |ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        ui.horizontal(|ui| {
            let w = (ui.available_width() - 2.0 * (items.len() as f32 - 1.0)) / items.len() as f32;
            for (v, label) in items {
                let on = *value == *v;
                let text = RichText::new(*label).color(if on { Color32::WHITE } else { pal.text });
                let b = egui::Button::new(text).fill(if on { ACCENT } else { Color32::TRANSPARENT }).corner_radius(CornerRadius::same(6)).min_size(vec2(w, 26.0));
                if ui.add(b).clicked() && !on {
                    *value = *v;
                    changed = true;
                }
            }
        });
    });
    changed
}

pub fn num(ui: &mut Ui, label: &str, v: &mut f32, range: std::ops::RangeInclusive<f32>, suffix: &str) -> egui::Response {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| ui.add(egui::DragValue::new(v).range(range).speed(0.1).max_decimals(2).suffix(suffix))).inner
    })
    .inner
}

pub fn numu(ui: &mut Ui, label: &str, v: &mut u32, range: std::ops::RangeInclusive<u32>) -> egui::Response {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| ui.add(egui::DragValue::new(v).range(range).speed(0.1))).inner
    })
    .inner
}

pub fn primary(ui: &mut Ui, text: &str) -> egui::Response {
    ui.add(egui::Button::new(RichText::new(text).color(Color32::WHITE).strong()).fill(ACCENT).min_size(vec2(110.0, 28.0)))
}

/// A titled group of compact icon tiles, laid out like the original's Design panel.
fn section(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui)) {
    ui.add_space(6.0);
    ui.label(RichText::new(title).strong().size(13.5));
    ui.add_space(2.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
        add(ui);
    });
    ui.add_space(4.0);
}

fn tiles(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
        add(ui);
    });
}

impl App {
    pub fn top_bar(&mut self, ctx: &egui::Context) {
        let pal = theme::palette(self.dark);
        egui::TopBottomPanel::top("top")
            .frame(egui::Frame::new().fill(ctx.style().visuals.panel_fill).inner_margin(egui::Margin::symmetric(12, 6)).stroke(egui::Stroke::new(1.0_f32, pal.border)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(26.0, 26.0), egui::Sense::hover());
                    ui.painter().rect_filled(r, CornerRadius::same(7), ACCENT);
                    ui.painter().add(egui::Shape::convex_polygon(
                        vec![r.left_top() + vec2(6.0, 8.0), r.right_top() + vec2(-6.0, 8.0), r.center_bottom() + vec2(0.0, -6.0)],
                        Color32::WHITE,
                        egui::Stroke::NONE,
                    ));
                    ui.label(RichText::new("CarveCraft").strong().size(16.0));
                    ui.label(RichText::new("Studio").size(16.0).color(pal.weak));
                    ui.add_space(14.0);
                    egui::menu::bar(ui, |ui| self.menus(ui));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let icon = if self.dark { "☀ Light" } else { "🌙 Dark" };
                        if ui.button(icon).on_hover_text("Switch theme").clicked() {
                            self.dark = !self.dark;
                        }
                        if ui.add_enabled(!self.redo.is_empty(), egui::Button::new("↪ Redo")).clicked() {
                            self.do_redo();
                        }
                        if ui.add_enabled(!self.undo.is_empty(), egui::Button::new("↩ Undo")).clicked() {
                            self.do_undo();
                        }
                        if ui.button("💾 Save").clicked() {
                            self.save(false);
                        }
                        ui.add_space(10.0);
                        ui.label(RichText::new(&self.doc.name).color(pal.weak));
                    });
                });
            });
    }

    fn menus(&mut self, ui: &mut Ui) {
        ui.menu_button("File", |ui| {
            if ui.button("New job…").clicked() {
                self.new_doc();
                ui.close_menu();
            }
            if ui.button("Open…   Ctrl+O").clicked() {
                self.open_dialog();
                ui.close_menu();
            }
            if ui.button("Save   Ctrl+S").clicked() {
                self.save(false);
                ui.close_menu();
            }
            if ui.button("Save as…").clicked() {
                self.save(true);
                ui.close_menu();
            }
            ui.separator();
            if ui.button("Import height map (PNG)…").clicked() {
                self.left_tab = LeftTab::Model;
                self.form = Some(Form::ImportHeight);
                ui.close_menu();
            }
            if ui.button("Export 3D model (STL)…").clicked() {
                self.export_stl();
                ui.close_menu();
            }
            if ui.button("Save toolpaths as G-code…").clicked() {
                self.export_gcode(None);
                ui.close_menu();
            }
            ui.separator();
            if ui.button("Start screen").clicked() {
                self.home = true;
                ui.close_menu();
            }
            if ui.button("Load sample project").clicked() {
                self.push_undo();
                self.doc = crate::panels_right::sample_doc();
                self.after_doc_swap();
                ui.close_menu();
            }
            if ui.button("Exit").clicked() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
        ui.menu_button("Edit", |ui| {
            if ui.add_enabled(!self.undo.is_empty(), egui::Button::new("Undo   Ctrl+Z")).clicked() {
                self.do_undo();
                ui.close_menu();
            }
            if ui.add_enabled(!self.redo.is_empty(), egui::Button::new("Redo   Ctrl+Y")).clicked() {
                self.do_redo();
                ui.close_menu();
            }
            ui.separator();
            if ui.button("Select all   Ctrl+A").clicked() {
                self.sel = self.doc.vectors.iter().map(|v| v.id).collect();
                ui.close_menu();
            }
            if ui.button("Duplicate   Ctrl+D").clicked() {
                self.duplicate_selected(vec2(10.0, -10.0));
                ui.close_menu();
            }
            if ui.button("Delete   Del").clicked() {
                self.delete_selected();
                ui.close_menu();
            }
        });
        ui.menu_button("Model", |ui| {
            for (label, form, prof) in [
                ("Create shape…", Form::Shape, None),
                ("Create prism…", Form::Shape, Some(Profile::Angle)),
                ("Extrude along vector…", Form::Extrude, None),
                ("Two-rail sweep…", Form::TwoRail, None),
                ("Texture area…", Form::Texture, None),
                ("Relief library…", Form::Relief, None),
                ("Import height map…", Form::ImportHeight, None),
            ] {
                if ui.button(label).clicked() {
                    self.left_tab = LeftTab::Model;
                    self.form = Some(form);
                    if let Some(p) = prof {
                        self.p.profile = p;
                        self.p.limit = true;
                    } else if form == Form::Shape {
                        self.p.profile = Profile::Round;
                    }
                    ui.close_menu();
                }
            }
            ui.separator();
            if ui.button("Bake model").clicked() && !self.doc.model.components.is_empty() {
                self.push_undo();
                let id = self.doc.id();
                self.doc.model.bake(id);
                self.invalidate_sim();
                ui.close_menu();
            }
            if ui.button("Export model as STL…").clicked() {
                self.export_stl();
                ui.close_menu();
            }
        });
        ui.menu_button("Machine", |ui| {
            if ui.button("Job setup…").clicked() {
                self.left_tab = LeftTab::Design;
                self.form = Some(Form::Material);
                ui.close_menu();
            }
            ui.menu_button("Tool", |ui| {
                for i in 0..self.tools.len() {
                    let name = self.tools[i].name.clone();
                    ui.selectable_value(&mut self.tool_idx, i, name);
                }
            });
        });
        ui.menu_button("Toolpaths", |ui| {
            for (label, op, tool) in [
                ("Profile", crate::toolpath::Op::profile(6.0, crate::toolpath::Side::Outside), 0usize),
                ("Pocket", crate::toolpath::Op::pocket(3.0, true), 0),
                ("Drilling", crate::toolpath::Op::Drill { depth: 8.0 }, 0),
                ("V-Carve", crate::toolpath::Op::VCarve { flat_depth: 0.0 }, 4),
                ("Quick engrave", crate::toolpath::Op::Engrave { depth: 1.0 }, 6),
                ("Fluting", crate::toolpath::Op::Fluting { depth: 3.0 }, 2),
                ("3D roughing", crate::toolpath::Op::Rough3D { allowance: 0.5 }, 0),
                ("3D finishing", crate::toolpath::Op::Finish3D, 2),
                ("Texture", crate::toolpath::Op::Texture { depth: 1.0, length: 12.0 }, 3),
            ] {
                if ui.button(label).clicked() {
                    self.pending_op = Some(op);
                    self.tool_idx = tool;
                    ui.close_menu();
                }
            }
            ui.separator();
            if ui.button("Simulate all").clicked() {
                self.start_sim(true);
                ui.close_menu();
            }
            if ui.button("Save toolpaths as G-code…").clicked() {
                self.export_gcode(None);
                ui.close_menu();
            }
        });
        ui.menu_button("View", |ui| {
            if ui.button("2D design view   F2").clicked() {
                self.view = View::D2;
                ui.close_menu();
            }
            if ui.button("3D view   F3").clicked() {
                self.view = View::D3;
                ui.close_menu();
            }
            ui.checkbox(&mut self.show_grid, "Show grid");
            ui.checkbox(&mut self.snap, "Snap to 5 mm");
            ui.checkbox(&mut self.show_paths, "Show toolpaths");
            ui.checkbox(&mut self.show_model2d, "Show 3D model in 2D view");
            ui.separator();
            ui.checkbox(&mut self.dark, "Dark theme");
        });
        ui.menu_button("Help", |ui| {
            ui.label("1. Draw or type vectors in the Design tab.");
            ui.label("2. Turn them into 3D shapes in the 3D Model tab.");
            ui.label("3. Pick a tool and toolpath on the right, then Simulate.");
            ui.label("4. Save the toolpaths as G-code for your machine.");
            ui.separator();
            ui.label("Wheel = zoom · right-drag = pan · left-drag in 3D = orbit");
            ui.separator();
            if ui.button("Quick guide").clicked() {
                self.show_guide = true;
                ui.close_menu();
            }
            ui.hyperlink_to("CarveCraft on GitHub", crate::start::GITHUB_URL);
            ui.hyperlink_to("Report a problem", format!("{}/issues", crate::start::GITHUB_URL));
        });
    }

    pub fn status_bar(&mut self, ctx: &egui::Context) {
        let pal = theme::palette(self.dark);
        egui::TopBottomPanel::bottom("status")
            .frame(egui::Frame::new().fill(ctx.style().visuals.panel_fill).inner_margin(egui::Margin::symmetric(12, 4)).stroke(egui::Stroke::new(1.0_f32, pal.border)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&self.status).color(pal.text));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new("mm").color(pal.weak));
                        ui.separator();
                        if let Some(b) = self.selection_bounds() {
                            let layer = self
                                .selected_vectors()
                                .first()
                                .and_then(|v| self.doc.layers.get(v.layer))
                                .map(|l| l.name.clone())
                                .unwrap_or_default();
                            ui.label(RichText::new(format!("W {:.2}  H {:.2}  {layer}", b.width(), b.height())).monospace().color(pal.weak));
                            ui.separator();
                        }
                        if let Some(c) = self.cursor {
                            ui.label(RichText::new(format!("X {:7.2}  Y {:7.2}", c.x, c.y)).monospace().color(pal.weak));
                            ui.separator();
                        }
                        let m = &self.doc.material;
                        ui.label(RichText::new(format!("Material {} × {} × {} mm", m.w, m.h, m.thickness)).color(pal.weak));
                    });
                });
            });
    }

    pub fn view_toolbar(&mut self, ui: &mut Ui) {
        let pal = theme::palette(self.dark);
        egui::Frame::new().inner_margin(egui::Margin::symmetric(10, 6)).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.allocate_ui(vec2(220.0, 32.0), |ui| {
                    segmented(ui, self.dark, &mut self.view, &[(View::D2, "2D View"), (View::D3, "3D View")]);
                });
                ui.add_space(8.0);
                match self.view {
                    View::D2 => {
                        if ui.button("⛶ Fit").on_hover_text("Zoom to the material").clicked() {
                            self.fit_2d = true;
                        }
                        ui.toggle_value(&mut self.show_grid, "Grid");
                        ui.toggle_value(&mut self.snap, "Snap");
                        ui.toggle_value(&mut self.show_model2d, "Model");
                        ui.toggle_value(&mut self.show_paths, "Toolpaths");
                        ui.toggle_value(&mut self.show_dims, "Dimensions").on_hover_text("Show lengths and angles while drawing and editing points");
                        // Active layer picker, like the original's Layer drop-down.
                        ui.add_space(8.0);
                        ui.label(RichText::new("Layer").color(pal.weak));
                        let names: Vec<String> = self.doc.layers.iter().map(|l| l.name.clone()).collect();
                        let cur = self.active_layer.min(names.len().saturating_sub(1));
                        egui::ComboBox::from_id_salt("layerpick").width(90.0).selected_text(names.get(cur).cloned().unwrap_or_default()).show_ui(ui, |ui| {
                            for (i, n) in names.iter().enumerate() {
                                ui.selectable_value(&mut self.active_layer, i, n);
                            }
                        });
                    }
                    View::D3 => {
                        if ui.button("Iso").clicked() {
                            self.cam = Camera { exaggerate: self.cam.exaggerate, ..Camera::default() };
                        }
                        if ui.button("Top").clicked() {
                            self.cam.pitch = 90.0;
                            self.cam.yaw = 0.0;
                        }
                        if ui.button("Front").clicked() {
                            self.cam.pitch = 18.0;
                            self.cam.yaw = 0.0;
                        }
                        ui.toggle_value(&mut self.show_paths, "Toolpaths");
                        if self.sim.is_some() {
                            ui.toggle_value(&mut self.show_sim, "Cut result");
                        }
                        ui.label(RichText::new("Relief ×").color(pal.weak));
                        ui.add(egui::DragValue::new(&mut self.cam.exaggerate).range(0.5..=5.0).speed(0.02).max_decimals(1));
                        let names = [(ModelColor::Wood, "Wood"), (ModelColor::Gold, "Gold"), (ModelColor::Slate, "Slate")];
                        let cur = names.iter().find(|(c, _)| *c == self.model_color).map(|(_, n)| *n).unwrap_or("Wood");
                        egui::ComboBox::from_id_salt("mcolor").width(80.0).selected_text(cur).show_ui(ui, |ui| {
                            for (c, n) in names {
                                ui.selectable_value(&mut self.model_color, c, n);
                            }
                        });
                    }
                }
            });
        });
    }

    /// The original's vertical tab strip down the left edge, as rounded pills.
    pub fn left_tabs(&mut self, ui: &mut Ui) {
        let pal = theme::palette(self.dark);
        let tabs = [(LeftTab::Design, "Design"), (LeftTab::Model, "Modeling"), (LeftTab::Layers, "Layers"), (LeftTab::Components, "Components")];
        let mut pick = None;
        for (tab, label) in tabs {
            let on = self.left_tab == tab;
            let galley = ui.painter().layout_no_wrap(label.to_string(), FontId::proportional(12.5), if on { Color32::WHITE } else { pal.weak });
            let (rect, resp) = ui.allocate_exact_size(vec2(24.0, galley.size().x + 22.0), egui::Sense::click());
            let fill = if on {
                ACCENT
            } else if resp.hovered() {
                pal.tile_hover
            } else {
                Color32::TRANSPARENT
            };
            ui.painter().rect_filled(rect, CornerRadius::same(7), fill);
            // Rotated a quarter turn so it reads bottom-to-top, like the original's tabs.
            let pos = pos2(rect.center().x - galley.size().y * 0.5, rect.bottom() - 11.0);
            ui.painter().add(egui::epaint::TextShape::new(pos, galley, pal.text).with_angle(-std::f32::consts::FRAC_PI_2));
            if resp.clicked() {
                pick = Some(tab);
            }
            ui.add_space(4.0);
        }
        if let Some(tab) = pick {
            if tab != self.left_tab {
                self.left_tab = tab;
                self.form = None;
                if self.tool == DrawTool::Sculpt {
                    self.tool = DrawTool::Select;
                }
            }
        }
    }

    pub fn left_panel(&mut self, ui: &mut Ui) {
        let title = match self.left_tab {
            LeftTab::Design => "Design",
            LeftTab::Model => "Modeling",
            LeftTab::Layers => "Layers",
            LeftTab::Components => "Components",
        };
        ui.label(RichText::new(title).strong().size(16.0));
        ui.add_space(4.0);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            if let Some(f) = self.form {
                self.form_card(ui, f);
            }
            match self.left_tab {
                LeftTab::Design => self.design_tab(ui),
                LeftTab::Model => self.model_tab(ui),
                LeftTab::Layers => self.layers_card(ui),
                LeftTab::Components => self.components_card(ui),
            }
        });
    }

    fn form_card(&mut self, ui: &mut Ui, f: Form) {
        let dark = self.dark;
        let pal = theme::palette(dark);
        let title = match f {
            Form::Material => "Job setup",
            Form::Offset => "Offset vectors",
            Form::CircularArray => "Circular array",
            Form::GridArray => "Grid array",
            Form::Shape => "Create shape",
            Form::Extrude => "Extrude along vector",
            Form::TwoRail => "Two-rail sweep",
            Form::Texture => "Texture area",
            Form::Relief => "Relief library",
            Form::ImportHeight => "Import height map",
        };
        let mut apply = false;
        let mut close = false;
        let mut material_done = false;
        accent_card(ui, dark, |ui| {
            ui.label(RichText::new(title).strong().size(15.0));
            ui.add_space(4.0);
            match f {
                Form::Material => {
                    let m = &mut self.doc.material;
                    num(ui, "Width (X)", &mut m.w, 10.0..=3000.0, " mm");
                    num(ui, "Height (Y)", &mut m.h, 10.0..=3000.0, " mm");
                    num(ui, "Thickness (Z)", &mut m.thickness, 1.0..=200.0, " mm");
                    num(ui, "Safe Z", &mut m.safe_z, 1.0..=50.0, " mm");
                    ui.horizontal(|ui| {
                        ui.label("XY origin");
                        ui.radio_value(&mut m.origin, Origin::BottomLeft, "Bottom-left");
                        ui.radio_value(&mut m.origin, Origin::Center, "Centre");
                    });
                    ui.horizontal(|ui| {
                        ui.label("Z zero");
                        ui.radio_value(&mut m.z_bed, false, "Material surface");
                        ui.radio_value(&mut m.z_bed, true, "Machine bed");
                    });
                    if primary(ui, "Done").clicked() {
                        material_done = true;
                    }
                    return;
                }
                Form::Offset => {
                    num(ui, "Distance", &mut self.p.offset, 0.1..=200.0, " mm");
                    ui.horizontal(|ui| {
                        ui.radio_value(&mut self.p.outward, true, "Outwards");
                        ui.radio_value(&mut self.p.outward, false, "Inwards");
                    });
                }
                Form::CircularArray => {
                    numu(ui, "Copies (total)", &mut self.p.arr_count, 2..=200);
                    num(ui, "Spread angle", &mut self.p.arr_angle, 10.0..=360.0, "°");
                    ui.label(RichText::new("Copies rotate around the centre of the job.").small().color(pal.weak));
                }
                Form::GridArray => {
                    numu(ui, "Columns", &mut self.p.grid_cols, 1..=50);
                    numu(ui, "Rows", &mut self.p.grid_rows, 1..=50);
                    num(ui, "Gap", &mut self.p.grid_gap, 0.0..=200.0, " mm");
                }
                Form::Shape | Form::Extrude | Form::TwoRail => {
                    ui.horizontal(|ui| {
                        ui.label("Profile");
                        egui::ComboBox::from_id_salt("profile").selected_text(self.p.profile.label()).show_ui(ui, |ui| {
                            for pr in Profile::ALL {
                                ui.selectable_value(&mut self.p.profile, pr, pr.label());
                            }
                        });
                    });
                    if f == Form::Shape && self.p.profile == Profile::Angle {
                        num(ui, "Angle", &mut self.p.angle, 1.0..=89.0, "°");
                        ui.checkbox(&mut self.p.limit, "Limit height");
                        if self.p.limit {
                            num(ui, "Limit to", &mut self.p.height, 0.1..=100.0, " mm");
                        }
                    } else {
                        num(ui, "Height", &mut self.p.height, 0.1..=100.0, " mm");
                    }
                    if f == Form::Shape {
                        num(ui, "Start height", &mut self.p.base, 0.0..=100.0, " mm");
                    }
                    if f == Form::Extrude {
                        num(ui, "Width", &mut self.p.ext_width, 0.5..=200.0, " mm");
                    }
                    let hint = match f {
                        Form::Shape => "Uses the selected closed vectors as the outline.",
                        Form::Extrude => "Sweeps a profile along the selected vectors.",
                        _ => "Select two curves; the shape is swept between them.",
                    };
                    ui.label(RichText::new(hint).small().color(pal.weak));
                }
                Form::Texture => {
                    egui::ComboBox::from_id_salt("tex").selected_text(self.p.tex_kind.label()).show_ui(ui, |ui| {
                        for t in TextureKind::ALL {
                            ui.selectable_value(&mut self.p.tex_kind, t, t.label());
                        }
                    });
                    num(ui, "Depth", &mut self.p.tex_depth, 0.1..=20.0, " mm");
                    num(ui, "Pattern size", &mut self.p.tex_size, 1.0..=100.0, " mm");
                    ui.label(RichText::new("Fills the selected closed vectors, or the whole job.").small().color(pal.weak));
                }
                Form::Relief => {
                    ui.horizontal_wrapped(|ui| {
                        for k in ReliefKind::ALL {
                            ui.selectable_value(&mut self.p.relief, k, k.label());
                        }
                    });
                    num(ui, "Height", &mut self.p.relief_height, 0.5..=50.0, " mm");
                    num(ui, "Size", &mut self.p.relief_size, 5.0..=1000.0, " mm");
                    ui.label(RichText::new("Fits the selection's box, or sits in the job centre.").small().color(pal.weak));
                }
                Form::ImportHeight => {
                    num(ui, "Height", &mut self.p.img_height, 0.1..=100.0, " mm");
                    ui.label(RichText::new("Light pixels are high, dark are low. Fits the selection or the job.").small().color(pal.weak));
                }
            }
            if matches!(f, Form::Shape | Form::Extrude | Form::TwoRail | Form::Texture | Form::Relief | Form::ImportHeight) {
                ui.horizontal(|ui| {
                    ui.label("Combine");
                    for c in Combine::ALL {
                        ui.selectable_value(&mut self.p.mode, c, c.label());
                    }
                });
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let label = if f == Form::ImportHeight { "Choose picture…" } else { "Apply" };
                if primary(ui, label).clicked() {
                    apply = true;
                }
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
        });
        if material_done {
            self.push_undo();
            let (w, h) = (self.doc.material.w, self.doc.material.h);
            self.doc.model.resize(w, h);
            let (mm, mat) = (&self.doc.model, &self.doc.material);
            for t in &mut self.doc.toolpaths {
                t.calculate(mm, mat);
            }
            self.invalidate_sim();
            self.fit_2d = true;
            self.form = None;
        }
        if close {
            self.form = None;
        }
        if apply {
            match f {
                Form::Offset => self.apply_offset(),
                Form::CircularArray => self.apply_circular_array(),
                Form::GridArray => self.apply_grid_array(),
                Form::ImportHeight => self.import_heightmap(),
                other => self.apply_model_form(other),
            }
        }
    }

    fn open_3d_form(&mut self, form: Form, prof: Option<Profile>) {
        self.form = Some(form);
        if let Some(pr) = prof {
            self.p.profile = pr;
            self.p.limit = true;
            self.p.height = 3.0;
        } else if form == Form::Shape && self.p.profile == Profile::Angle {
            self.p.profile = Profile::Round;
        }
    }

    fn design_tab(&mut self, ui: &mut Ui) {
        let dark = self.dark;
        let pal = theme::palette(dark);
        let has_sel = !self.sel.is_empty();
        let mut cmd: Option<&'static str> = None;
        let mut pick_tool: Option<DrawTool> = None;
        let mut open3d: Option<(Form, Option<Profile>)> = None;
        let tool_now = self.tool;
        let (form_now, prof_now) = (self.form, self.p.profile);

        section(ui, "File Operations", |ui| {
            for (id, icon, tip) in [
                ("job", "job", "Job setup – material size, thickness and origin"),
                ("open", "open", "Open a saved job"),
                ("save", "save", "Save the job"),
                ("heightmap", "import", "Import a picture as a 3D height map"),
            ] {
                if icons::icon_tile(ui, dark, icon, form_now == Some(Form::Material) && id == "job", tip).clicked() {
                    cmd = Some(id);
                }
            }
        });
        section(ui, "Create Vectors", |ui| {
            for (t, icon, tip) in [
                (DrawTool::Circle, "circle", "Circle – drag from the centre"),
                (DrawTool::Ellipse, "ellipse", "Ellipse – drag a box"),
                (DrawTool::Rect, "rect", "Rectangle – drag a box"),
                (DrawTool::Polygon, "polygon", "Polygon – drag from the centre"),
                (DrawTool::Star, "star", "Star – drag from the centre"),
                (DrawTool::Polyline, "polyline", "Polyline – click points; double-click or Enter to finish"),
                (DrawTool::Arc, "arc", "Arc – click start, end, then a point on the arc"),
                (DrawTool::Freehand, "freehand", "Freehand – drag to sketch a curve"),
                (DrawTool::Text, "text", "Text – click to place"),
            ] {
                if icons::icon_tile(ui, dark, icon, tool_now == t, tip).clicked() {
                    pick_tool = Some(t);
                }
            }
        });
        section(ui, "Transform Objects", |ui| {
            ui.add_enabled_ui(has_sel, |ui| {
                for (id, tip) in [
                    ("duplicate", "Duplicate the selection (Ctrl+D)"),
                    ("mirror_h", "Mirror left-right"),
                    ("mirror_v", "Mirror top-bottom"),
                    ("rotate", "Rotate the selection by 90°"),
                    ("center", "Move the selection to the middle of the job"),
                ] {
                    if icons::icon_tile(ui, dark, id, false, tip).clicked() {
                        cmd = Some(id);
                    }
                }
            });
        });
        section(ui, "Edit Objects", |ui| {
            for (t, icon, tip) in [
                (DrawTool::Select, "select", "Select and move vectors"),
                (DrawTool::Node, "node", "Node edit – drag points; double-click to add or delete"),
                (DrawTool::Trim, "trim", "Trim – click a piece of a vector to cut it back"),
                (DrawTool::Fillet, "fillet", "Fillet – click a corner to round it"),
            ] {
                if icons::icon_tile(ui, dark, icon, tool_now == t, tip).clicked() {
                    pick_tool = Some(t);
                }
            }
            ui.add_enabled_ui(has_sel, |ui| {
                for (id, tip) in [("weld", "Weld overlapping closed shapes into one outline"), ("delete", "Delete the selection (Del)")] {
                    if icons::icon_tile(ui, dark, id, false, tip).clicked() {
                        cmd = Some(id);
                    }
                }
            });
        });
        section(ui, "Offset and Layout", |ui| {
            ui.add_enabled_ui(has_sel, |ui| {
                for (id, tip) in [
                    ("offset", "Offset – a bigger or smaller copy of the outline"),
                    ("circ_array", "Circular array – copies around the job centre"),
                    ("grid_array", "Grid array – copies in rows and columns"),
                ] {
                    let on = matches!((id, form_now), ("offset", Some(Form::Offset)) | ("circ_array", Some(Form::CircularArray)) | ("grid_array", Some(Form::GridArray)));
                    if icons::icon_tile(ui, dark, id, on, tip).clicked() {
                        cmd = Some(id);
                    }
                }
            });
        });
        section(ui, "3D Model Tools", |ui| {
            for (icon, tip, form, prof) in [
                ("shape", "Create shape – dome, flat or angled, from closed vectors", Form::Shape, None),
                ("prism", "Prism – sharp ridges, great for lettering", Form::Shape, Some(Profile::Angle)),
                ("extrude", "Extrude a profile along a curve", Form::Extrude, None),
                ("two_rail", "Two-rail sweep between two curves", Form::TwoRail, None),
                ("texture", "Texture an area", Form::Texture, None),
                ("relief", "Relief library", Form::Relief, None),
            ] {
                let on = form_now == Some(form) && (form != Form::Shape || (prof == Some(Profile::Angle)) == (prof_now == Profile::Angle));
                if icons::icon_tile(ui, dark, icon, on, tip).clicked() {
                    open3d = Some((form, prof));
                }
            }
        });

        if let Some(t) = pick_tool {
            self.tool = t;
            self.poly_pts.clear();
            if t != DrawTool::Select {
                self.view = View::D2;
            }
        }
        if let Some((f, pr)) = open3d {
            self.open_3d_form(f, pr);
        }
        let (mw, mh) = (self.doc.material.w, self.doc.material.h);
        match cmd {
            Some("job") => self.form = Some(Form::Material),
            Some("open") => self.open_dialog(),
            Some("save") => self.save(false),
            Some("heightmap") => self.form = Some(Form::ImportHeight),
            Some("duplicate") => self.duplicate_selected(vec2(10.0, -10.0)),
            Some("delete") => self.delete_selected(),
            Some("mirror_h") => self.transform_selected(|v, c| {
                v.center.x = 2.0 * c.x - v.center.x;
                v.scale.x = -v.scale.x;
                v.angle = -v.angle;
            }),
            Some("mirror_v") => self.transform_selected(|v, c| {
                v.center.y = 2.0 * c.y - v.center.y;
                v.scale.y = -v.scale.y;
                v.angle = -v.angle;
            }),
            Some("rotate") => self.transform_selected(|v, c| {
                let d = v.center - c;
                v.center = c + vec2(-d.y, d.x);
                v.angle += 90.0;
            }),
            Some("center") => self.transform_selected(move |v, c| {
                v.center += pos2(mw * 0.5, mh * 0.5) - c;
            }),
            Some("offset") => self.form = Some(Form::Offset),
            Some("weld") => self.apply_weld(),
            Some("circ_array") => self.form = Some(Form::CircularArray),
            Some("grid_array") => self.form = Some(Form::GridArray),
            _ => {}
        }

        // Options for the active drawing tool, like the original's tool form.
        let title = match self.tool {
            DrawTool::Rect => Some("Rectangle"),
            DrawTool::Polygon => Some("Polygon"),
            DrawTool::Star => Some("Star"),
            DrawTool::Text => Some("Text"),
            DrawTool::Fillet => Some("Fillet"),
            _ => None,
        };
        if let Some(title) = title {
            ui.add_space(4.0);
            let tool = self.tool;
            card(ui, dark, title, |ui| match tool {
                DrawTool::Rect => {
                    num(ui, "Corner radius", &mut self.p.rect_r, 0.0..=500.0, " mm");
                }
                DrawTool::Polygon => {
                    numu(ui, "Sides", &mut self.p.sides, 3..=64);
                }
                DrawTool::Star => {
                    numu(ui, "Points", &mut self.p.star_points, 3..=64);
                    num(ui, "Inner ratio", &mut self.p.star_ratio, 0.05..=0.95, "");
                }
                DrawTool::Text => {
                    ui.add(egui::TextEdit::singleline(&mut self.p.text).hint_text("Text"));
                    num(ui, "Letter height", &mut self.p.text_size, 2.0..=500.0, " mm");
                }
                _ => {
                    num(ui, "Radius", &mut self.p.fillet_r, 0.1..=500.0, " mm");
                }
            });
            if tool == DrawTool::Fillet && has_sel && ui.button("Round all corners of selection").clicked() {
                self.apply_fillet_all();
            }
        }
        self.properties_card(ui);

        let m = self.doc.material.clone();
        let mut edit_job = false;
        ui.add_space(6.0);
        card(ui, dark, "Job Dimensions", |ui| {
            egui::Grid::new("jobdims").num_columns(2).spacing(vec2(10.0, 2.0)).show(ui, |ui| {
                for (k, v) in [("Width (X)", m.w), ("Height (Y)", m.h), ("Depth (Z)", m.thickness)] {
                    ui.label(RichText::new(k).color(pal.weak));
                    ui.label(RichText::new(format!("{v} mm")).strong());
                    ui.end_row();
                }
            });
            if ui.button("Edit job…").clicked() {
                edit_job = true;
            }
        });
        if edit_job {
            self.form = Some(Form::Material);
        }
    }

    fn properties_card(&mut self, ui: &mut Ui) {
        let dark = self.dark;
        if self.sel.is_empty() {
            return;
        }
        let pal = theme::palette(dark);
        let single = self.sel.len() == 1;
        let id0 = self.sel[0];
        let bounds = self.selection_bounds();
        let count = self.sel.len();
        let mut changed: Option<(usize, crate::geom::Vector)> = None;
        card(ui, dark, if single { "Selection" } else { "Selection (group)" }, |ui| {
            if !single {
                ui.label(format!("{count} vectors selected"));
                if let Some(b) = bounds {
                    ui.label(RichText::new(format!("{:.1} × {:.1} mm", b.width(), b.height())).color(pal.weak));
                }
                return;
            }
            let Some(idx) = self.doc.vectors.iter().position(|v| v.id == id0) else { return };
            let mut v = self.doc.vectors[idx].clone();
            let before = format!("{:?}{:?}{:?}{:?}", v.kind, v.center, v.angle, v.scale);
            ui.label(RichText::new(v.kind.label()).strong());
            num(ui, "Centre X", &mut v.center.x, -5000.0..=5000.0, " mm");
            num(ui, "Centre Y", &mut v.center.y, -5000.0..=5000.0, " mm");
            num(ui, "Rotation", &mut v.angle, -360.0..=360.0, "°");
            match &mut v.kind {
                ShapeKind::Rect { w, h, r } => {
                    num(ui, "Width", w, 0.1..=5000.0, " mm");
                    num(ui, "Height", h, 0.1..=5000.0, " mm");
                    num(ui, "Corner radius", r, 0.0..=2500.0, " mm");
                }
                ShapeKind::Ellipse { rx, ry } => {
                    num(ui, "Radius X", rx, 0.05..=2500.0, " mm");
                    num(ui, "Radius Y", ry, 0.05..=2500.0, " mm");
                }
                ShapeKind::Polygon { r, sides } => {
                    num(ui, "Radius", r, 0.1..=2500.0, " mm");
                    numu(ui, "Sides", sides, 3..=64);
                }
                ShapeKind::Star { r1, r2, points } => {
                    num(ui, "Outer radius", r1, 0.1..=2500.0, " mm");
                    num(ui, "Inner radius", r2, 0.1..=2500.0, " mm");
                    numu(ui, "Points", points, 3..=64);
                }
                ShapeKind::Text { text, size } => {
                    ui.add(egui::TextEdit::singleline(text));
                    num(ui, "Letter height", size, 1.0..=1000.0, " mm");
                }
                ShapeKind::Path { pts, closed } => {
                    ui.label(format!("{} points, {}", pts.len(), if *closed { "closed" } else { "open" }));
                }
            }
            num(ui, "Scale X", &mut v.scale.x, -100.0..=100.0, "×");
            num(ui, "Scale Y", &mut v.scale.y, -100.0..=100.0, "×");
            let b = v.bounds();
            ui.label(RichText::new(format!("Size {:.1} × {:.1} mm", b.width(), b.height())).color(pal.weak));
            let after = format!("{:?}{:?}{:?}{:?}", v.kind, v.center, v.angle, v.scale);
            if before != after {
                changed = Some((idx, v));
            }
        });
        if let Some((idx, v)) = changed {
            if !self.editing {
                self.push_undo();
                self.editing = true;
            }
            self.doc.vectors[idx] = v;
        }
    }

    fn layers_card(&mut self, ui: &mut Ui) {
        let dark = self.dark;
        let mut add = false;
        let mut move_to: Option<usize> = None;
        let has_sel = !self.sel.is_empty();
        let mut active = self.active_layer;
        card(ui, dark, "Layers", |ui| {
            for (i, l) in self.doc.layers.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut l.visible, "");
                    if ui.selectable_label(active == i, &l.name).on_hover_text("New vectors go on this layer").clicked() {
                        active = i;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if has_sel && ui.small_button("Move here").clicked() {
                            move_to = Some(i);
                        }
                    });
                });
            }
            if ui.button("+ Add layer").clicked() {
                add = true;
            }
        });
        self.active_layer = active;
        if add {
            let n = self.doc.layers.len() + 1;
            self.doc.layers.push(crate::app::Layer { name: format!("Layer {n}"), visible: true });
            self.active_layer = n - 1;
        }
        if let Some(l) = move_to {
            self.push_undo();
            let sel = self.sel.clone();
            for v in self.doc.vectors.iter_mut().filter(|v| sel.contains(&v.id)) {
                v.layer = l;
            }
        }
    }

    fn model_tab(&mut self, ui: &mut Ui) {
        let dark = self.dark;
        let mut open_form: Option<(Form, Option<Profile>)> = None;
        let form_now = self.form;
        let prof_now = self.p.profile;
        card(ui, dark, "Create 3D", |ui| {
            tiles(ui, |ui| {
                let items = [
                    ("shape", "Shape", "Dome, flat or angled shape from closed vectors", Form::Shape, None),
                    ("prism", "Prism", "Sharp-ridged prism from closed vectors – great for lettering", Form::Shape, Some(Profile::Angle)),
                    ("extrude", "Extrude", "Sweep a profile along a curve", Form::Extrude, None),
                    ("two_rail", "Two-rail", "Sweep between two guide curves", Form::TwoRail, None),
                    ("texture", "Texture", "Fill an area with a texture", Form::Texture, None),
                    ("relief", "Library", "Add a ready-made relief", Form::Relief, None),
                    ("import", "Height map", "Import a picture as a 3D height map", Form::ImportHeight, None),
                ];
                for (icon, label, tip, form, prof) in items {
                    let on = form_now == Some(form) && (form != Form::Shape || (prof == Some(Profile::Angle)) == (prof_now == Profile::Angle));
                    if icons::tile(ui, dark, icon, label, on, tip).clicked() {
                        open_form = Some((form, prof));
                    }
                }
            });
        });
        if let Some((form, prof)) = open_form {
            self.form = Some(form);
            if let Some(pr) = prof {
                self.p.profile = pr;
                self.p.limit = true;
                self.p.height = 3.0;
            } else if form == Form::Shape && self.p.profile == Profile::Angle {
                self.p.profile = Profile::Round;
            }
        }
        let mut brush_click: Option<Brush> = None;
        let (tool, brush) = (self.tool, self.p.brush);
        card(ui, dark, "Sculpt", |ui| {
            tiles(ui, |ui| {
                for (b, icon) in [(Brush::Smooth, "smooth"), (Brush::Deposit, "deposit"), (Brush::Remove, "remove"), (Brush::Erase, "erase"), (Brush::Flatten, "flatten")] {
                    let on = tool == DrawTool::Sculpt && brush == b;
                    if icons::tile(ui, dark, icon, b.label(), on, "Paint on the model in the 2D view").clicked() {
                        brush_click = Some(b);
                    }
                }
            });
            ui.add(egui::Slider::new(&mut self.p.brush_size, 1.0..=80.0).text("Brush mm"));
            ui.add(egui::Slider::new(&mut self.p.brush_strength, 0.05..=1.0).text("Strength"));
        });
        if let Some(b) = brush_click {
            if self.tool == DrawTool::Sculpt && self.p.brush == b {
                self.tool = DrawTool::Select;
            } else {
                self.tool = DrawTool::Sculpt;
                self.p.brush = b;
                self.view = View::D2;
                self.show_model2d = true;
            }
        }
        let mut cmd: Option<&str> = None;
        card(ui, dark, "Model tools", |ui| {
            tiles(ui, |ui| {
                for (icon, label, tip, id) in [
                    ("bake", "Bake", "Merge all components and sculpting into one", "bake"),
                    ("smooth", "Smooth all", "Soften the whole model", "smooth"),
                    ("erase", "Clear sculpt", "Remove all sculpting", "clear"),
                    ("stl", "Export STL", "Save the 3D model as an STL mesh", "stl"),
                ] {
                    if icons::tile(ui, dark, icon, label, false, tip).clicked() {
                        cmd = Some(id);
                    }
                }
            });
        });
        match cmd {
            Some("bake") if !self.doc.model.components.is_empty() => {
                self.push_undo();
                let id = self.doc.id();
                self.doc.model.bake(id);
                self.sel_comp = Some(0);
                self.invalidate_sim();
                self.toast("Model baked into one component");
            }
            Some("smooth") => {
                self.push_undo();
                self.doc.model.smooth_all(2);
                self.invalidate_sim();
            }
            Some("clear") => {
                self.push_undo();
                let n = self.doc.model.grid.len();
                self.doc.model.sculpt = vec![0.0; n];
                self.doc.model.composite_only();
                self.invalidate_sim();
            }
            Some("stl") => self.export_stl(),
            _ => {}
        }
    }

    fn components_card(&mut self, ui: &mut Ui) {
        let dark = self.dark;
        let pal = theme::palette(dark);
        let mut action: Option<(&str, usize)> = None;
        let mut recomposite = false;
        let mut sel_comp = self.sel_comp;
        let mut old: Option<(usize, (Combine, f32, f32, f32, f32, f32, f32, bool))> = None;
        card(ui, dark, "Components", |ui| {
            let comps = &mut self.doc.model.components;
            if comps.is_empty() {
                ui.label(RichText::new("No 3D components yet. Select vectors and use Create 3D.").color(pal.weak));
            }
            for (i, c) in comps.iter_mut().enumerate().rev() {
                ui.horizontal(|ui| {
                    if ui.checkbox(&mut c.visible, "").changed() {
                        recomposite = true;
                    }
                    if ui.selectable_label(sel_comp == Some(i), format!("{}  ·  {}", c.name, c.source.kind_label())).clicked() {
                        sel_comp = Some(i);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(c.mode.label()).small().color(ACCENT));
                    });
                });
            }
            let Some(i) = sel_comp.filter(|i| *i < comps.len()) else { return };
            ui.separator();
            let c = &mut comps[i];
            let snap = (c.mode, c.scale, c.offset, c.tilt, c.tilt_dir, c.fade, c.fade_dir, c.invert);
            ui.add(egui::TextEdit::singleline(&mut c.name));
            ui.horizontal(|ui| {
                for m in Combine::ALL {
                    ui.selectable_value(&mut c.mode, m, m.label());
                }
            });
            ui.add(egui::Slider::new(&mut c.scale, 0.0..=4.0).text("Height scale"));
            ui.add(egui::Slider::new(&mut c.offset, -30.0..=30.0).text("Base height"));
            ui.add(egui::Slider::new(&mut c.tilt, -30.0..=30.0).text("Tilt °"));
            if c.tilt != 0.0 {
                ui.add(egui::Slider::new(&mut c.tilt_dir, 0.0..=360.0).text("Tilt direction"));
            }
            ui.add(egui::Slider::new(&mut c.fade, 0.0..=1.0).text("Fade to"));
            if c.fade < 1.0 {
                ui.add(egui::Slider::new(&mut c.fade_dir, 0.0..=360.0).text("Fade direction"));
            }
            ui.checkbox(&mut c.invert, "Invert (carve into the surface)");
            if snap != (c.mode, c.scale, c.offset, c.tilt, c.tilt_dir, c.fade, c.fade_dir, c.invert) {
                recomposite = true;
                old = Some((i, snap));
            }
            ui.horizontal(|ui| {
                if ui.button("▲").on_hover_text("Move up").clicked() {
                    action = Some(("up", i));
                }
                if ui.button("▼").on_hover_text("Move down").clicked() {
                    action = Some(("down", i));
                }
                if ui.button("Copy").clicked() {
                    action = Some(("copy", i));
                }
                if ui.button("🗑 Delete").clicked() {
                    action = Some(("delete", i));
                }
            });
        });
        self.sel_comp = sel_comp;
        if recomposite {
            if !self.editing {
                // The edit already happened in place: store the state from just before it.
                let mut prev = self.doc.clone();
                if let Some((i, s)) = old {
                    let c = &mut prev.model.components[i];
                    (c.mode, c.scale, c.offset, c.tilt, c.tilt_dir, c.fade, c.fade_dir, c.invert) = s;
                }
                self.undo.push(prev);
                self.redo.clear();
                self.editing = old.is_some();
            }
            self.doc.model.composite_only();
            self.invalidate_sim();
        }
        if let Some((a, i)) = action {
            self.push_undo();
            let comps = &mut self.doc.model.components;
            match a {
                "up" if i + 1 < comps.len() => {
                    comps.swap(i, i + 1);
                    self.sel_comp = Some(i + 1);
                }
                "down" if i > 0 => {
                    comps.swap(i, i - 1);
                    self.sel_comp = Some(i - 1);
                }
                "copy" => {
                    let mut c = comps[i].clone();
                    c.name.push_str(" copy");
                    comps.push(c);
                    self.sel_comp = Some(comps.len() - 1);
                }
                "delete" => {
                    comps.remove(i);
                    self.sel_comp = None;
                }
                _ => {}
            }
            self.doc.model.composite_only();
            self.invalidate_sim();
        }
    }
}
