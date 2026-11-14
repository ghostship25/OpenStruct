use crate::app::OpenStructApp;
use eframe::egui;
use openstruct::loads::LoadDirection;

/// Assign a uniform pressure to every selected area object.
pub fn open_area_loads_dialog(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.area_load_dialog_open {
        return;
    }
    let mut open = app.area_load_dialog_open;
    let mut close = false;
    let mut apply = false;
    let units = app.units;

    egui::Window::new("Assign Area Loads…")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(430.0)
        .show(ctx, |ui| {
            ui.label(format!("{} area(s) selected.", app.selected_areas.len()));
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Direction:");
                ui.selectable_value(&mut app.area_load_direction, LoadDirection::Global, "Global");
                ui.selectable_value(&mut app.area_load_direction, LoadDirection::Local, "Local");
            });
            ui.horizontal(|ui| {
                ui.label("Load case:");
                let selected = app
                    .area_load_case
                    .and_then(|id| app.model.load_cases.iter().find(|case| case.id == id))
                    .map(|case| case.name.clone())
                    .unwrap_or_else(|| "All cases".to_string());
                egui::ComboBox::from_id_salt("area_load_case").selected_text(selected).show_ui(ui, |ui| {
                    ui.selectable_value(&mut app.area_load_case, None, "All cases");
                    let cases: Vec<(usize, String)> = app.model.load_cases.iter().map(|case| (case.id, case.name.clone())).collect();
                    for (id, name) in cases {
                        ui.selectable_value(&mut app.area_load_case, Some(id), name);
                    }
                });
            });
            ui.horizontal(|ui| {
                ui.label(format!("Pressure ({}):", units.pressure_label()));
                ui.add(
                    egui::TextEdit::singleline(&mut app.area_load_pressure)
                        .hint_text(format!("e.g. 2 {}", units.pressure_label()))
                        .desired_width(150.0),
                );
            });
            ui.separator();
            ui.label(egui::RichText::new("The pressure is uniform over each selected area and replaces its existing area loads in the chosen case.").weak());
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
        if app.apply_area_loads() {
            open = false;
        }
    }
    app.area_load_dialog_open = open;
}
