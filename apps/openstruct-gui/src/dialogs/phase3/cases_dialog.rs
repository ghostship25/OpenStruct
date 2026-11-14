use crate::app::OpenStructApp;
use eframe::egui;
use openstruct::loads::LoadType;

/// Phase 3 load-case editor. Cases are stored on the model and the linear
/// static analysis runs the first one.
pub fn open_cases_dialog(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.define_load_cases_dialog_open {
        return;
    }
    let mut open = app.define_load_cases_dialog_open;
    let mut close = false;
    let mut apply = false;
    let mut add = false;
    let mut remove: Option<usize> = None;
    let patterns: Vec<(String, LoadType)> = app
        .model
        .load_patterns
        .iter()
        .map(|pattern| (pattern.name.clone(), pattern.load_type.clone()))
        .collect();

    egui::Window::new("Load Cases…")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(680.0)
        .show(ctx, |ui| {
            ui.label("A case ties a load pattern to the loads assigned on the model. The linear static analysis runs the first case.");
            ui.separator();
            egui::ScrollArea::vertical().max_height(300.0).id_salt("load_case_rows").show(ui, |ui| {
                egui::Grid::new("load_case_grid").striped(true).num_columns(5).show(ui, |ui| {
                    ui.strong("Case");
                    ui.strong("Pattern");
                    ui.strong("Self weight");
                    ui.strong("P-Δ");
                    ui.strong("");
                    ui.end_row();
                    for (index, case) in app.load_case_draft.iter_mut().enumerate() {
                        ui.add(egui::TextEdit::singleline(&mut case.name).desired_width(160.0));
                        egui::ComboBox::from_id_salt(("case_pattern", index))
                            .selected_text(case.pattern.label())
                            .show_ui(ui, |ui| {
                                if patterns.is_empty() {
                                    ui.label("No load patterns defined.");
                                }
                                for (name, load_type) in &patterns {
                                    if ui.selectable_label(case.pattern == *load_type, name).clicked() {
                                        case.pattern = load_type.clone();
                                    }
                                }
                            });
                        ui.add(egui::DragValue::new(&mut case.self_weight_multiplier).speed(0.05).range(0.0..=10.0));
                        ui.checkbox(&mut case.p_delta, "");
                        if ui.small_button("✕").on_hover_text("Delete this case").clicked() {
                            remove = Some(index);
                        }
                        ui.end_row();
                    }
                });
            });
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("New case:");
                ui.add(
                    egui::TextEdit::singleline(&mut app.load_case_new_name)
                        .hint_text("CASE 1")
                        .desired_width(160.0),
                );
                if ui.button("Add").clicked() {
                    add = true;
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Apply").clicked() {
                    apply = true;
                }
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
            ui.label(egui::RichText::new("Point loads assigned on the canvas are added to the first case; frame and area loads apply to every case.").weak());
        });

    if let Some(index) = remove {
        app.load_case_draft.remove(index);
    }
    if add {
        app.add_load_case();
    }
    if close {
        open = false;
    }
    if apply {
        if app.apply_load_cases() {
            open = false;
        }
    }
    app.define_load_cases_dialog_open = open;
}
