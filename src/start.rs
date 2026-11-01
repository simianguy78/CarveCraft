//! Start screen: startup tasks, recent files and a welcome area.

use crate::app::{App, LeftTab, View};
use crate::theme::{self, ACCENT};
use egui::{pos2, vec2, Align2, Color32, CornerRadius, FontId, RichText, Sense, Shape, Stroke, Ui};
use std::path::PathBuf;

pub const GITHUB_URL: &str = "https://github.com/simianguy78/CarveCraft";

fn recent_file() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("CarveCraft").join("recent.txt"))
}

pub fn load_recent() -> Vec<PathBuf> {
    recent_file()
        .and_then(|f| std::fs::read_to_string(f).ok())
        .map(|s| s.lines().filter(|l| !l.trim().is_empty()).map(PathBuf::from).collect())
        .unwrap_or_default()
}

pub fn remember(list: &mut Vec<PathBuf>, path: &std::path::Path) {
    list.retain(|p| p != path);
    list.insert(0, path.to_path_buf());
    list.truncate(8);
    if let Some(f) = recent_file() {
        if let Some(dir) = f.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let text: Vec<String> = list.iter().map(|p| p.display().to_string()).collect();
        let _ = std::fs::write(f, text.join("\n"));
    }
}

/// A row in the startup task list: small icon, label, hover highlight.
fn task(ui: &mut Ui, dark: bool, icon: &str, label: &str) -> bool {
    let pal = theme::palette(dark);
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(6), pal.tile_hover);
    }
    let ir = egui::Rect::from_center_size(pos2(rect.left() + 20.0, rect.center().y), vec2(20.0, 18.0));
    crate::icons::draw(ui.painter(), ir, ACCENT, icon);
    ui.painter().text(pos2(rect.left() + 42.0, rect.center().y), Align2::LEFT_CENTER, label, FontId::proportional(14.0), pal.text);
    resp.clicked()
}

