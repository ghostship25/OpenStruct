use crate::app::OpenStructApp;
use eframe::egui;

/// Assign temperature and initial-strain loads to the selected frames.
pub fn open_temperature_loads_dialog(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.temperature_dialog_open {
        return;
    }
    let mut open = app.temperature_dialog_open;
    let mut apply = false;
    let mut remove = false;
    let mut close = false;
    let cases: Vec<(usize, String)> = app.model.load_cases.iter().map(|case| (case.id, case.name.clone())).collect();
    let selected_case = app
        .temperature_case
        .and_then(|id| cases.iter().find(|(case_id, _)| *case_id == id).map(|(_, name)| name.clone()))
        .unwrap_or_else(|| "All cases".to_string());
    egui::Window::new("Temperature / Strain Loads…")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(380.0)
        .show(ctx, |ui| {
            ui.label("A uniform ΔT and an initial axial strain both act as an imposed axial strain α·ΔT + ε. A free end moves by that strain times the member length; a restrained member develops EA·(α·ΔT + ε).");
            ui.separator();
            egui::Grid::new("temperature_grid").striped(true).num_columns(2).show(ui, |ui| {
                ui.label("ΔT (°C)");
                ui.add(egui::DragValue::new(&mut app.temperature_delta).speed(0.5).max_decimals(2));
                ui.end_row();
                ui.label("Initial strain ε");
                ui.add(egui::DragValue::new(&mut app.temperature_strain).speed(1.0e-5).max_decimals(8));
                ui.end_row();
                ui.label("Load case");
                egui::ComboBox::from_id_salt("temperature_case").selected_text(selected_case).show_ui(ui, |ui| {
                    ui.selectable_value(&mut app.temperature_case, None, "All cases");
                    for (id, name) in &cases {
                        ui.selectable_value(&mut app.temperature_case, Some(*id), name);
                    }
                });
                ui.end_row();
            });
            ui.separator();
            ui.label(
                egui::RichText::new(format!(
                    "Applies to {} selected frame(s); existing temperature loads on them for this case are replaced. Re-run the analysis (F5) after applying.",
                    app.selected_elements.len()
                ))
                .weak(),
            );
            ui.horizontal(|ui| {
                if ui.add_enabled(!app.selected_elements.is_empty(), egui::Button::new("Apply")).clicked() {
                    apply = true;
                }
                if ui
                    .add_enabled(!app.selected_elements.is_empty(), egui::Button::new("Remove assigned"))
                    .on_hover_text("Delete the temperature and strain loads on the selected frames for this case")
                    .clicked()
                {
                    remove = true;
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
        app.apply_temperature_loads(false);
        open = app.temperature_dialog_open;
    }
    if remove {
        app.apply_temperature_loads(true);
        open = app.temperature_dialog_open;
    }
    app.temperature_dialog_open = open;
}
