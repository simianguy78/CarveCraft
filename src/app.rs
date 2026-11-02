//! Application state, document and commands.

use crate::geom::{self, Poly, ShapeKind, Vector};
use crate::model::{Brush, Combine, Component, Model, Profile, ReliefKind, Source, TextureKind};
use crate::render3d::Camera;
use crate::theme;
use crate::toolpath::{self, Material, Op, Sim, Tool, ToolKind, Toolpath};
use egui::{pos2, vec2, Pos2, Rect, TextureHandle};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Layer {
    pub name: String,
    pub visible: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Doc {
    pub name: String,
    pub material: Material,
    pub vectors: Vec<Vector>,
    pub layers: Vec<Layer>,
    pub model: Model,
    pub toolpaths: Vec<Toolpath>,
    pub next_id: u64,
}

impl Doc {
    pub fn new(material: Material) -> Doc {
        let model = Model::new(material.w, material.h);
        Doc {
            name: "Untitled".into(),
            material,
            vectors: Vec::new(),
            layers: vec![Layer { name: "Layer 1".into(), visible: true }],
            model,
            toolpaths: Vec::new(),
            next_id: 1,
        }
    }
    pub fn id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }
    pub fn after_load(&mut self) {
        self.model.rebuild();
        let (m, mat) = (&self.model, &self.material);
        for t in &mut self.toolpaths {
            t.calculate(m, mat);
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum View {
    D2,
    D3,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LeftTab {
    Design,
    Model,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DrawTool {
    Select,
    Rect,
    Circle,
    Ellipse,
    Polygon,
    Star,
    Polyline,
    Arc,
    Freehand,
    Text,
    Node,
    Trim,
    Fillet,
    Sculpt,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Form {
    Material,
    Offset,
    CircularArray,
    GridArray,
    Shape,
    Extrude,
    TwoRail,
    Texture,
    Relief,
    ImportHeight,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ModelColor {
    Wood,
    Gold,
    Slate,
}

pub struct Params {
    pub rect_r: f32,
    pub sides: u32,
    pub star_points: u32,
    pub star_ratio: f32,
    pub text: String,
    pub text_size: f32,
    pub offset: f32,
    pub outward: bool,
    pub arr_count: u32,
    pub arr_angle: f32,
    pub grid_cols: u32,
    pub grid_rows: u32,
    pub grid_gap: f32,
    pub profile: Profile,
    pub height: f32,
    pub angle: f32,
    pub base: f32,
    pub limit: bool,
    pub mode: Combine,
    pub ext_width: f32,
    pub tex_kind: TextureKind,
    pub tex_depth: f32,
    pub tex_size: f32,
    pub relief: ReliefKind,
    pub relief_height: f32,
    pub relief_size: f32,
    pub brush: Brush,
    pub brush_size: f32,
    pub brush_strength: f32,
    pub img_height: f32,
    pub fillet_r: f32,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            rect_r: 0.0,
            sides: 6,
            star_points: 5,
            star_ratio: 0.45,
            text: "Hello".into(),
            text_size: 25.0,
            offset: 5.0,
            outward: true,
            arr_count: 8,
            arr_angle: 360.0,
            grid_cols: 3,
            grid_rows: 2,
            grid_gap: 5.0,
            profile: Profile::Round,
            height: 6.0,
            angle: 45.0,
            base: 0.0,
            limit: false,
            mode: Combine::Add,
            ext_width: 10.0,
            tex_kind: TextureKind::Hammered,
            tex_depth: 1.0,
            tex_size: 6.0,
            relief: ReliefKind::Rosette,
            relief_height: 8.0,
            relief_size: 80.0,
            brush: Brush::Smooth,
            brush_size: 10.0,
            brush_strength: 0.5,
            img_height: 8.0,
            fillet_r: 5.0,
        }
    }
}

pub enum Drag {
    None,
    Pan,
    Orbit,
    Move,
    Marquee { start: Pos2 },
    Create { start: Pos2 },
    Freehand { pts: Vec<Pos2> },
    Sculpt { target: f32 },
    Node { id: u64, idx: usize },
}

pub struct App {
    pub doc: Doc,
    pub undo: Vec<Doc>,
    pub redo: Vec<Doc>,
    pub file: Option<PathBuf>,
    pub view: View,
    pub left_tab: LeftTab,
    pub tool: DrawTool,
    pub sel: Vec<u64>,
    pub form: Option<Form>,
    pub p: Params,
    pub pending_op: Option<Op>,
    pub tools: Vec<Tool>,
    pub tool_idx: usize,
    pub v2_center: Pos2,
    pub v2_zoom: f32,
    pub fit_2d: bool,
    pub cam: Camera,
    pub drag: Drag,
    pub poly_pts: Vec<Pos2>,
    pub model_tex: Option<TextureHandle>,
    pub model_tex_key: usize,
    pub model_z: Vec<f32>,
    pub sim: Option<Sim>,
    pub sim_tex: Option<TextureHandle>,
    pub sim_tex_rev: u64,
    pub show_sim: bool,
    pub sim_running: bool,
    pub sim_speed: f32,
    pub sel_comp: Option<usize>,
    pub sel_tp: Option<usize>,
    pub dark: bool,
    pub applied_dark: Option<bool>,
    pub status: String,
    pub show_grid: bool,
    pub snap: bool,
    pub show_paths: bool,
    pub show_model2d: bool,
    pub model_color: ModelColor,
    pub cursor: Option<Pos2>,
    pub active_layer: usize,
    pub editing: bool,
    pub home: bool,
    pub recent: Vec<PathBuf>,
    pub show_guide: bool,
    screenshot: Option<PathBuf>,
    frames: u32,
    shot_sent: bool,
}

impl App {
    pub fn new(cc: &eframe::CreationContext, screenshot: Option<PathBuf>, open: Option<PathBuf>) -> App {
        theme::apply(&cc.egui_ctx, true);
        let mut app = App {
            doc: crate::panels_right::sample_doc(),
            undo: Vec::new(),
            redo: Vec::new(),
            file: None,
            view: View::D3,
            left_tab: LeftTab::Model,
            tool: DrawTool::Select,
            sel: Vec::new(),
            form: None,
            p: Params::default(),
            pending_op: None,
            tools: Tool::defaults(),
            tool_idx: 2,
            v2_center: pos2(150.0, 100.0),
            v2_zoom: 2.0,
            fit_2d: true,
            cam: Camera::default(),
            drag: Drag::None,
            poly_pts: Vec::new(),
            model_tex: None,
            model_tex_key: 0,
            model_z: Vec::new(),
            sim: None,
            sim_tex: None,
            sim_tex_rev: 0,
            show_sim: false,
            sim_running: false,
            sim_speed: 3.2,
            sel_comp: Some(3),
            sel_tp: None,
            dark: true,
            applied_dark: Some(true),
            status: "Ready".into(),
            show_grid: true,
            snap: false,
            show_paths: true,
            show_model2d: true,
            model_color: ModelColor::Wood,
            cursor: None,
            active_layer: 0,
            editing: false,
            home: true,
            recent: crate::start::load_recent(),
            show_guide: false,
            screenshot,
            frames: 0,
            shot_sent: false,
        };
        // Start-up view overrides, handy for checking screens: CARVECRAFT_START=2d|design|light|sim
        for opt in std::env::var("CARVECRAFT_START").unwrap_or_default().split(',') {
            if !opt.trim().is_empty() && opt.trim() != "home" {
                app.home = false;
            }
            match opt.trim() {
                "2d" => app.view = View::D2,
                "design" => app.left_tab = LeftTab::Design,
                "light" => app.dark = false,
                "sim" => app.start_sim(false),
                "select" => app.sel = app.doc.vectors.iter().skip(3).take(1).map(|v| v.id).collect(),
                _ => {}
            }
        }
        if let Some(path) = open {
            app.open_path(path);
        }
        app
    }

    // ------------------------------------------------------------ helpers

    pub fn push_undo(&mut self) {
        self.undo.push(self.doc.clone());
        if self.undo.len() > 40 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub fn do_undo(&mut self) {
        if let Some(d) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.doc, d));
            self.after_doc_swap();
            self.status = "Undo".into();
        }
    }

    pub fn do_redo(&mut self) {
        if let Some(d) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.doc, d));
            self.after_doc_swap();
            self.status = "Redo".into();
        }
    }

    pub fn after_doc_swap(&mut self) {
        self.sel.retain(|id| self.doc.vectors.iter().any(|v| v.id == *id));
        if self.sel_comp.is_some_and(|i| i >= self.doc.model.components.len()) {
            self.sel_comp = None;
        }
        if self.sel_tp.is_some_and(|i| i >= self.doc.toolpaths.len()) {
            self.sel_tp = None;
        }
        self.invalidate_sim();
    }

    pub fn selected_vectors(&self) -> Vec<&Vector> {
        self.doc.vectors.iter().filter(|v| self.sel.contains(&v.id)).collect()
    }

    pub fn selected_polys(&self) -> Vec<Poly> {
        self.selected_vectors().iter().flat_map(|v| v.polys()).collect()
    }

    pub fn selection_bounds(&self) -> Option<Rect> {
        let mut r = Rect::NOTHING;
        for v in self.selected_vectors() {
            r = r.union(v.bounds());
        }
        r.is_positive().then_some(r)
    }

    pub fn add_vector(&mut self, kind: ShapeKind, center: Pos2) -> u64 {
        let id = self.doc.id();
        let layer = self.active_layer.min(self.doc.layers.len().saturating_sub(1));
        self.doc.vectors.push(Vector::new(id, kind, center, layer));
        id
    }

    pub fn snap_pt(&self, p: Pos2) -> Pos2 {
        if self.snap {
            pos2((p.x / 5.0).round() * 5.0, (p.y / 5.0).round() * 5.0)
        } else {
            p
        }
    }

    pub fn toast(&mut self, s: impl Into<String>) {
        self.status = s.into();
    }

    pub fn invalidate_sim(&mut self) {
        self.sim = None;
        self.sim_running = false;
    }

    /// Recreate the shaded model texture when the model changed.
    fn refresh_model_texture(&mut self, ctx: &egui::Context) {
        let key = (Arc::as_ptr(&self.doc.model.composite) as usize) ^ ((self.model_color as usize) << 1) ^ (self.doc.material.thickness.to_bits() as usize);
        if key == self.model_tex_key && self.model_tex.is_some() {
            return;
        }
        self.model_tex_key = key;
        let m = &self.doc.model;
        let g = m.grid;
        if m.composite.len() != g.len() {
            return;
        }
        let top = m.max_height();
        self.model_z = m.composite.iter().map(|h| (h - top).max(-self.doc.material.thickness)).collect();
        let base = match self.model_color {
            ModelColor::Wood => [0.86, 0.66, 0.42],
            ModelColor::Gold => [0.93, 0.74, 0.30],
            ModelColor::Slate => [0.62, 0.67, 0.74],
        };
        let img = crate::render3d::shade(&m.composite, g.nx, g.ny, g.cell, |_| base);
        match &mut self.model_tex {
            Some(t) => t.set(img, egui::TextureOptions::LINEAR),
            None => self.model_tex = Some(ctx.load_texture("model", img, egui::TextureOptions::LINEAR)),
        }
    }

    fn refresh_sim_texture(&mut self, ctx: &egui::Context) {
        let Some(sim) = &self.sim else { return };
        if sim.revision == self.sim_tex_rev && self.sim_tex.is_some() {
            return;
        }
        self.sim_tex_rev = sim.revision;
        let s = &sim.stock;
        let t = s.thickness.max(1.0);
        let img = crate::render3d::shade(&s.z, s.nx, s.ny, s.cell, |z| {
            if z > -0.001 {
                [0.90, 0.78, 0.60]
            } else {
                let f = (-z / t * 2.5).min(1.0);
                [0.84 - 0.16 * f, 0.66 - 0.18 * f, 0.44 - 0.16 * f]
            }
        });
        match &mut self.sim_tex {
            Some(tx) => tx.set(img, egui::TextureOptions::LINEAR),
            None => self.sim_tex = Some(ctx.load_texture("stock", img, egui::TextureOptions::LINEAR)),
        }
    }

    // ------------------------------------------------------------ files

    pub fn new_doc(&mut self) {
        self.push_undo();
        self.doc = Doc::new(Material::default());
        self.doc.model.rebuild();
        self.sel.clear();
        self.sel_comp = None;
        self.sel_tp = None;
        self.file = None;
        self.invalidate_sim();
        self.fit_2d = true;
        self.view = View::D2;
        self.left_tab = LeftTab::Design;
        self.form = Some(Form::Material);
        self.home = false;
        self.toast("New job: set the material size, then start drawing");
    }

    pub fn open_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new().add_filter("CarveCraft job", &["ccj"]).pick_file() {
            self.open_path(path);
        }
    }

    pub fn open_path(&mut self, path: PathBuf) {
        let res = std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|s| serde_json::from_str::<Doc>(&s).map_err(|e| e.to_string()));
        match res {
            Ok(mut d) => {
                d.after_load();
                self.push_undo();
                self.doc = d;
                self.after_doc_swap();
                self.sel.clear();
                self.fit_2d = true;
                self.toast(format!("Opened {}", path.display()));
                self.home = false;
                crate::start::remember(&mut self.recent, &path);
                self.file = Some(path);
            }
            Err(e) => self.toast(format!("Could not open the file: {e}")),
        }
    }

    pub fn save(&mut self, save_as: bool) {
        let path = match (&self.file, save_as) {
            (Some(p), false) => Some(p.clone()),
            _ => rfd::FileDialog::new().add_filter("CarveCraft job", &["ccj"]).set_file_name(format!("{}.ccj", self.doc.name)).save_file(),
        };
        if let Some(path) = path {
            if let Some(stem) = path.file_stem() {
                self.doc.name = stem.to_string_lossy().to_string();
            }
            let res = serde_json::to_string(&self.doc).map_err(|e| e.to_string()).and_then(|s| std::fs::write(&path, s).map_err(|e| e.to_string()));
            match res {
                Ok(()) => {
                    self.toast(format!("Saved {}", path.display()));
                    crate::start::remember(&mut self.recent, &path);
                    self.file = Some(path);
                }
                Err(e) => self.toast(format!("Could not save: {e}")),
            }
        }
    }

    pub fn export_gcode(&mut self, only: Option<usize>) {
        let paths: Vec<&Toolpath> = match only {
            Some(i) => self.doc.toolpaths.get(i).into_iter().collect(),
            None => self.doc.toolpaths.iter().filter(|t| t.visible).collect(),
        };
        if paths.is_empty() {
            self.toast("No toolpaths to save – calculate one first");
            return;
        }
        let code = toolpath::gcode(&paths, &self.doc.material);
        if let Some(path) = rfd::FileDialog::new().add_filter("G-code", &["nc", "gcode", "tap"]).set_file_name(format!("{}.nc", self.doc.name)).save_file() {
            match std::fs::write(&path, code) {
                Ok(()) => self.toast(format!("G-code saved to {}", path.display())),
                Err(e) => self.toast(format!("Could not save G-code: {e}")),
            }
        }
    }

    pub fn export_stl(&mut self) {
        let Some(path) = rfd::FileDialog::new().add_filter("STL mesh", &["stl"]).set_file_name(format!("{}.stl", self.doc.name)).save_file() else { return };
        let m = &self.doc.model;
        let g = m.grid;
        let v = |i: usize, j: usize| {
            let p = g.pos(i, j);
            [p.x, p.y, m.composite[j * g.nx + i]]
        };
        let mut tris: Vec<[[f32; 3]; 3]> = Vec::new();
        for j in 0..g.ny - 1 {
            for i in 0..g.nx - 1 {
                tris.push([v(i, j), v(i + 1, j), v(i + 1, j + 1)]);
                tris.push([v(i, j), v(i + 1, j + 1), v(i, j + 1)]);
            }
        }
        let mut bytes = vec![0u8; 80];
        bytes.extend_from_slice(&(tris.len() as u32).to_le_bytes());
        for t in &tris {
            let a = [t[1][0] - t[0][0], t[1][1] - t[0][1], t[1][2] - t[0][2]];
            let b = [t[2][0] - t[0][0], t[2][1] - t[0][1], t[2][2] - t[0][2]];
            let n = [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
            let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-9);
            for x in [n[0] / l, n[1] / l, n[2] / l] {
                bytes.extend_from_slice(&x.to_le_bytes());
            }
            for p in t {
                for x in p {
                    bytes.extend_from_slice(&x.to_le_bytes());
                }
            }
            bytes.extend_from_slice(&[0, 0]);
        }
        match std::fs::write(&path, bytes) {
            Ok(()) => self.toast(format!("3D model saved to {}", path.display())),
            Err(e) => self.toast(format!("Could not save: {e}")),
        }
    }

    pub fn import_heightmap(&mut self) {
        let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png"]).pick_file() else { return };
        match image::open(&path) {
            Ok(img) => {
                let l = img.to_luma32f();
                let (w, h) = (l.width() as usize, l.height() as usize);
                let mut data = vec![0.0f32; w * h];
                for y in 0..h {
                    for x in 0..w {
                        data[(h - 1 - y) * w + x] = l.get_pixel(x as u32, y as u32).0[0];
                    }
                }
                let rect = self.selection_bounds().unwrap_or(toolpath::material_rect(&self.doc.material).shrink(10.0));
                let name = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or("Height map".into());
                self.add_component(name, Source::Image { w, h, data, rect, height: self.p.img_height }, self.p.mode);
                self.form = None;
            }
            Err(e) => self.toast(format!("Could not read the picture: {e}")),
        }
    }

    // ------------------------------------------------------------ edits

    pub fn delete_selected(&mut self) {
        if self.sel.is_empty() {
            return;
        }
        self.push_undo();
        let n = self.sel.len();
        let sel = self.sel.clone();
        self.doc.vectors.retain(|v| !sel.contains(&v.id));
        self.sel.clear();
        self.toast(format!("Deleted {n} vector(s)"));
    }

    pub fn duplicate_selected(&mut self, offset: egui::Vec2) {
        if self.sel.is_empty() {
            return;
        }
        self.push_undo();
        let copies: Vec<Vector> = self.selected_vectors().into_iter().cloned().collect();
        self.sel.clear();
        for mut v in copies {
            v.id = self.doc.id();
            v.center += offset;
            self.sel.push(v.id);
            self.doc.vectors.push(v);
        }
    }

    pub fn transform_selected(&mut self, f: impl Fn(&mut Vector, Pos2)) {
        let Some(b) = self.selection_bounds() else { return };
        self.push_undo();
        let c = b.center();
        let sel = self.sel.clone();
        for v in self.doc.vectors.iter_mut().filter(|v| sel.contains(&v.id)) {
            f(v, c);
        }
    }

    pub fn apply_offset(&mut self) {
        let polys: Vec<Poly> = self.selected_polys().into_iter().filter(|p| p.closed).collect();
        if polys.is_empty() {
            self.toast("Select a closed shape to offset");
            return;
        }
        self.push_undo();
        let d = if self.p.outward { self.p.offset } else { -self.p.offset };
        let res = geom::offset_polys(&polys, d);
        let mut new = Vec::new();
        for l in res {
            new.push(self.add_path(l.pts, true));
        }
        self.sel = new;
        self.toast("Offset created");
    }

    pub fn apply_weld(&mut self) {
        let groups: Vec<Vec<Poly>> = self.selected_vectors().iter().filter(|v| v.is_closed()).map(|v| v.polys()).collect();
        if groups.len() < 2 {
            self.toast("Select two or more closed shapes to weld");
            return;
        }
        self.push_undo();
        let sel = self.sel.clone();
        let layer = self.selected_vectors().first().map(|v| v.layer).unwrap_or(self.active_layer);
        self.doc.vectors.retain(|v| !(sel.contains(&v.id) && v.is_closed()));
        let mut new = Vec::new();
        for l in geom::weld(&groups) {
            let id = self.add_path(l.pts, true);
            if let Some(v) = self.doc.vectors.iter_mut().find(|v| v.id == id) {
                v.layer = layer;
            }
            new.push(id);
        }
        self.toast(format!("Welded {} shapes into {}", groups.len(), new.len()));
        self.sel = new;
    }

    /// Round every corner of the selected shapes.
    pub fn apply_fillet_all(&mut self) {
        if self.sel.is_empty() {
            self.toast("Select shapes to round their corners");
            return;
        }
        self.push_undo();
        let (sel, r) = (self.sel.clone(), self.p.fillet_r);
        let mut skipped = 0;
        for v in self.doc.vectors.iter_mut().filter(|v| sel.contains(&v.id)) {
            if !v.to_path() {
                skipped += 1;
                continue;
            }
            if let ShapeKind::Path { pts, closed } = &mut v.kind {
                *pts = geom::fillet_all(pts, *closed, r);
            }
        }
        self.toast(if skipped > 0 { format!("Corners rounded ({skipped} multi-outline shape(s) skipped)") } else { "Corners rounded".to_string() });
    }

    pub fn apply_circular_array(&mut self) {
        if self.sel.is_empty() {
            self.toast("Select something to copy around the centre of the job");
            return;
        }
        self.push_undo();
        let center = pos2(self.doc.material.w * 0.5, self.doc.material.h * 0.5);
        let originals: Vec<Vector> = self.selected_vectors().into_iter().cloned().collect();
        let n = self.p.arr_count.max(2);
        let step = if (self.p.arr_angle - 360.0).abs() < 0.01 { 360.0 / n as f32 } else { self.p.arr_angle / (n - 1) as f32 };
        for k in 1..n {
            let a = (step * k as f32).to_radians();
            for o in &originals {
                let mut v = o.clone();
                v.id = self.doc.id();
                let d = v.center - center;
                v.center = center + vec2(d.x * a.cos() - d.y * a.sin(), d.x * a.sin() + d.y * a.cos());
                v.angle += step * k as f32;
                self.doc.vectors.push(v);
            }
        }
        self.toast(format!("Made {} copies around the job centre", n - 1));
    }

    pub fn apply_grid_array(&mut self) {
        let Some(b) = self.selection_bounds() else {
            self.toast("Select something to copy in a grid");
            return;
        };
        self.push_undo();
        let originals: Vec<Vector> = self.selected_vectors().into_iter().cloned().collect();
        for r in 0..self.p.grid_rows.max(1) {
            for c in 0..self.p.grid_cols.max(1) {
                if r == 0 && c == 0 {
                    continue;
                }
                let off = vec2(c as f32 * (b.width() + self.p.grid_gap), r as f32 * (b.height() + self.p.grid_gap));
                for o in &originals {
                    let mut v = o.clone();
                    v.id = self.doc.id();
                    v.center += off;
                    self.doc.vectors.push(v);
                }
            }
        }
    }

    // ------------------------------------------------------------ 3D

    pub fn add_component(&mut self, name: String, src: Source, mode: Combine) {
        self.push_undo();
        let id = self.doc.id();
        self.doc.model.components.push(Component::new(id, name, src, mode));
        self.doc.model.rebuild();
        self.sel_comp = Some(self.doc.model.components.len() - 1);
        self.invalidate_sim();
        self.toast("3D component added");
    }

    pub fn apply_model_form(&mut self, f: Form) {
        let polys = self.selected_polys();
        let closed: Vec<Poly> = polys.iter().filter(|p| p.closed).cloned().collect();
        let n = self.doc.model.components.len() + 1;
        let (profile, height, angle, base, limit, mode) = (self.p.profile, self.p.height, self.p.angle, self.p.base, self.p.limit, self.p.mode);
        match f {
            Form::Shape => {
                if closed.is_empty() {
                    self.toast("Select one or more closed vectors first");
                    return;
                }
                let name = if profile == Profile::Angle { format!("Prism {n}") } else { format!("Shape {n}") };
                self.add_component(name, Source::Shape { polys: closed, profile, height, angle, base, limit }, mode);
            }
            Form::Extrude => {
                if polys.is_empty() {
                    self.toast("Select a vector to sweep along");
                    return;
                }
                let src = Source::Extrude { polys, profile, width: self.p.ext_width, height };
                self.add_component(format!("Extrusion {n}"), src, mode);
            }
            Form::TwoRail => {
                let v = self.selected_vectors();
                if v.len() != 2 {
                    self.toast("Select exactly two vectors to use as rails");
                    return;
                }
                let a = v[0].polys().remove(0);
                let b = v[1].polys().remove(0);
                self.add_component(format!("Sweep {n}"), Source::TwoRail { a, b, profile, height }, mode);
            }
            Form::Texture => {
                let src = Source::Texture { polys: closed, kind: self.p.tex_kind, depth: self.p.tex_depth, size: self.p.tex_size };
                self.add_component(format!("{} texture", self.p.tex_kind.label()), src, mode);
            }
            Form::Relief => {
                let rect = self.selection_bounds().unwrap_or_else(|| {
                    Rect::from_center_size(pos2(self.doc.material.w * 0.5, self.doc.material.h * 0.5), vec2(self.p.relief_size, self.p.relief_size))
                });
                let src = Source::Relief { kind: self.p.relief, rect, height: self.p.relief_height };
                self.add_component(self.p.relief.label().to_string(), src, mode);
            }
            _ => {}
        }
        self.form = None;
    }

    // ------------------------------------------------------------ toolpaths

    pub fn calculate_pending(&mut self) {
        let Some(op) = self.pending_op.clone() else { return };
        let tool = self.tools[self.tool_idx].clone();
        let polys = self.selected_polys();
        if op.needs_vectors() && polys.is_empty() {
            self.toast("Select the vectors to machine first");
            return;
        }
        if matches!(op, Op::VCarve { .. }) && tool.kind != ToolKind::VBit {
            self.toast("V-Carving needs a V-bit – pick one in the tool list");
            return;
        }
        self.push_undo();
        let id = self.doc.id();
        let n = self.doc.toolpaths.len() + 1;
        let mut tp = Toolpath { id, name: format!("{} {}", op.label(), n), op, tool, polys, visible: true, chains: Arc::new(Vec::new()), length: 0.0, minutes: 0.0 };
        tp.calculate(&self.doc.model, &self.doc.material);
        let msg = format!("{} calculated – {} moves, about {:.0} min", tp.name, tp.chains.len(), tp.minutes.max(1.0));
        self.doc.toolpaths.push(tp);
        self.sel_tp = Some(self.doc.toolpaths.len() - 1);
        self.pending_op = None;
        self.invalidate_sim();
        self.toast(msg);
    }

    pub fn start_sim(&mut self, animate: bool) {
        let paths: Vec<&Toolpath> = self.doc.toolpaths.iter().filter(|t| t.visible).collect();
        if paths.is_empty() {
            self.toast("Nothing to preview – calculate a toolpath first");
            return;
        }
        let mut sim = Sim::new(&self.doc.material, &paths);
        if !animate {
            sim.finish();
        }
        self.sim = Some(sim);
        self.sim_running = animate;
        self.show_sim = true;
        self.view = View::D3;
    }

    // ------------------------------------------------------------ frame

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() {
            return;
        }
        use egui::{Key, Modifiers};
        let mut cmds = Vec::new();
        ctx.input_mut(|i| {
            let table = [
                (Modifiers::COMMAND, Key::Z, "undo"),
                (Modifiers::COMMAND, Key::Y, "redo"),
                (Modifiers::COMMAND, Key::S, "save"),
                (Modifiers::COMMAND, Key::O, "open"),
                (Modifiers::COMMAND, Key::D, "dup"),
                (Modifiers::COMMAND, Key::A, "all"),
                (Modifiers::NONE, Key::Delete, "del"),
                (Modifiers::NONE, Key::Escape, "esc"),
                (Modifiers::NONE, Key::Enter, "enter"),
                (Modifiers::NONE, Key::F2, "2d"),
                (Modifiers::NONE, Key::F3, "3d"),
            ];
            for (m, k, c) in table {
                if i.consume_key(m, k) {
                    cmds.push(c);
                }
            }
        });
        for c in cmds {
            match c {
                "undo" => self.do_undo(),
                "redo" => self.do_redo(),
                "save" => self.save(false),
                "open" => self.open_dialog(),
                "dup" => self.duplicate_selected(vec2(10.0, -10.0)),
                "all" => self.sel = self.doc.vectors.iter().map(|v| v.id).collect(),
                "del" => self.delete_selected(),
                "esc" => {
                    self.poly_pts.clear();
                    self.form = None;
                    self.pending_op = None;
                    self.tool = DrawTool::Select;
                }
                "enter" => self.finish_polyline(false),
                "2d" => self.view = View::D2,
                "3d" => self.view = View::D3,
                _ => {}
            }
        }
    }

    pub fn finish_polyline(&mut self, closed: bool) {
        if self.poly_pts.len() < 2 {
            self.poly_pts.clear();
            return;
        }
        let pts = std::mem::take(&mut self.poly_pts);
        self.push_undo();
        let id = self.add_path(pts, closed);
        self.sel = vec![id];
    }

    pub fn add_path(&mut self, pts: Vec<Pos2>, closed: bool) -> u64 {
        let mut b = Rect::NOTHING;
        for p in &pts {
            b.extend_with(*p);
        }
        let c = b.center();
        let local = pts.iter().map(|p| pos2(p.x - c.x, p.y - c.y)).collect();
        self.add_vector(ShapeKind::Path { pts: local, closed }, c)
    }

    fn handle_screenshot(&mut self, ctx: &egui::Context) {
        let Some(path) = self.screenshot.clone() else { return };
        self.frames += 1;
        ctx.request_repaint();
        if self.frames >= 120 && !self.shot_sent {
            self.shot_sent = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        }
        let shot = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(img) = shot {
            let [w, h] = img.size;
            let raw: Vec<u8> = img.pixels.iter().flat_map(|c| [c.r(), c.g(), c.b(), 255]).collect();
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            match image::RgbaImage::from_raw(w as u32, h as u32, raw) {
                Some(buf) => {
                    if let Err(e) = buf.save(&path) {
                        eprintln!("could not save screenshot: {e}");
                    }
                }
                None => eprintln!("bad screenshot buffer"),
            }
            std::process::exit(0);
        }
        if self.frames > 900 {
            eprintln!("screenshot timed out");
            std::process::exit(1);
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.applied_dark != Some(self.dark) {
            theme::apply(ctx, self.dark);
            self.applied_dark = Some(self.dark);
        }
        if !ctx.input(|i| i.pointer.any_down()) && ctx.memory(|m| m.focused().is_none()) {
            self.editing = false;
        }
        self.shortcuts(ctx);
        self.refresh_model_texture(ctx);
        if self.sim_running {
            if let Some(sim) = &mut self.sim {
                // speed 1..=10: slow enough to watch, fast enough not to wait
                sim.advance((250_000.0 * self.sim_speed.powi(2)) as usize);
                if sim.done {
                    self.sim_running = false;
                }
            }
            ctx.request_repaint();
        }
        self.refresh_sim_texture(ctx);

        self.top_bar(ctx);
        self.status_bar(ctx);
        let fill = ctx.style().visuals.panel_fill;
        self.guide_window(ctx);
        if self.home {
            egui::SidePanel::left("start")
                .exact_width(304.0)
                .resizable(false)
                .frame(egui::Frame::new().fill(fill).inner_margin(egui::Margin::symmetric(14, 8)))
                .show(ctx, |ui| self.start_panel(ui));
            let pal = theme::palette(self.dark);
            egui::CentralPanel::default().frame(egui::Frame::new().fill(pal.canvas)).show(ctx, |ui| self.welcome(ui));
            self.handle_screenshot(ctx);
            return;
        }
        egui::SidePanel::left("left")
            .exact_width(304.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(fill).inner_margin(egui::Margin::symmetric(10, 8)))
            .show(ctx, |ui| self.left_panel(ui));
        egui::SidePanel::right("right")
            .exact_width(304.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(fill).inner_margin(egui::Margin::symmetric(10, 8)))
            .show(ctx, |ui| self.right_panel(ui));
        let pal = theme::palette(self.dark);
        egui::CentralPanel::default().frame(egui::Frame::new().fill(pal.canvas)).show(ctx, |ui| {
            self.view_toolbar(ui);
            match self.view {
                View::D2 => self.view_2d(ui),
                View::D3 => self.view_3d(ui),
            }
        });
        self.handle_screenshot(ctx);
    }
}
