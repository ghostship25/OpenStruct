//! Status bar: units, cursor, story, model counts, solver state and messages.

use crate::app::{OpenStructApp, UnitSystem};
use crate::commands::Cmd;
use crate::theme;
use eframe::egui;

pub fn statusbar(app: &mut OpenStructApp, ui: &mut egui::Ui) {
    let pal = theme::palette(app.dark);
    ui.horizontal_centered(|ui| {
        egui::ComboBox::from_id_salt("units_status").width(132.0).selected_text(app.units.label()).show_ui(ui, |ui| {
            if ui.selectable_label(app.units == UnitSystem::Metric, Cmd::UnitsMetric.label()).clicked() {
                app.run(Cmd::UnitsMetric);
                ui.close();
            }
            if ui.selectable_label(app.units == UnitSystem::Imperial, Cmd::UnitsImperial.label()).clicked() {
                app.run(Cmd::UnitsImperial);
                ui.close();
            }
        });
        ui.separator();

        let unit = app.units;
        let (u_label, v_label) = app.active_work_plane().axis_labels();
        ui.label(format!(
            "{u_label} {:.2}   {v_label} {:.2} {}",
            app.mouse_world.0 * unit.len_factor(),
            app.mouse_world.1 * unit.len_factor(),
            unit.len_label()
        ));
        ui.separator();
        let plane = if app.work_plane_active() {
            format!("Work plane: {} ({})", app.work_plane_name(), app.work_plane_description())
        } else {
            format!("Story: {} @ {:.2} m", app.active_story_name(), app.active_elevation())
        };
        ui.label(plane);
        ui.separator();
        ui.label(format!("Joints {}   Frames {}   Areas {}", app.model.nodes.len(), app.model.elements.len(), app.model.areas.len()));
        ui.separator();
        ui.label(format!("Selected {}J / {}F / {}A", app.selected_nodes.len(), app.selected_elements.len(), app.selected_areas.len()));
        ui.separator();

        match &app.results {
            Some(results) => {
                ui.label(egui::RichText::new("Solved").color(pal.ok)).on_hover_text(results.status.as_str());
            }
            None => {
                ui.label(egui::RichText::new("Not analysed").color(pal.text_dim));
            }
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // One line only: a long message used to wrap, make the bar taller and
            // shrink the canvas, which moved the view under the pointer.
            let text = egui::RichText::new(&app.status_text).color(pal.text_dim);
            ui.add(egui::Label::new(text).truncate()).on_hover_text(&app.status_text);
        });
    });
}
