//! Modern dark / light look.

use egui::{Color32, CornerRadius, Stroke, Visuals};

pub const ACCENT: Color32 = Color32::from_rgb(61, 139, 253);

pub struct Palette {
    pub canvas: Color32,
    pub sheet: Color32,
    pub card: Color32,
    pub tile: Color32,
    pub tile_hover: Color32,
    pub text: Color32,
    pub weak: Color32,
    pub vector: Color32,
    pub grid: Color32,
    pub grid_major: Color32,
    pub border: Color32,
}

pub fn palette(dark: bool) -> Palette {
    if dark {
        Palette {
            canvas: Color32::from_rgb(18, 20, 25),
            sheet: Color32::from_rgb(40, 44, 52),
            card: Color32::from_rgb(33, 36, 43),
            tile: Color32::from_rgb(43, 47, 56),
            tile_hover: Color32::from_rgb(54, 59, 70),
            text: Color32::from_rgb(226, 230, 237),
            weak: Color32::from_rgb(140, 148, 162),
            vector: Color32::from_rgb(235, 238, 243),
            grid: Color32::from_rgb(48, 53, 62),
            grid_major: Color32::from_rgb(62, 68, 80),
            border: Color32::from_rgb(52, 57, 67),
        }
    } else {
        Palette {
            canvas: Color32::from_rgb(226, 230, 236),
            sheet: Color32::from_rgb(255, 255, 255),
            card: Color32::from_rgb(246, 247, 250),
            tile: Color32::from_rgb(234, 237, 242),
            tile_hover: Color32::from_rgb(220, 226, 235),
            text: Color32::from_rgb(30, 34, 42),
            weak: Color32::from_rgb(105, 113, 126),
            vector: Color32::from_rgb(25, 28, 35),
            grid: Color32::from_rgb(234, 237, 242),
            grid_major: Color32::from_rgb(214, 219, 227),
            border: Color32::from_rgb(214, 219, 227),
        }
    }
}

pub fn apply(ctx: &egui::Context, dark: bool) {
    let p = palette(dark);
    let mut v = if dark { Visuals::dark() } else { Visuals::light() };
    v.panel_fill = if dark { Color32::from_rgb(27, 30, 36) } else { Color32::from_rgb(252, 252, 253) };
    v.window_fill = p.card;
    v.extreme_bg_color = if dark { Color32::from_rgb(22, 24, 29) } else { Color32::WHITE };
    v.faint_bg_color = p.card;
    v.window_corner_radius = CornerRadius::same(10);
    v.menu_corner_radius = CornerRadius::same(8);
    v.window_stroke = Stroke::new(1.0_f32, p.border);
    v.selection.bg_fill = ACCENT.gamma_multiply(if dark { 0.55 } else { 0.35 });
    v.selection.stroke = Stroke::new(1.0_f32, ACCENT);
    v.hyperlink_color = ACCENT;
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.noninteractive, &mut v.widgets.open] {
        w.corner_radius = CornerRadius::same(6);
    }
    v.widgets.inactive.bg_fill = p.tile;
    v.widgets.inactive.weak_bg_fill = p.tile;
    v.widgets.inactive.bg_stroke = Stroke::NONE;
    v.widgets.hovered.weak_bg_fill = p.tile_hover;
    v.widgets.hovered.bg_fill = p.tile_hover;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, ACCENT.gamma_multiply(0.6));
    v.widgets.active.weak_bg_fill = ACCENT.gamma_multiply(0.7);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.border);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, p.text);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, p.text);
    ctx.set_visuals(v);
    ctx.style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 6.0);
        s.spacing.button_padding = egui::vec2(10.0, 5.0);
        s.spacing.interact_size.y = 26.0;
        s.spacing.slider_width = 120.0;
        s.text_styles.insert(egui::TextStyle::Body, egui::FontId::proportional(13.5));
        s.text_styles.insert(egui::TextStyle::Button, egui::FontId::proportional(13.5));
        s.text_styles.insert(egui::TextStyle::Small, egui::FontId::proportional(11.0));
        s.text_styles.insert(egui::TextStyle::Heading, egui::FontId::proportional(17.0));
    });
}
