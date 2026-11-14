use crate::app::OpenStructApp;
use eframe::egui;
use openstruct::loads::LoadDirection;

/// Assign a uniform distributed load to every selected frame.
pub fn open_frame_loads_dialog(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.frame_load_dialog_open {
        return;
    }
    let mut open = app.frame_load_dialog_open;
    let mut close = false;
    let mut apply = false;
    let units = app.units;

    egui::Window::new("Assign Frame Loads…")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(430.0)
        .show(ctx, |ui| {
            ui.label(format!("{} frame(s) selected.", app.selected_elements.len()));
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Direction:");
                ui.selectable_value(&mut app.frame_load_direction, LoadDirection::Global, "Global");
                ui.selectable_value(&mut app.frame_load_direction, LoadDirection::Local, "Local");
            });
            ui.horizontal(|ui| {
                ui.label("Load case:");
                let selected = app
                    .frame_load_case
                    .and_then(|id| app.model.load_cases.iter().find(|case| case.id == id))
                    .map(|case| case.name.clone())
                    .unwrap_or_else(|| "All cases".to_string());
                egui::ComboBox::from_id_salt("frame_load_case").selected_text(selected).show_ui(ui, |ui| {
                    ui.selectable_value(&mut app.frame_load_case, None, "All cases");
                    let cases: Vec<(usize, String)> = app.model.load_cases.iter().map(|case| (case.id, case.name.clone())).collect();
                    for (id, name) in cases {
                        ui.selectable_value(&mut app.frame_load_case, Some(id), name);
                    }
                });
            });
            ui.horizontal(|ui| {
                ui.label("Axis:");
                ui.selectable_value(&mut app.frame_load_axis, 0, "X");
                ui.selectable_value(&mut app.frame_load_axis, 1, "Y");
                ui.selectable_value(&mut app.frame_load_axis, 2, "Z");
            });
            ui.horizontal(|ui| {
                ui.label(format!("Intensity ({}):", units.dist_label()));
                ui.add(
                    egui::TextEdit::singleline(&mut app.frame_load_magnitude)
                        .hint_text(format!("e.g. 12 {}", units.dist_label()))
                        .desired_width(150.0),
                );
            });
            ui.checkbox(&mut app.frame_load_projected, "Projected onto the member length");
            ui.separator();
            ui.label(egui::RichText::new("The load is uniform over each selected frame and replaces its existing frame loads in the chosen case.").weak());
            ui.horizontal(|ui| {
                if ui.button("Assign").clicked() {
                    apply = true;
                }
                if ui.button("Cancel").clicked() {
                    close = true;
                }
            });
        });

    if close {
        open = false;
    }
    if apply {
        if app.apply_frame_loads() {
            open = false;
        }
    }
    app.frame_load_dialog_open = open;
}
