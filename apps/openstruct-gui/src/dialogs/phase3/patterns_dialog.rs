use crate::app::OpenStructApp;
use eframe::egui;
use openstruct::loads::standard_catalogue;

/// Phase 3 load-pattern editor: a table of patterns backed by the core
/// catalogue, with add, delete, rename and self-weight multipliers.
pub fn open_patterns_dialog(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.define_load_patterns_dialog_open {
        return;
    }
    let mut open = app.define_load_patterns_dialog_open;
    let mut close = false;
    let mut apply = false;
    let mut add = false;
    let mut add_catalogue = false;
    let mut remove: Option<usize> = None;

    egui::Window::new("Load Patterns…")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(620.0)
        .show(ctx, |ui| {
            ui.label("A pattern carries its load type and the self-weight multiplier applied when its cases run.");
            ui.separator();
            egui::ScrollArea::vertical().max_height(300.0).id_salt("load_pattern_rows").show(ui, |ui| {
                egui::Grid::new("load_pattern_grid").striped(true).num_columns(4).show(ui, |ui| {
                    ui.strong("Pattern");
                    ui.strong("Type");
                    ui.strong("Self weight");
                    ui.strong("");
                    ui.end_row();
                    for (index, pattern) in app.load_pattern_draft.iter_mut().enumerate() {
                        ui.add(egui::TextEdit::singleline(&mut pattern.name).desired_width(160.0));
                        let mut chosen = None;
                        egui::ComboBox::from_id_salt(("pattern_type", index))
                            .selected_text(pattern.load_type.label())
                            .show_ui(ui, |ui| {
                                for (load_type, _) in standard_catalogue() {
                                    if ui.selectable_label(pattern.load_type == load_type, load_type.label()).clicked() {
                                        chosen = Some(load_type);
                                    }
                                }
                            });
                        if let Some(load_type) = chosen {
                            pattern.load_type = load_type;
                        }
                        ui.add(egui::DragValue::new(&mut pattern.self_weight_multiplier).range(0.0..=10.0).speed(0.05));
                        if ui.small_button("✕").on_hover_text("Delete this pattern").clicked() {
                            remove = Some(index);
                        }
                        ui.end_row();
                    }
                });
            });
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("New pattern:");
                ui.add(
                    egui::TextEdit::singleline(&mut app.load_pattern_new_name)
                        .hint_text("PATTERN")
                        .desired_width(140.0),
                );
                egui::ComboBox::from_id_salt("new_pattern_type")
                    .selected_text(app.load_pattern_new_type.label())
                    .show_ui(ui, |ui| {
                        for (load_type, _) in standard_catalogue() {
                            if ui.selectable_label(app.load_pattern_new_type == load_type, load_type.label()).clicked() {
                                app.load_pattern_new_type = load_type;
                            }
                        }
                    });
                if ui.button("Add").clicked() {
                    add = true;
                }
                if ui.button("Add reference catalogue").clicked() {
                    add_catalogue = true;
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
            ui.label(egui::RichText::new("Applied patterns are saved in the .ostrut document and are one undo step.").weak());
        });

    if let Some(index) = remove {
        app.load_pattern_draft.remove(index);
    }
    if add {
        app.add_load_pattern();
    }
    if add_catalogue {
        app.add_catalogue_patterns();
    }
    if close {
        open = false;
    }
    if apply {
        if app.apply_load_patterns() {
            open = false;
        }
    }
    app.define_load_patterns_dialog_open = open;
}