impl App {
    pub fn start_panel(&mut self, ui: &mut Ui) {
        let dark = self.dark;
        let pal = theme::palette(dark);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(8.0);
            let heading = |ui: &mut Ui, t: &str| {
                ui.add_space(10.0);
                ui.label(RichText::new(t).strong().size(14.0));
                ui.add_space(2.0);
            };
            heading(ui, "Startup Tasks");
            if task(ui, dark, "rect", "Create a new file") {
                self.new_doc();
            }
            if task(ui, dark, "relief", "New file from template") {
                self.push_undo();
                self.doc = crate::panels_right::sample_doc();
                self.after_doc_swap();
                self.home = false;
                self.view = View::D3;
                self.left_tab = LeftTab::Model;
                self.fit_2d = true;
            }
            if task(ui, dark, "import", "Open an existing file") {
                self.open_dialog();
            }
            heading(ui, "Recently opened files …");
            if self.recent.is_empty() {
                ui.label(RichText::new("Files you save or open will appear here.").color(pal.weak));
            }
            let mut open: Option<PathBuf> = None;
            for p in self.recent.clone() {
                let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                if task(ui, dark, "duplicate", &name) {
                    open = Some(p);
                }
            }
            if let Some(p) = open {
                self.open_path(p);
            }
            heading(ui, "Online Resources");
            if task(ui, dark, "import", "CarveCraft on GitHub") {
                ui.ctx().open_url(egui::OpenUrl::new_tab(GITHUB_URL));
            }
            if task(ui, dark, "delete", "Report a problem") {
                ui.ctx().open_url(egui::OpenUrl::new_tab(format!("{GITHUB_URL}/issues")));
            }
            heading(ui, "Learn");
            if task(ui, dark, "text", "Quick guide") {
                self.show_guide = true;
            }
            if task(ui, dark, "shape", "Explore the sample plaque") {
                self.doc = crate::panels_right::sample_doc();
                self.after_doc_swap();
                self.home = false;
                self.view = View::D3;
                self.left_tab = LeftTab::Model;
            }
        });
    }

    pub fn welcome(&mut self, ui: &mut Ui) {
        let pal = theme::palette(self.dark);
        let rect = ui.available_rect_before_wrap();
        ui.allocate_rect(rect, Sense::hover());
        let p = ui.painter_at(rect);
        let c = rect.center() - vec2(0.0, 60.0);
        // brand mark
        let mark = egui::Rect::from_center_size(c - vec2(190.0, 0.0), vec2(92.0, 92.0));
        p.rect_filled(mark, CornerRadius::same(22), ACCENT);
        p.add(Shape::convex_polygon(
            vec![mark.left_top() + vec2(20.0, 26.0), mark.right_top() + vec2(-20.0, 26.0), mark.center_bottom() + vec2(0.0, -20.0)],
            Color32::WHITE,
            Stroke::NONE,
        ));
        p.text(c - vec2(120.0, 14.0), Align2::LEFT_CENTER, "CarveCraft", FontId::proportional(54.0), pal.text);
        p.text(c - vec2(118.0, -32.0), Align2::LEFT_CENTER, "STUDIO  ·  DESIGN  ·  MODEL  ·  CARVE", FontId::proportional(15.0), ACCENT);
        // quick action cards
        let cards = [("rect", "New job", "Start with an empty sheet"), ("relief", "Template", "Open the 3D plaque sample"), ("import", "Open", "Continue a saved job")];
        let w = 200.0;
        let total = w * 3.0 + 24.0 * 2.0;
        let mut x = rect.center().x - total * 0.5;
        let y = c.y + 110.0;
        let mut clicked = None;
        for (i, (icon, title, sub)) in cards.iter().enumerate() {
            let r = egui::Rect::from_min_size(pos2(x, y), vec2(w, 120.0));
            let resp = ui.interact(r, ui.id().with(("card", i)), Sense::click());
            p.rect_filled(r, CornerRadius::same(14), if resp.hovered() { pal.tile_hover } else { pal.card });
            p.rect_stroke(r, CornerRadius::same(14), Stroke::new(1.0_f32, if resp.hovered() { ACCENT } else { pal.border }), egui::StrokeKind::Inside);
            crate::icons::draw(&p, egui::Rect::from_center_size(r.center_top() + vec2(0.0, 36.0), vec2(34.0, 30.0)), ACCENT, icon);
            p.text(r.center_top() + vec2(0.0, 76.0), Align2::CENTER_CENTER, *title, FontId::proportional(16.0), pal.text);
            p.text(r.center_top() + vec2(0.0, 98.0), Align2::CENTER_CENTER, *sub, FontId::proportional(12.0), pal.weak);
            if resp.clicked() {
                clicked = Some(i);
            }
            x += w + 24.0;
        }
        p.text(rect.left_bottom() + vec2(16.0, -16.0), Align2::LEFT_BOTTOM, "CarveCraft Studio 1.0 · 2D design, 3D modelling and toolpaths for CNC routers · github.com/simianguy78/CarveCraft", FontId::proportional(12.0), pal.weak);
        match clicked {
            Some(0) => self.new_doc(),
            Some(1) => {
                self.doc = crate::panels_right::sample_doc();
                self.after_doc_swap();
                self.home = false;
                self.view = View::D3;
                self.left_tab = LeftTab::Model;
            }
            Some(2) => self.open_dialog(),
            _ => {}
        }
    }

    pub fn guide_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_guide;
        egui::Window::new("Quick guide").open(&mut open).collapsible(false).resizable(false).default_width(420.0).show(ctx, |ui| {
            for (n, t) in [
                ("1", "Set up the job: material size and thickness (Job › Edit)."),
                ("2", "Draw vectors or type text in the Design tab."),
                ("3", "Shape them in 3D: Shape, Prism, Extrude, Two-rail, Texture or the relief Library."),
                ("4", "Sculpt to refine: smooth, deposit, remove, erase, flatten."),
                ("5", "Pick a tool and a toolpath on the right: Profile, Pocket, Drill, V-Carve, 3D Roughing/Finishing…"),
                ("6", "Simulate to see the cut, then Save G-code for your machine."),
            ] {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(n).strong().color(ACCENT));
                    ui.label(t);
                });
            }
        });
        self.show_guide = open;
    }
}
