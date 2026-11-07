//! Tool library (like VCarve's Tool Database): tools in groups, feeds and speeds per material,
//! saved between sessions, with an editor window.

use crate::app::App;
use crate::theme::{self, ACCENT};
use crate::toolpath::{Cutting, Tool, ToolKind};
use egui::{vec2, Color32, RichText};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Library {
    pub tools: Vec<Tool>,
    pub materials: Vec<String>,
    /// The material whose feeds and speeds the tools currently show.
    pub material: String,
    /// Spindle and feed limits used by the feeds & speeds calculator.
    #[serde(default)]
    pub machine: crate::feeds::Machine,
}

fn library_file() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("CarveCraft").join("tools.json"))
}

pub fn load() -> Library {
    library_file()
        .and_then(|f| std::fs::read_to_string(f).ok())
        .and_then(|s| serde_json::from_str::<Library>(&s).ok())
        .filter(|l| !l.tools.is_empty())
        .unwrap_or_else(default_library)
}

pub fn save(lib: &Library) {
    if let Some(f) = library_file() {
        if let Some(dir) = f.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(s) = serde_json::to_string_pretty(lib) {
            let _ = std::fs::write(f, s);
        }
    }
}

/// A starting library much like VCarve's, in millimetres, with common inch sizes too.
pub fn default_library() -> Library {
    let mut n = 0;
    let mut t = |group: &str, name: &str, kind: ToolKind, dia: f32, angle: f32, step: f32, pass: f32, feed: f32, rpm: f32| {
        n += 1;
        let mut tool = Tool::new(n, name, kind, dia, angle, step, pass, feed);
        tool.rpm = rpm;
        tool.group = group.into();
        tool
    };
    let tools = vec![
        t("End Mills", "End Mill 6 mm", ToolKind::EndMill, 6.0, 0.0, 2.4, 3.0, 1500.0, 18000.0),
        t("End Mills", "End Mill 3 mm", ToolKind::EndMill, 3.0, 0.0, 1.2, 1.5, 1000.0, 18000.0),
        t("End Mills", "End Mill 1 mm", ToolKind::EndMill, 1.0, 0.0, 0.4, 0.5, 500.0, 20000.0),
        t("End Mills", "End Mill (1/8\")", ToolKind::EndMill, 3.175, 0.0, 1.27, 1.6, 1000.0, 18000.0),
        t("End Mills", "End Mill (1/4\")", ToolKind::EndMill, 6.35, 0.0, 2.54, 3.2, 1500.0, 18000.0),
        t("Ball Nose", "Ball Nose 3 mm", ToolKind::BallNose, 3.0, 0.0, 0.3, 1.5, 2000.0, 18000.0),
        t("Ball Nose", "Ball Nose 6 mm", ToolKind::BallNose, 6.0, 0.0, 0.6, 3.0, 2500.0, 18000.0),
        t("Ball Nose", "Ball Nose (1/8\")", ToolKind::BallNose, 3.175, 0.0, 0.32, 1.6, 2000.0, 18000.0),
        t("V-Bits", "V-Bit 60°", ToolKind::VBit, 12.0, 60.0, 0.4, 2.0, 1200.0, 18000.0),
        t("V-Bits", "V-Bit 90°", ToolKind::VBit, 12.0, 90.0, 0.5, 2.0, 1200.0, 18000.0),
        t("V-Bits", "V-Bit 90° (1/2\")", ToolKind::VBit, 12.7, 90.0, 0.5, 2.0, 1200.0, 18000.0),
        t("Engraving", "Engraving 30°", ToolKind::VBit, 3.0, 30.0, 0.2, 0.5, 800.0, 20000.0),
        t("Engraving", "Engraving 15°", ToolKind::VBit, 3.0, 15.0, 0.1, 0.3, 600.0, 22000.0),
    ];
    Library { tools, materials: ["Default", "Softwood", "Hardwood", "MDF", "Plywood", "Plastic (acrylic)", "Aluminium"].map(String::from).to_vec(), material: "Default".into(), machine: Default::default() }
}

