//! The Save Toolpaths window: pick toolpaths and machine, set output options, check the
//! result and preview the G-code, then save one file or one file per tool.

use crate::app::App;
use crate::cam::Post;
use crate::theme::{self, ACCENT};
use crate::toolpath::{self, Toolpath};
use egui::{vec2, Color32, RichText};

impl App {
    /// Open the window with one toolpath ticked, or every visible one.
    pub fn open_export(&mut self, only: Option<usize>) {
        if self.doc.toolpaths.is_empty() {
            self.toast("No toolpaths to save – calculate one first");
            return;
        }
        self.export_pick = match only {
            Some(i) => self.doc.toolpaths.get(i).map(|t| vec![t.id]).unwrap_or_default(),
            None => self.doc.toolpaths.iter().filter(|t| t.visible).map(|t| t.id).collect(),
        };
        self.show_export = true;
    }

    fn picked_paths(&self) -> Vec<&Toolpath> {
        self.doc.toolpaths.iter().filter(|t| self.export_pick.contains(&t.id)).collect()
    }

    pub fn export_window(&mut self, ctx: &egui::Context) {
        if !self.show_export {
            return;
        }
        let pal = theme::palette(self.dark);
        let mut open = true;
        let mut cmd: Option<&'static str> = None;
        egui::Window::new("Save Toolpaths").open(&mut open).default_size([640.0, 620.0]).collapsible(false).show(ctx, |ui| {
            ui.label(RichText::new("Toolpaths to save").strong());
            egui::Frame::new().fill(pal.card).corner_radius(6.0).inner_margin(6.0).show(ui, |ui| {
                ui.set_width(ui.available_width());
                for t in &self.doc.toolpaths {
                    let mut on = self.export_pick.contains(&t.id);
                    ui.horizontal(|ui| {
                        if ui.checkbox(&mut on, &t.name).changed() {
                            if on {
                                self.export_pick.push(t.id);
                            } else {
                                self.export_pick.retain(|x| *x != t.id);
                            }
                        }
                        ui.label(RichText::new(format!("#{} {} · {:.0} min", t.tool.number, t.tool.name, t.minutes.max(1.0))).small().color(pal.weak));
                    });
                }
            });
            // keep the cutting order of the list
            let order: Vec<u64> = self.doc.toolpaths.iter().map(|t| t.id).collect();
            self.export_pick.sort_by_key(|id| order.iter().position(|o| o == id));
            ui.add_space(6.0);

            egui::Grid::new("export_opts").num_columns(2).spacing(vec2(12.0, 6.0)).show(ui, |ui| {
                ui.label("Machine");
                let post = &mut self.doc.material.post;
                egui::ComboBox::from_id_salt("export_post").width(200.0).selected_text(post.label()).show_ui(ui, |ui| {
                    for p in Post::ALL {
                        ui.selectable_value(post, p, p.label());
                    }
                });
                ui.end_row();
                ui.label("Files");
                ui.horizontal(|ui| {
                    ui.radio_value(&mut self.export_split, false, "One file");
                    ui.radio_value(&mut self.export_split, true, "One file per tool");
                });
                ui.end_row();
                let o = &mut self.doc.material.gcode;
                ui.label("Spindle warm-up");
                ui.add(egui::DragValue::new(&mut o.dwell).range(0.0..=60.0).speed(0.1).max_decimals(1).suffix(" s"));
                ui.end_row();
                ui.label("At the end");
                ui.horizontal(|ui| {
                    ui.radio_value(&mut o.end_home, true, "Go home (X0 Y0)");
                    ui.radio_value(&mut o.end_home, false, "Stay, lifted to safe Z");
                });
                ui.end_row();
                ui.label("");
                ui.horizontal(|ui| {
                    ui.checkbox(&mut o.line_numbers, "Line numbers");
                    ui.checkbox(&mut o.compact, "Compact (skip unchanged values)");
                });
                ui.end_row();
            });
            ui.add_space(6.0);

            let paths = self.picked_paths();
            let report = toolpath::check_gcode(&paths, &self.doc.material);
            ui.label(RichText::new("Check").strong());
            egui::Frame::new().fill(pal.card).corner_radius(6.0).inner_margin(8.0).show(ui, |ui| {
                ui.set_width(ui.available_width());
                if paths.is_empty() {
                    ui.label(RichText::new("Tick at least one toolpath").color(pal.weak));
                    return;
                }
                let mut info = format!("About {:.0} min of machining · {:.1} m of cutting · deepest cut {:.2} mm", report.minutes.max(1.0), report.cut_length / 1000.0, -report.deepest);
                if let Some(e) = report.extents {
                    info.push_str(&format!("\nTool centres travel X {:.1} to {:.1}, Y {:.1} to {:.1} mm", e.min.x, e.max.x, e.min.y, e.max.y));
                }
                info.push_str(&format!("\nTools: {}", report.tools.join(", ")));
                if self.export_split && report.tools.len() > 1 {
                    info.push_str(&format!(" – {} files", report.tools.len()));
                }
                ui.label(info);
                if report.warnings.is_empty() {
                    ui.label(RichText::new("✔ No problems found").color(Color32::from_rgb(52, 199, 120)));
                }
                for w in &report.warnings {
                    ui.label(RichText::new(format!("⚠ {w}")).color(Color32::from_rgb(240, 160, 60)));
                }
            });
            ui.add_space(4.0);
            egui::CollapsingHeader::new("Preview G-code").default_open(false).show(ui, |ui| {
                if paths.is_empty() {
                    return;
                }
                let code = toolpath::gcode(&paths, &self.doc.material);
                let lines = code.lines().count();
                ui.label(RichText::new(format!("{lines} lines · {:.0} KB", code.len() as f32 / 1024.0)).small().color(pal.weak));
                let shown: String = code.lines().take(400).collect::<Vec<_>>().join("\n");
                egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                    ui.add(egui::Label::new(RichText::new(shown).monospace().size(11.5)).selectable(true));
                    if lines > 400 {
                        ui.label(RichText::new(format!("… {} more lines", lines - 400)).small().color(pal.weak));
                    }
                });
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let save = egui::Button::new(RichText::new("Save…").color(Color32::WHITE).strong()).fill(ACCENT).min_size(vec2(110.0, 28.0));
                if ui.add_enabled(!paths.is_empty(), save).clicked() {
                    cmd = Some("save");
                }
                if ui.add_enabled(!paths.is_empty(), egui::Button::new("Copy to clipboard")).clicked() {
                    cmd = Some("copy");
                }
                if ui.button("Close").clicked() {
                    cmd = Some("close");
                }
            });
        });
        match cmd {
            Some("save") => self.save_gcode_files(),
            Some("copy") => {
                let code = toolpath::gcode(&self.picked_paths(), &self.doc.material);
                ctx.copy_text(code);
                self.toast("G-code copied");
            }
            Some("close") => self.show_export = false,
            _ => {}
        }
        if !open {
            self.show_export = false;
        }
    }

    fn save_gcode_files(&mut self) {
        let ext = self.doc.material.post.extension();
        let paths = self.picked_paths();
        // group by tool, keeping the cutting order within each file
        let groups: Vec<(String, Vec<&Toolpath>)> = if self.export_split {
            let mut g: Vec<(String, Vec<&Toolpath>)> = Vec::new();
            for t in paths {
                let key = format!("T{} {}", t.tool.number, t.tool.name);
                match g.iter_mut().find(|(k, _)| *k == key) {
                    Some((_, v)) => v.push(t),
                    None => g.push((key, vec![t])),
                }
            }
            g
        } else {
            vec![(String::new(), paths)]
        };
        let Some(path) = rfd::FileDialog::new().add_filter("G-code", &[ext, "nc", "gcode", "tap", "ngc"]).set_file_name(format!("{}.{ext}", self.doc.name)).save_file() else { return };
        let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| self.doc.name.clone());
        let dir = path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
        let real_ext = path.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_else(|| ext.to_string());
        let mut saved = Vec::new();
        for (key, group) in &groups {
            let file = if groups.len() > 1 {
                let safe: String = key.chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' { c } else { '_' }).collect();
                dir.join(format!("{stem} - {}.{real_ext}", safe.trim()))
            } else {
                path.clone()
            };
            let code = toolpath::gcode(group, &self.doc.material);
            match std::fs::write(&file, code) {
                Ok(()) => saved.push(file.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default()),
                Err(e) => {
                    self.toast(format!("Couldn't save {}: {e}", file.display()));
                    return;
                }
            }
        }
        self.toast(if saved.len() == 1 { format!("G-code saved: {}", saved[0]) } else { format!("Saved {} files: {}", saved.len(), saved.join(", ")) });
        self.show_export = false;
    }
}
