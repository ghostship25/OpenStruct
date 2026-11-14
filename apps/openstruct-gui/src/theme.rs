//! OpenStruct colour tokens and style installation.
//!
//! The palette follows engineering-desktop look: graphite chrome, a darker
//! drawing canvas, a blue accent for selection and an amber highlight for the
//! active object. Tokens are central so every panel, table and canvas reads the
//! same values and the theme switch applies everywhere.

use eframe::egui::{self, Color32, Stroke};
use std::sync::Arc;

#[derive(Clone, Copy)]
pub struct Palette {
    pub panel: Color32,
    pub panel_alt: Color32,
    pub canvas: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub accent: Color32,
    pub accent_soft: Color32,
    pub selection: Color32,
    pub grid: Color32,
    pub joint: Color32,
    pub frame: Color32,
    pub deformation: Color32,
    pub load: Color32,
    pub ok: Color32,
    pub warn: Color32,
    pub border: Color32,
}

pub fn palette(dark: bool) -> Palette {
    if dark {
        Palette {
            panel: Color32::from_rgb(0x28, 0x2a, 0x2e),
            panel_alt: Color32::from_rgb(0x31, 0x34, 0x38),
            canvas: Color32::from_rgb(0x0e, 0x11, 0x15),
            text: Color32::from_rgb(0xdc, 0xdf, 0xe4),
            text_dim: Color32::from_rgb(0x94, 0x9a, 0xa3),
            accent: Color32::from_rgb(0x2d, 0x81, 0xff),
            accent_soft: Color32::from_rgba_unmultiplied(0x2d, 0x81, 0xff, 48),
            selection: Color32::from_rgb(0xff, 0xb0, 0x20),
            grid: Color32::from_rgb(0x45, 0x51, 0x63),
            joint: Color32::from_rgb(0xff, 0xd6, 0x3a),
            frame: Color32::from_rgb(0x6c, 0xb4, 0xff),
            deformation: Color32::from_rgb(0xff, 0x5c, 0x5c),
            load: Color32::from_rgb(0xff, 0x9f, 0x43),
            ok: Color32::from_rgb(0x5b, 0xbf, 0x8f),
            warn: Color32::from_rgb(0xe0, 0xa2, 0x3a),
            border: Color32::from_rgb(0x3a, 0x3d, 0x42),
        }
    } else {
        Palette {
            panel: Color32::from_rgb(0xf2, 0xf3, 0xf5),
            panel_alt: Color32::from_rgb(0xe4, 0xe6, 0xea),
            canvas: Color32::from_rgb(0xfa, 0xfb, 0xfc),
            text: Color32::from_rgb(0x18, 0x1c, 0x22),
            text_dim: Color32::from_rgb(0x55, 0x5d, 0x68),
            accent: Color32::from_rgb(0x16, 0x59, 0xc4),
            accent_soft: Color32::from_rgba_unmultiplied(0x2d, 0x81, 0xff, 32),
            selection: Color32::from_rgb(0xb4, 0x62, 0x00),
            grid: Color32::from_rgb(0xb9, 0xc2, 0xcf),
            joint: Color32::from_rgb(0xb8, 0x6e, 0x00),
            frame: Color32::from_rgb(0x1f, 0x4f, 0x9a),
            deformation: Color32::from_rgb(0xc0, 0x2b, 0x2b),
            load: Color32::from_rgb(0xb4, 0x5f, 0x00),
            ok: Color32::from_rgb(0x12, 0x75, 0x36),
            warn: Color32::from_rgb(0x9a, 0x67, 0x00),
            border: Color32::from_rgb(0xc3, 0xc8, 0xcf),
        }
    }
}

/// Install the OpenStruct style as both the dark and the light style so the app
/// keeps its look regardless of the operating system theme.
pub fn install(ctx: &egui::Context, dark: bool) {
    let p = palette(dark);
    let mut style = egui::Style::default();
    style.visuals = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    let v = &mut style.visuals;
    v.override_text_color = Some(p.text);
    v.panel_fill = p.panel;
    v.window_fill = p.panel;
    v.extreme_bg_color = p.canvas;
    v.faint_bg_color = p.panel_alt;
    v.code_bg_color = p.panel_alt;
    v.window_stroke = Stroke::new(1.0, p.border);
    v.selection.bg_fill = p.accent;
    v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    v.widgets.noninteractive.bg_fill = p.panel;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text_dim);
    v.widgets.inactive.bg_fill = p.panel_alt;
    v.widgets.inactive.weak_bg_fill = p.panel_alt;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, p.border);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, p.text);
    v.widgets.hovered.bg_fill = p.accent_soft;
    v.widgets.hovered.weak_bg_fill = p.accent_soft;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, p.accent);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, p.text);
    v.widgets.active.bg_fill = p.accent;
    v.widgets.active.weak_bg_fill = p.accent;
    v.widgets.active.bg_stroke = Stroke::new(1.0, p.accent);
    v.widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    style.spacing.item_spacing = egui::vec2(6.0, 4.0);
    style.spacing.button_padding = egui::vec2(7.0, 3.0);

    let style = Arc::new(style);
    ctx.options_mut(|o| {
        o.dark_style = style.clone();
        o.light_style = style;
    });
}