/// The library tool nearest to what an operation usually wants (kind, then size/angle).
pub fn pick(tools: &[Tool], kind: ToolKind, size: f32) -> usize {
    tools
        .iter()
        .enumerate()
        .min_by(|a, b| {
            let score = |t: &Tool| {
                let key = if kind == ToolKind::VBit { t.angle } else { t.diameter };
                (if t.kind == kind { 0.0 } else { 1e6 }) + (key - size).abs()
            };
            score(a.1).total_cmp(&score(b.1))
        })
        .map(|(i, _)| i)
        .unwrap_or(0)
}

/// Switch every tool to `material`'s feeds and speeds, remembering the current ones.
pub fn switch_material(lib: &mut Library, material: &str) {
    if lib.material == material {
        return;
    }
    let old = lib.material.clone();
    for t in &mut lib.tools {
        t.materials.insert(old.clone(), t.cutting());
        if let Some(c) = t.materials.get(material).cloned() {
            t.set_cutting(&c);
        }
    }
    lib.material = material.to_string();
}

impl App {
    fn library(&self) -> Library {
        Library { tools: self.tools.clone(), materials: self.tool_materials.clone(), material: self.tool_material.clone(), machine: self.fs_machine.clone() }
    }

    fn set_library(&mut self, lib: Library) {
        self.tools = lib.tools;
        self.tool_materials = lib.materials;
        self.tool_material = lib.material;
        self.fs_machine = lib.machine;
        self.tool_idx = self.tool_idx.min(self.tools.len().saturating_sub(1));
        self.db_sel = self.db_sel.min(self.tools.len().saturating_sub(1));
    }

