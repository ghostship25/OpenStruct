//! In-house vector toolbar icons.
//!
//! Drawn with the egui painter at 24-unit design coordinates, so there are no
//! raster assets to copy, license or keep in sync. Each icon is a handful of
//! lines/polygons and stays crisp at any DPI.

use eframe::egui::{self, Color32, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Icon {
    New,
    Open,
    Save,
    Undo,
    Redo,
    Select,
    Joint,
    Frame,
    Area,
    Wall,
    Delete,
    ZoomFit,
    Grid,
    Run,
    Deformed,
    Table,
    Fixed,
    Pinned,
    Apply,
}

fn pt(rect: Rect, x: f32, y: f32) -> Pos2 {
    Pos2::new(rect.left() + x * rect.width(), rect.top() + y * rect.height())
}

fn line(painter: &egui::Painter, rect: Rect, pts: &[(f32, f32)], color: Color32, width: f32) {
    let points: Vec<Pos2> = pts.iter().map(|&(x, y)| pt(rect, x, y)).collect();
    painter.add(egui::Shape::line(points, Stroke::new(width, color)));
}

fn poly(painter: &egui::Painter, rect: Rect, pts: &[(f32, f32)], color: Color32) {
    let points: Vec<Pos2> = pts.iter().map(|&(x, y)| pt(rect, x, y)).collect();
    painter.add(egui::Shape::convex_polygon(points, color, Stroke::new(0.0, Color32::TRANSPARENT)));
}

pub fn draw_icon(painter: &egui::Painter, rect: Rect, icon: Icon, color: Color32) {
    let w = 1.5;
    match icon {
        Icon::New => {
            line(painter, rect, &[(0.26, 0.1), (0.62, 0.1), (0.78, 0.28), (0.78, 0.9), (0.26, 0.9), (0.26, 0.1)], color, w);
            line(painter, rect, &[(0.62, 0.1), (0.62, 0.28), (0.78, 0.28)], color, w);
        }
        Icon::Open => {
            line(painter, rect, &[(0.14, 0.32), (0.4, 0.32), (0.48, 0.42), (0.86, 0.42), (0.86, 0.86), (0.14, 0.86), (0.14, 0.32)], color, w);
        }
        Icon::Save => {
            line(painter, rect, &[(0.2, 0.12), (0.74, 0.12), (0.86, 0.26), (0.86, 0.88), (0.2, 0.88), (0.2, 0.12)], color, w);
            line(painter, rect, &[(0.34, 0.12), (0.34, 0.38), (0.68, 0.38), (0.68, 0.12)], color, w);
            line(painter, rect, &[(0.32, 0.88), (0.32, 0.6), (0.74, 0.6), (0.74, 0.88)], color, w);
        }
        Icon::Undo => {
            line(painter, rect, &[(0.24, 0.48), (0.36, 0.3), (0.58, 0.28), (0.74, 0.42), (0.76, 0.62), (0.62, 0.74), (0.4, 0.7)], color, w);
            poly(painter, rect, &[(0.24, 0.48), (0.36, 0.38), (0.32, 0.62)], color);
        }
        Icon::Redo => {
            line(painter, rect, &[(0.76, 0.48), (0.64, 0.3), (0.42, 0.28), (0.26, 0.42), (0.24, 0.62), (0.38, 0.74), (0.6, 0.7)], color, w);
            poly(painter, rect, &[(0.76, 0.48), (0.64, 0.38), (0.68, 0.62)], color);
        }
        Icon::Select => {
            poly(painter, rect, &[(0.34, 0.12), (0.34, 0.82), (0.5, 0.64), (0.6, 0.88), (0.72, 0.8), (0.62, 0.58), (0.8, 0.56)], color);
        }
        Icon::Joint => {
            let c = pt(rect, 0.5, 0.5);
            let r = rect.width() * 0.14;
            painter.circle_filled(c, r, color);
            painter.circle_stroke(c, r * 2.0, Stroke::new(1.2, color));
        }
        Icon::Frame => {
            let a = pt(rect, 0.2, 0.8);
            let b = pt(rect, 0.8, 0.2);
            painter.line_segment([a, b], Stroke::new(2.0, color));
            painter.circle_filled(a, rect.width() * 0.1, color);
            painter.circle_filled(b, rect.width() * 0.1, color);
        }
        Icon::Area => {
            line(painter, rect, &[(0.18, 0.3), (0.82, 0.3), (0.82, 0.82), (0.18, 0.82), (0.18, 0.3)], color, w);
            line(painter, rect, &[(0.18, 0.3), (0.82, 0.82)], color, 1.0);
            line(painter, rect, &[(0.18, 0.82), (0.82, 0.3)], color, 1.0);
        }
        Icon::Wall => {
            line(painter, rect, &[(0.22, 0.86), (0.22, 0.24), (0.62, 0.14), (0.62, 0.76), (0.22, 0.86)], color, w);
            line(painter, rect, &[(0.62, 0.76), (0.84, 0.66), (0.84, 0.14), (0.62, 0.24)], color, 1.0);
            line(painter, rect, &[(0.3, 0.74), (0.3, 0.33)], color, 1.0);
            line(painter, rect, &[(0.42, 0.72), (0.42, 0.3)], color, 1.0);
            line(painter, rect, &[(0.54, 0.69), (0.54, 0.27)], color, 1.0);
        }
        Icon::Delete => {
            line(painter, rect, &[(0.18, 0.26), (0.82, 0.26)], color, w);
            line(painter, rect, &[(0.4, 0.16), (0.6, 0.16)], color, w);
            line(painter, rect, &[(0.28, 0.26), (0.32, 0.9), (0.68, 0.9), (0.72, 0.26)], color, w);
            line(painter, rect, &[(0.44, 0.38), (0.44, 0.78)], color, 1.1);
            line(painter, rect, &[(0.56, 0.38), (0.56, 0.78)], color, 1.1);
        }
        Icon::ZoomFit => {
            let c = pt(rect, 0.44, 0.42);
            painter.circle_stroke(c, rect.width() * 0.24, Stroke::new(1.6, color));
            painter.line_segment([pt(rect, 0.62, 0.62), pt(rect, 0.86, 0.86)], Stroke::new(2.0, color));
            line(painter, rect, &[(0.3, 0.42), (0.44, 0.42), (0.44, 0.3)], color, 1.0);
        }
        Icon::Grid => {
            for i in 0..4 {
                let t = 0.16 + i as f32 * 0.226;
                line(painter, rect, &[(t, 0.16), (t, 0.84)], color, 1.0);
                line(painter, rect, &[(0.16, t), (0.84, t)], color, 1.0);
            }
        }
        Icon::Run => {
            poly(painter, rect, &[(0.32, 0.16), (0.32, 0.84), (0.84, 0.5)], color);
        }
        Icon::Deformed => {
            line(painter, rect, &[(0.14, 0.8), (0.86, 0.8)], Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 90), 1.0);
            line(painter, rect, &[(0.14, 0.78), (0.34, 0.5), (0.54, 0.64), (0.72, 0.34), (0.86, 0.42)], color, w);
        }
        Icon::Table => {
            line(painter, rect, &[(0.16, 0.2), (0.84, 0.2), (0.84, 0.82), (0.16, 0.82), (0.16, 0.2)], color, w);
            line(painter, rect, &[(0.16, 0.4), (0.84, 0.4)], color, 1.0);
            line(painter, rect, &[(0.16, 0.61), (0.84, 0.61)], color, 1.0);
            line(painter, rect, &[(0.42, 0.2), (0.42, 0.82)], color, 1.0);
        }
        Icon::Fixed => {
            line(painter, rect, &[(0.14, 0.78), (0.86, 0.78)], color, 2.0);
            poly(painter, rect, &[(0.5, 0.24), (0.28, 0.78), (0.72, 0.78)], color);
            line(painter, rect, &[(0.2, 0.9), (0.3, 0.78)], color, 1.0);
            line(painter, rect, &[(0.4, 0.9), (0.5, 0.78)], color, 1.0);
            line(painter, rect, &[(0.6, 0.9), (0.7, 0.78)], color, 1.0);
            line(painter, rect, &[(0.78, 0.9), (0.86, 0.78)], color, 1.0);
        }
        Icon::Pinned => {
            line(painter, rect, &[(0.16, 0.82), (0.84, 0.82)], color, 1.6);
            line(painter, rect, &[(0.5, 0.66), (0.28, 0.82)], color, w);
            line(painter, rect, &[(0.5, 0.66), (0.72, 0.82)], color, w);
            painter.circle_stroke(pt(rect, 0.5, 0.6), rect.width() * 0.08, Stroke::new(1.4, color));
        }
        Icon::Apply => {
            line(painter, rect, &[(0.2, 0.54), (0.42, 0.76), (0.82, 0.26)], color, 2.2);
        }
    }
}

/// A compact square icon button with hover/active states and a tooltip.
pub fn icon_button(ui: &mut Ui, icon: Icon, tooltip: &str, active: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(28.0, 24.0), Sense::click());
    let visuals = ui.visuals().clone();
    let color = if active {
        visuals.selection.stroke.color
    } else if response.hovered() {
        visuals.strong_text_color()
    } else {
        visuals.text_color()
    };
    let painter = ui.painter();
    if active {
        painter.rect_filled(rect, egui::CornerRadius::same(3), visuals.selection.bg_fill);
    } else if response.hovered() {
        painter.rect_filled(rect, egui::CornerRadius::same(3), visuals.widgets.hovered.bg_fill);
    }
    let inner = Rect::from_center_size(rect.center(), Vec2::splat(16.0));
    draw_icon(painter, inner, icon, color);
    response.on_hover_text(tooltip)
}
