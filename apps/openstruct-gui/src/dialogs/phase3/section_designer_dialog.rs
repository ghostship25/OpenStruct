use crate::app::{DesignerPartKind, OpenStructApp};
use eframe::egui;
use openstruct::sections::{DesignerPart, Section};

/// Draw a composite frame section from rectangles, circles, polygons and holes.
pub fn open_section_designer_dialog(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.section_designer_open {
        return;
    }
    let mut open = app.section_designer_open;
    let mut close = false;
    let mut apply = false;
    let mut clear = false;
    let mut finish_polygon = false;
    let mut finish_hole = false;
    let units = app.units;

    egui::Window::new("Section Designer…")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(780.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("New part:");
                ui.selectable_value(&mut app.designer_new_kind, DesignerPartKind::Rectangle, "Rectangle");
                ui.selectable_value(&mut app.designer_new_kind, DesignerPartKind::Circle, "Circle");
                ui.selectable_value(&mut app.designer_new_kind, DesignerPartKind::Polygon, "Polygon");
                if app.designer_new_kind == DesignerPartKind::Polygon {
                    ui.checkbox(&mut app.designer_hole_mode, "Into selected polygon as a hole");
                }
            });
            ui.horizontal(|ui| {
                ui.add(egui::Slider::new(&mut app.designer_view, 0.1..=5.0).text("view half-span (m)"));
                if ui.button("Clear").clicked() {
                    clear = true;
                }
                if ui
                    .add_enabled(app.designer_pending.len() >= 3, egui::Button::new(format!("Finish polygon ({})", app.designer_pending.len())))
                    .clicked()
                {
                    finish_polygon = true;
                }
                if ui
                    .add_enabled(app.designer_pending_hole.len() >= 3, egui::Button::new(format!("Finish hole ({})", app.designer_pending_hole.len())))
                    .clicked()
                {
                    finish_hole = true;
                }
                if ui.button("Cancel path").clicked() {
                    app.designer_pending.clear();
                    app.designer_pending_hole.clear();
                }
            });
            ui.label(
                egui::RichText::new(
                    "Drag on empty space to draw a rectangle or circle; with Polygon selected, click to add vertices then Finish polygon. Select a polygon and enable the hole option to clip a void, drag a vertex handle to reshape it, or use the arrow keys to nudge the selected vertex (Delete removes the part, Escape cancels a pending outline).",
                )
                .weak(),
            );
            if app.designer_hole_mode
                && !matches!(app.designer_selected.and_then(|index| app.designer_parts.get(index)), Some(DesignerPart::Polygon { .. }))
            {
                ui.label(
                    egui::RichText::new("Select a polygon part before drawing a hole; a hole must lie inside its outline.")
                        .color(egui::Color32::from_rgb(220, 140, 120)),
                );
            }

            let mut remove: Option<usize> = None;
            let size = egui::vec2(ui.available_width().max(320.0), 280.0);
            let (response, painter) = ui.allocate_painter(size, egui::Sense::click_and_drag());
            let rect = response.rect;
            let span = app.designer_view.max(0.05);
            let scale = rect.width().min(rect.height()) / (2.0 * span) as f32;
            let center = rect.center();
            let to_screen = |x: f64, y: f64| egui::pos2(center.x + x as f32 * scale, center.y - y as f32 * scale);
            let to_model = |pos: egui::Pos2| (((pos.x - center.x) / scale) as f64, ((center.y - pos.y) / scale) as f64);

            let background = ui.visuals().extreme_bg_color;
            let grid = ui.visuals().weak_text_color().gamma_multiply(0.35);
            let outline = ui.visuals().text_color();
            painter.rect_filled(rect, 2.0, background);
            let mut tick = -span;
            while tick <= span + 1e-9 {
                painter.line_segment([to_screen(tick, -span), to_screen(tick, span)], egui::Stroke::new(1.0, grid));
                painter.line_segment([to_screen(-span, tick), to_screen(span, tick)], egui::Stroke::new(1.0, grid));
                tick += 0.1;
            }
            painter.line_segment([to_screen(-span, 0.0), to_screen(span, 0.0)], egui::Stroke::new(1.5, grid));
            painter.line_segment([to_screen(0.0, -span), to_screen(0.0, span)], egui::Stroke::new(1.5, grid));

            for (index, part) in app.designer_parts.iter().enumerate() {
                let selected = app.designer_selected == Some(index);
                let stroke = egui::Stroke::new(if selected { 2.5 } else { 1.2 }, if selected { egui::Color32::LIGHT_BLUE } else { outline });
                let fill = if selected {
                    egui::Color32::from_rgba_unmultiplied(90, 150, 220, 110)
                } else {
                    egui::Color32::from_rgba_unmultiplied(140, 140, 140, 90)
                };
                match part {
                    DesignerPart::Rectangle { x, y, width, depth } => {
                        let min = to_screen(x - width / 2.0, y + depth / 2.0);
                        let max = to_screen(x + width / 2.0, y - depth / 2.0);
                        let part_rect = egui::Rect::from_min_max(min, max);
                        painter.rect_filled(part_rect, 1.0, fill);
                        let corners = [part_rect.left_top(), part_rect.right_top(), part_rect.right_bottom(), part_rect.left_bottom()];
                        for corner in 0..4 {
                            painter.line_segment([corners[corner], corners[(corner + 1) % 4]], stroke);
                        }
                    }
                    DesignerPart::Circle { x, y, diameter } => {
                        let radius = (diameter / 2.0) as f32 * scale;
                        painter.circle_filled(to_screen(*x, *y), radius, fill);
                        painter.circle_stroke(to_screen(*x, *y), radius, stroke);
                    }
                    DesignerPart::Polygon { points, holes } => {
                        let screen: Vec<egui::Pos2> = points.iter().map(|&(x, y)| to_screen(x, y)).collect();
                        if screen.len() >= 3 {
                            painter.add(egui::Shape::convex_polygon(screen.clone(), fill, egui::Stroke::NONE));
                            painter.add(egui::Shape::closed_line(screen, stroke));
                        }
                        for hole in holes {
                            let hole_screen: Vec<egui::Pos2> = hole.iter().map(|&(x, y)| to_screen(x, y)).collect();
                            if hole_screen.len() >= 3 {
                                painter.add(egui::Shape::convex_polygon(hole_screen.clone(), background, egui::Stroke::NONE));
                                painter.add(egui::Shape::closed_line(hole_screen, egui::Stroke::new(1.4, egui::Color32::LIGHT_RED)));
                            }
                        }
                    }
                }
            }

            for (path, color) in [
                (&app.designer_pending, egui::Color32::LIGHT_GREEN),
                (&app.designer_pending_hole, egui::Color32::LIGHT_RED),
            ] {
                let screen: Vec<egui::Pos2> = path.iter().map(|&(x, y)| to_screen(x, y)).collect();
                if screen.len() >= 2 {
                    painter.add(egui::Shape::line(screen.clone(), egui::Stroke::new(1.6, color)));
                }
                for point in &screen {
                    painter.circle_filled(*point, 3.0, color);
                }
            }

            // Handles of the selected polygon: drag one to move that vertex.
            let mut handles: Vec<(usize, Option<usize>, usize, egui::Pos2)> = Vec::new();
            if let Some(index) = app.designer_selected {
                if let Some(DesignerPart::Polygon { points, holes }) = app.designer_parts.get(index) {
                    for (vertex, &(x, y)) in points.iter().enumerate() {
                        handles.push((index, None, vertex, to_screen(x, y)));
                    }
                    for (hole, ring) in holes.iter().enumerate() {
                        for (vertex, &(x, y)) in ring.iter().enumerate() {
                            handles.push((index, Some(hole), vertex, to_screen(x, y)));
                        }
                    }
                }
            }
            for (part, hole, vertex, point) in &handles {
                let active = app.designer_vertex == Some((*part, *hole, *vertex));
                if hole.is_some() {
                    painter.circle_filled(*point, if active { 5.5 } else { 3.5 }, egui::Color32::LIGHT_RED);
                } else {
                    let size = if active { 9.0 } else { 7.0 };
                    painter.rect_filled(egui::Rect::from_center_size(*point, egui::vec2(size, size)), 1.0, egui::Color32::LIGHT_GREEN);
                }
            }
            let grab = |pos: egui::Pos2| {
                handles
                    .iter()
                    .find(|(_, _, _, point)| point.distance(pos) <= 7.0)
                    .map(|&(part, hole, vertex, _)| (part, hole, vertex))
            };
            if response.hovered() {
                let (delete, escape, dx, dy) = ui.input(|input| {
                    let dx = input.key_pressed(egui::Key::ArrowRight) as i32 - input.key_pressed(egui::Key::ArrowLeft) as i32;
                    let dy = input.key_pressed(egui::Key::ArrowUp) as i32 - input.key_pressed(egui::Key::ArrowDown) as i32;
                    (
                        input.key_pressed(egui::Key::Delete) || input.key_pressed(egui::Key::Backspace),
                        input.key_pressed(egui::Key::Escape),
                        dx,
                        dy,
                    )
                });
                if delete {
                    if let Some(index) = app.designer_selected {
                        remove = Some(index);
                    }
                }
                if escape {
                    app.designer_pending.clear();
                    app.designer_pending_hole.clear();
                }
                if dx != 0 || dy != 0 {
                    if let Some((part, hole, vertex)) = app.designer_vertex {
                        if let Some(DesignerPart::Polygon { points, holes }) = app.designer_parts.get_mut(part) {
                            let slot = match hole {
                                Some(index) => holes.get_mut(index).and_then(|ring| ring.get_mut(vertex)),
                                None => points.get_mut(vertex),
                            };
                            if let Some((x, y)) = slot {
                                *x += dx as f64 * 0.01;
                                *y += dy as f64 * 0.01;
                            }
                        }
                    }
                }
            }
            if response.drag_started() {
                if let Some(hit) = response.interact_pointer_pos().and_then(|pos| grab(pos)) {
                    app.designer_drag_vertex = Some(hit);
                    app.designer_vertex = Some(hit);
                }
            }
            if let (Some((part, hole, vertex)), Some(pos)) = (app.designer_drag_vertex, response.interact_pointer_pos()) {
                if response.dragged() {
                    let point = to_model(pos);
                    if let Some(DesignerPart::Polygon { points, holes }) = app.designer_parts.get_mut(part) {
                        let slot = match hole {
                            Some(index) => holes.get_mut(index).and_then(|ring| ring.get_mut(vertex)),
                            None => points.get_mut(vertex),
                        };
                        if let Some(slot) = slot {
                            *slot = point;
                        }
                    }
                }
            }
            if app.designer_new_kind != DesignerPartKind::Polygon && app.designer_drag_vertex.is_none() {
                if response.drag_started() {
                    if let Some(pos) = response.interact_pointer_pos() {
                        app.designer_drag_start = Some(to_model(pos));
                    }
                }
                if response.dragged() {
                    if let (Some(start), Some(pos)) = (app.designer_drag_start, response.interact_pointer_pos()) {
                        let (start_x, start_y) = start;
                        let (end_x, end_y) = to_model(pos);
                        let preview = egui::Color32::from_rgba_unmultiplied(140, 200, 140, 120);
                        match app.designer_new_kind {
                            DesignerPartKind::Rectangle => {
                                let min = to_screen(start_x.min(end_x), start_y.max(end_y));
                                let max = to_screen(start_x.max(end_x), start_y.min(end_y));
                                painter.rect_filled(egui::Rect::from_min_max(min, max), 1.0, preview);
                            }
                            DesignerPartKind::Circle => {
                                let radius = ((end_x - start_x).powi(2) + (end_y - start_y).powi(2)).sqrt();
                                painter.circle_filled(to_screen(start_x, start_y), radius as f32 * scale, preview);
                            }
                            DesignerPartKind::Polygon => {}
                        }
                    }
                }
                if response.drag_stopped() {
                    if let (Some(start), Some(pos)) = (app.designer_drag_start.take(), response.interact_pointer_pos()) {
                        let (start_x, start_y) = start;
                        let (end_x, end_y) = to_model(pos);
                        let part = match app.designer_new_kind {
                            DesignerPartKind::Rectangle => {
                                let width = (end_x - start_x).abs();
                                let depth = (end_y - start_y).abs();
                                if width > 1e-6 && depth > 1e-6 {
                                    Some(DesignerPart::Rectangle {
                                        x: (start_x + end_x) / 2.0,
                                        y: (start_y + end_y) / 2.0,
                                        width,
                                        depth,
                                    })
                                } else {
                                    None
                                }
                            }
                            DesignerPartKind::Circle => {
                                let radius = ((end_x - start_x).powi(2) + (end_y - start_y).powi(2)).sqrt();
                                if radius > 1e-6 {
                                    Some(DesignerPart::Circle {
                                        x: start_x,
                                        y: start_y,
                                        diameter: radius * 2.0,
                                    })
                                } else {
                                    None
                                }
                            }
                            DesignerPartKind::Polygon => None,
                        };
                        if let Some(part) = part {
                            app.designer_parts.push(part);
                            app.designer_selected = Some(app.designer_parts.len() - 1);
                        }
                    }
                }
            }

            if response.drag_stopped() {
                app.designer_drag_vertex = None;
            }

            if response.clicked() {
                if let Some(pos) = response.interact_pointer_pos() {
                    let hit = grab(pos);
                    let on_handle = hit.is_some();
                    if let Some(hit) = hit {
                        app.designer_vertex = Some(hit);
                    }
                    let (x, y) = to_model(pos);
                    if on_handle {
                        // A handle click only arms a drag; it adds no polygon vertex.
                    } else if app.designer_new_kind == DesignerPartKind::Polygon {
                        if app.designer_hole_mode {
                            app.designer_pending_hole.push((x, y));
                        } else {
                            app.designer_pending.push((x, y));
                        }
                    } else {
                        app.designer_selected = app.designer_parts.iter().rposition(|part| part.contains(x, y));
                    }
                }
            }

            ui.separator();
            egui::ScrollArea::vertical().max_height(160.0).id_salt("designer_parts").show(ui, |ui| {
                egui::Grid::new("designer_grid").striped(true).num_columns(6).show(ui, |ui| {
                    ui.strong("Part");
                    ui.strong("X");
                    ui.strong("Y");
                    ui.strong("Size 1 (m)");
                    ui.strong("Size 2 (m)");
                    ui.strong("");
                    ui.end_row();
                    for (index, part) in app.designer_parts.iter_mut().enumerate() {
                        match part {
                            DesignerPart::Rectangle { x, y, width, depth } => {
                                ui.label("Rect");
                                ui.add(egui::DragValue::new(x).speed(0.005).range(-10.0..=10.0));
                                ui.add(egui::DragValue::new(y).speed(0.005).range(-10.0..=10.0));
                                ui.add(egui::DragValue::new(width).speed(0.005).range(0.0..=10.0));
                                ui.add(egui::DragValue::new(depth).speed(0.005).range(0.0..=10.0));
                            }
                            DesignerPart::Circle { x, y, diameter } => {
                                ui.label("Circle");
                                ui.add(egui::DragValue::new(x).speed(0.005).range(-10.0..=10.0));
                                ui.add(egui::DragValue::new(y).speed(0.005).range(-10.0..=10.0));
                                ui.add(egui::DragValue::new(diameter).speed(0.005).range(0.0..=10.0));
                                ui.label("—");
                            }
                            DesignerPart::Polygon { points, holes } => {
                                ui.label("Polygon");
                                ui.label(format!("{} pts", points.len()));
                                ui.label(format!("{} holes", holes.len()));
                                ui.label("—");
                                ui.label("—");
                            }
                        }
                        if ui.small_button("✕").on_hover_text("Delete this part").clicked() {
                            remove = Some(index);
                        }
                        ui.end_row();
                    }
                });
            });
            if let Some(index) = remove {
                app.designer_parts.remove(index);
                app.designer_selected = None;
                app.designer_drag_vertex = None;
                app.designer_vertex = None;
            }

            ui.separator();
            match Section::from_designer("preview", &app.designer_parts) {
                Some(section) => {
                    let (sy, sz) = section.section_moduli();
                    ui.label(format!(
                        "A = {:.5} m²    Iy = {:.4e} m⁴    Iz = {:.4e} m⁴    J = {:.4e} m⁴    Sy = {:.4e} m³    Sz = {:.4e} m³",
                        section.area, section.iy, section.iz, section.j, sy, sz
                    ));
                    ui.label(
                        egui::RichText::new(format!(
                            "{} part(s); bounding box {:.1} × {:.1} mm. Holes are subtracted from their polygon.",
                            app.designer_parts.len(),
                            section.width * 1000.0,
                            section.depth * 1000.0
                        ))
                        .weak(),
                    );
                }
                None => {
                    ui.label("Draw at least one valid part to see the section properties.");
                }
            }
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Apply section").clicked() {
                    apply = true;
                }
                if ui.button("Cancel").clicked() {
                    close = true;
                }
            });
            ui.label(
                egui::RichText::new(format!(
                    "Canvas and table coordinates are metres (SI); the display unit ({}) does not rescale the designer.",
                    units.len_label()
                ))
                .weak(),
            );
        });

    if clear {
        app.designer_parts.clear();
        app.designer_selected = None;
        app.designer_drag_vertex = None;
        app.designer_vertex = None;
        app.designer_pending.clear();
        app.designer_pending_hole.clear();
    }
    if finish_polygon && app.designer_pending.len() >= 3 {
        app.designer_parts.push(DesignerPart::Polygon {
            points: app.designer_pending.clone(),
            holes: Vec::new(),
        });
        app.designer_selected = Some(app.designer_parts.len() - 1);
        app.designer_pending.clear();
    }
    if finish_hole && app.designer_pending_hole.len() >= 3 {
        if let Some(index) = app.designer_selected {
            if let Some(DesignerPart::Polygon { holes, .. }) = app.designer_parts.get_mut(index) {
                holes.push(app.designer_pending_hole.clone());
            }
        }
        app.designer_pending_hole.clear();
    }
    if close {
        open = false;
    }
    if apply && app.apply_section_designer() {
        open = false;
    }
    app.section_designer_open = open;
}