    /// The Tool Database window.
    pub fn tool_db_window(&mut self, ctx: &egui::Context) {
        if !self.show_tool_db {
            return;
        }
        let mut open = true;
        let mut changed = false;
        let mut cmd: Option<&'static str> = None;
        let mut open_calc: Option<usize> = None;
        let pal = theme::palette(self.dark);
        egui::Window::new("Tool Database").open(&mut open).default_size([780.0, 640.0]).collapsible(false).show(ctx, |ui| {
            // Material picker, like the original's top bar.
            ui.horizontal(|ui| {
                ui.label(RichText::new("Material").strong());
                let mut pick = self.tool_material.clone();
                egui::ComboBox::from_id_salt("tooldb_material").width(170.0).selected_text(&pick).show_ui(ui, |ui| {
                    for m in &self.tool_materials {
                        ui.selectable_value(&mut pick, m.clone(), m);
                    }
                });
                if pick != self.tool_material {
                    let mut lib = self.library();
                    switch_material(&mut lib, &pick);
                    self.set_library(lib);
                    changed = true;
                }
                if ui.button("+ Material").on_hover_text("Add a material; its feeds start as a copy of the current ones").clicked() {
                    let name = format!("Material {}", self.tool_materials.len() + 1);
                    self.tool_materials.push(name);
                    changed = true;
                }
                ui.label(RichText::new("Feeds and speeds below are for this material").small().color(pal.weak));
            });
            ui.separator();
            ui.horizontal_top(|ui| {
                // Left: tools in their groups.
                ui.vertical(|ui| {
                    ui.set_width(250.0);
                    egui::ScrollArea::vertical().id_salt("tooldb_tree").max_height(ui.available_height().max(420.0) - 110.0).show(ui, |ui| {
                        let mut groups: Vec<String> = Vec::new();
                        for t in &self.tools {
                            if !groups.contains(&t.group) {
                                groups.push(t.group.clone());
                            }
                        }
                        for g in groups {
                            egui::CollapsingHeader::new(RichText::new(if g.is_empty() { "Tools" } else { g.as_str() }).strong()).id_salt(("grp", g.clone())).default_open(true).show(ui, |ui| {
                                for i in (0..self.tools.len()).filter(|i| self.tools[*i].group == g) {
                                    let t = &self.tools[i];
                                    let label = format!("{}  #{}", t.name, t.number);
                                    ui.horizontal(|ui| {
                                        if ui.selectable_label(self.db_sel == i, label).clicked() {
                                            self.db_sel = i;
                                        }
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            if ui.small_button("Check").on_hover_text("Check this tool's feeds and speeds").clicked() {
                                                self.db_sel = i;
                                                open_calc = Some(i);
                                            }
                                        });
                                    });
                                }
                            });
                        }
                    });
                    ui.add_space(4.0);
                    ui.horizontal_wrapped(|ui| {
                        for (id, label, tip) in [
                            ("add", "+ Tool", "Add a tool to this group"),
                            ("group", "+ Group", "Start a new group with one tool"),
                            ("copy", "Copy", "Duplicate the selected tool"),
                            ("delete", "Delete", "Remove the selected tool"),
                            ("import", "Import…", "Load tools from a file"),
                            ("export", "Export…", "Save the library to a file"),
                            ("reset", "Reset", "Go back to the starting library"),
                        ] {
                            if ui.small_button(label).on_hover_text(tip).clicked() {
                                cmd = Some(id);
                            }
                        }
                    });
                });
                ui.separator();
                // Right: the selected tool's settings.
                ui.vertical(|ui| {
                    let Some(t) = self.tools.get_mut(self.db_sel) else { return };
                    let before = serde_json::to_string(t).unwrap_or_default();
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Name").strong());
                        ui.add(egui::TextEdit::singleline(&mut t.name).desired_width(300.0));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Group");
                        ui.add(egui::TextEdit::singleline(&mut t.group).desired_width(160.0));
                        ui.label("Tool number");
                        ui.add(egui::DragValue::new(&mut t.number).range(1..=999));
                    });
                    ui.label("Notes");
                    ui.add(egui::TextEdit::multiline(&mut t.notes).desired_rows(2).desired_width(f32::INFINITY));
                    ui.horizontal(|ui| {
                        ui.label("Tool type");
                        egui::ComboBox::from_id_salt("tooldb_kind").selected_text(t.kind.label()).show_ui(ui, |ui| {
                            for k in [ToolKind::EndMill, ToolKind::BallNose, ToolKind::VBit] {
                                ui.selectable_value(&mut t.kind, k, k.label());
                            }
                        });
                    });
                    ui.add_space(4.0);
                    ui.label(RichText::new("Geometry").strong());
                    egui::Grid::new("tooldb_geo").num_columns(2).spacing(vec2(12.0, 4.0)).show(ui, |ui| {
                        ui.label("Diameter (D)");
                        ui.add(egui::DragValue::new(&mut t.diameter).range(0.1..=100.0).speed(0.01).max_decimals(3).suffix(" mm"));
                        ui.end_row();
                        if t.kind == ToolKind::VBit {
                            ui.label("Included angle");
                            ui.add(egui::DragValue::new(&mut t.angle).range(5.0..=170.0).speed(0.5).suffix("°"));
                            ui.end_row();
                        }
                        ui.label("No. flutes");
                        ui.add(egui::DragValue::new(&mut t.flutes).range(1..=8));
                        ui.end_row();
                    });
                    ui.add_space(4.0);
                    ui.label(RichText::new("Cutting parameters").strong());
                    egui::Grid::new("tooldb_cut").num_columns(3).spacing(vec2(12.0, 4.0)).show(ui, |ui| {
                        ui.label("Pass depth");
                        ui.add(egui::DragValue::new(&mut t.pass_depth).range(0.05..=50.0).speed(0.02).max_decimals(2).suffix(" mm"));
                        ui.label("");
                        ui.end_row();
                        ui.label("Stepover");
                        ui.add(egui::DragValue::new(&mut t.stepover).range(0.01..=50.0).speed(0.01).max_decimals(3).suffix(" mm"));
                        // the same stepover as a percentage of the diameter, linked both ways
                        let mut pct = t.stepover / t.diameter.max(1e-3) * 100.0;
                        if ui.add(egui::DragValue::new(&mut pct).range(1.0..=100.0).speed(0.5).max_decimals(1).suffix(" %")).changed() {
                            t.stepover = t.diameter * pct / 100.0;
                        }
                        ui.end_row();
                    });
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Feeds and speeds").strong());
                        if ui.small_button("Feeds & Speeds calculator…").on_hover_text("Recommend settings for this tool, material and machine").clicked() {
                            open_calc = Some(self.db_sel);
                        }
                    });
                    egui::Grid::new("tooldb_feeds").num_columns(2).spacing(vec2(12.0, 4.0)).show(ui, |ui| {
                        ui.label("Spindle speed");
                        ui.add(egui::DragValue::new(&mut t.rpm).range(100.0..=60000.0).speed(50.0).suffix(" r.p.m"));
                        ui.end_row();
                        ui.label("Feed rate");
                        ui.add(egui::DragValue::new(&mut t.feed).range(1.0..=30000.0).speed(5.0).suffix(" mm/min"));
                        ui.end_row();
                        ui.label("Plunge rate");
                        ui.add(egui::DragValue::new(&mut t.plunge).range(1.0..=30000.0).speed(5.0).suffix(" mm/min"));
                        ui.end_row();
                        ui.label("Chip load");
                        let chip = t.feed / (t.rpm.max(1.0) * t.flutes.max(1) as f32);
                        ui.label(RichText::new(format!("{chip:.4} mm per tooth")).color(pal.weak));
                        ui.end_row();
                    });
                    if serde_json::to_string(t).unwrap_or_default() != before {
                        changed = true;
                    }
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        let select = egui::Button::new(RichText::new("Select").color(Color32::WHITE).strong()).fill(ACCENT);
                        if ui.add(select).on_hover_text("Use this tool for the toolpath").clicked() {
                            cmd = Some("select");
                        }
                        if ui.button("Close").clicked() {
                            cmd = Some("close");
                        }
                    });
                });
            });
        });
        if let Some(i) = open_calc {
            self.fs_calc = Some(FsCalc { tool: i, stock: crate::feeds::Stock::from_name(&self.tool_material), apply: [true; 5] });
        }
        if self.feeds_window(ctx) {
            changed = true;
        }
        let sel = self.db_sel.min(self.tools.len().saturating_sub(1));
        match cmd {
            Some("add") | Some("copy") | Some("group") => {
                let mut t = self.tools.get(sel).cloned().unwrap_or_else(|| Tool::defaults()[0].clone());
                t.number = self.tools.iter().map(|t| t.number).max().unwrap_or(0) + 1;
                match cmd {
                    Some("add") => t.name = format!("New {}", t.kind.label().to_lowercase()),
                    Some("group") => {
                        t.group = "New group".into();
                        t.name = format!("New {}", t.kind.label().to_lowercase());
                    }
                    _ => t.name = format!("{} (copy)", t.name),
                }
                self.tools.insert((sel + 1).min(self.tools.len()), t);
                self.db_sel = (sel + 1).min(self.tools.len() - 1);
                changed = true;
            }
            Some("delete") if self.tools.len() > 1 => {
                self.tools.remove(sel);
                self.db_sel = sel.min(self.tools.len() - 1);
                self.tool_idx = self.tool_idx.min(self.tools.len() - 1);
                changed = true;
            }
            Some("import") => {
                if let Some(p) = rfd::FileDialog::new().add_filter("CarveCraft tools", &["json"]).pick_file() {
                    match std::fs::read_to_string(&p).ok().and_then(|s| serde_json::from_str::<Library>(&s).ok()) {
                        Some(lib) if !lib.tools.is_empty() => {
                            self.set_library(lib);
                            changed = true;
                            self.toast("Tool library imported");
                        }
                        _ => self.toast("That file isn't a CarveCraft tool library"),
                    }
                }
            }
            Some("export") => {
                if let Some(p) = rfd::FileDialog::new().add_filter("CarveCraft tools", &["json"]).set_file_name("carvecraft-tools.json").save_file() {
                    let ok = serde_json::to_string_pretty(&self.library()).ok().and_then(|s| std::fs::write(&p, s).ok()).is_some();
                    self.toast(if ok { "Tool library exported" } else { "Couldn't save the tool library" });
                }
            }
            Some("reset") => {
                self.set_library(default_library());
                changed = true;
            }
            Some("select") => {
                self.tool_idx = sel;
                self.show_tool_db = false;
                self.toast(format!("Tool: {}", self.tools[sel].name));
            }
            Some("close") => self.show_tool_db = false,
            _ => {}
        }
        if !open {
            self.show_tool_db = false;
        }
        if changed {
            save(&self.library());
        }
    }
}

/// The calculator's state: which tool, which material, and which values to apply.
pub struct FsCalc {
    pub tool: usize,
    pub stock: crate::feeds::Stock,
    /// Spindle speed, feed, plunge, pass depth, stepover.
    pub apply: [bool; 5],
}

impl App {
    /// The Feeds & Speeds calculator window. Returns true when values were applied to a tool.
    fn feeds_window(&mut self, ctx: &egui::Context) -> bool {
        use crate::feeds::{advise, Rigidity, Stock};
        let Some(calc) = self.fs_calc.as_mut() else { return false };
        let Some(tool) = self.tools.get(calc.tool).cloned() else {
            self.fs_calc = None;
            return false;
        };
        let pal = theme::palette(self.dark);
        let mut open = true;
        let mut action: Option<bool> = None;
        let machine = &mut self.fs_machine;
        egui::Window::new("Feeds & Speeds calculator").open(&mut open).default_width(520.0).collapsible(false).order(egui::Order::Foreground).show(ctx, |ui| {
            // What was loaded from the tool
            ui.label(RichText::new(format!("{}  #{}", tool.name, tool.number)).strong().size(15.0));
            let shape = if tool.kind == ToolKind::VBit { format!("{} · {:.0}° · {} mm", tool.kind.label(), tool.angle, tool.diameter) } else { format!("{} · {} mm", tool.kind.label(), tool.diameter) };
            ui.label(RichText::new(format!("{shape} · {} flutes", tool.flutes)).color(pal.weak));
            ui.add_space(6.0);
            egui::Grid::new("fs_inputs").num_columns(2).spacing(vec2(12.0, 6.0)).show(ui, |ui| {
                ui.label("Material");
                egui::ComboBox::from_id_salt("fs_stock").width(220.0).selected_text(calc.stock.label()).show_ui(ui, |ui| {
                    for s in Stock::ALL {
                        ui.selectable_value(&mut calc.stock, s, s.label());
                    }
                });
                ui.end_row();
                ui.label("Machine");
                egui::ComboBox::from_id_salt("fs_rig").width(220.0).selected_text(machine.rigidity.label()).show_ui(ui, |ui| {
                    for r in Rigidity::ALL {
                        ui.selectable_value(&mut machine.rigidity, r, r.label());
                    }
                });
                ui.end_row();
                ui.label("Spindle range");
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut machine.min_rpm).range(1000.0..=60000.0).speed(100.0));
                    ui.label("to");
                    ui.add(egui::DragValue::new(&mut machine.max_rpm).range(1000.0..=60000.0).speed(100.0).suffix(" r.p.m"));
                });
                ui.end_row();
                ui.label("Fastest feed");
                ui.add(egui::DragValue::new(&mut machine.max_feed).range(100.0..=40000.0).speed(10.0).suffix(" mm/min"));
                ui.end_row();
            });
            machine.max_rpm = machine.max_rpm.max(machine.min_rpm);
            ui.add_space(8.0);

            // Current against recommended, with a tick for each value to use
            let adv = advise(&tool, calc.stock, machine);
            let rows = [
                ("Spindle speed", tool.rpm, adv.rpm, " r.p.m", 0),
                ("Feed rate", tool.feed, adv.feed, " mm/min", 0),
                ("Plunge rate", tool.plunge, adv.plunge, " mm/min", 0),
                ("Pass depth", tool.pass_depth, adv.pass_depth, " mm", 2),
                ("Stepover", tool.stepover, adv.stepover, " mm", 3),
            ];
            egui::Frame::new().fill(pal.card).corner_radius(8.0).inner_margin(8.0).show(ui, |ui| {
                ui.set_width(ui.available_width());
                egui::Grid::new("fs_results").num_columns(4).spacing(vec2(18.0, 6.0)).striped(true).show(ui, |ui| {
                    ui.label(RichText::new("Use").small().color(pal.weak));
                    ui.label(RichText::new("Setting").small().color(pal.weak));
                    ui.label(RichText::new("Current").small().color(pal.weak));
                    ui.label(RichText::new("Recommended").small().color(pal.weak));
                    ui.end_row();
                    for (k, (name, cur, rec, unit, dp)) in rows.iter().enumerate() {
                        ui.checkbox(&mut calc.apply[k], "");
                        ui.label(*name);
                        ui.label(format!("{:.*}{unit}", dp, cur));
                        // highlight values more than 15% away from the current ones
                        let far = (rec - cur).abs() > cur.abs().max(1e-3) * 0.15;
                        let col = if far { Color32::from_rgb(255, 184, 77) } else { pal.text };
                        ui.label(RichText::new(format!("{:.*}{unit}", dp, rec)).strong().color(col));
                        ui.end_row();
                    }
                });
                let cur_chip = tool.feed / (tool.rpm.max(1.0) * tool.flutes.max(1) as f32);
                ui.label(RichText::new(format!("Chip load: now {cur_chip:.3} mm per tooth, recommended {:.3}", adv.chip_load)).small().color(pal.weak));
            });
            for n in &adv.notes {
                ui.label(RichText::new(format!("⚠ {n}")).color(Color32::from_rgb(240, 160, 60)));
            }
            ui.label(RichText::new("Starting values: test on scrap and adjust by sound and chips.").small().color(pal.weak));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let n = calc.apply.iter().filter(|a| **a).count();
                let use_btn = egui::Button::new(RichText::new(format!("Use ticked values ({n})")).color(Color32::WHITE).strong()).fill(ACCENT);
                if ui.add_enabled(n > 0, use_btn).clicked() {
                    action = Some(true);
                }
                if ui.button("Keep current values").clicked() {
                    action = Some(false);
                }
            });
        });
        let mut applied = false;
        if action == Some(true) {
            if let Some(calc) = self.fs_calc.take() {
                let adv = advise(&tool, calc.stock, &self.fs_machine);
                if let Some(t) = self.tools.get_mut(calc.tool) {
                    let vals = [adv.rpm, adv.feed, adv.plunge, adv.pass_depth, adv.stepover];
                    let slots: [&mut f32; 5] = [&mut t.rpm, &mut t.feed, &mut t.plunge, &mut t.pass_depth, &mut t.stepover];
                    for ((slot, v), on) in slots.into_iter().zip(vals).zip(calc.apply) {
                        if on {
                            *slot = v;
                        }
                    }
                    self.status = format!("Feeds and speeds updated for {} ({})", t.name, self.tool_material);
                }
                applied = true;
            }
        } else if action == Some(false) || !open {
            self.fs_calc = None;
        }
        // the machine limits are worth keeping even when nothing is applied
        if applied || action.is_some() || !open {
            save(&self.library());
        }
        applied
    }
}

/// Settings that differ per material.
impl Tool {
    pub fn cutting(&self) -> Cutting {
        Cutting { stepover: self.stepover, pass_depth: self.pass_depth, feed: self.feed, plunge: self.plunge, rpm: self.rpm }
    }
    pub fn set_cutting(&mut self, c: &Cutting) {
        self.stepover = c.stepover;
        self.pass_depth = c.pass_depth;
        self.feed = c.feed;
        self.plunge = c.plunge;
        self.rpm = c.rpm;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn materials_keep_their_own_feeds() {
        let mut lib = default_library();
        lib.tools[0].feed = 1500.0;
        switch_material(&mut lib, "MDF");
        lib.tools[0].feed = 2400.0; // tuned for MDF
        switch_material(&mut lib, "Hardwood");
        assert_eq!(lib.tools[0].feed, 2400.0, "a new material starts from the current values");
        lib.tools[0].feed = 900.0;
        switch_material(&mut lib, "MDF");
        assert_eq!(lib.tools[0].feed, 2400.0);
        switch_material(&mut lib, "Hardwood");
        assert_eq!(lib.tools[0].feed, 900.0);
        switch_material(&mut lib, "Default");
        assert_eq!(lib.tools[0].feed, 1500.0);
    }

    #[test]
    fn picks_sensible_defaults() {
        let lib = default_library();
        let v = pick(&lib.tools, ToolKind::VBit, 60.0);
        assert_eq!(lib.tools[v].angle, 60.0);
        let b = pick(&lib.tools, ToolKind::BallNose, 3.0);
        assert_eq!((lib.tools[b].kind, lib.tools[b].diameter), (ToolKind::BallNose, 3.0));
        let library_round_trip: Library = serde_json::from_str(&serde_json::to_string(&lib).unwrap()).unwrap();
        assert_eq!(library_round_trip.tools.len(), lib.tools.len());
    }
}
