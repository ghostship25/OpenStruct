use crate::app::OpenStructApp;
use eframe::egui;
use openstruct::loads::ComboType;

/// Phase 3 load-combination editor with code default templates.
pub fn open_combos_dialog(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.define_combos_dialog_open {
        return;
    }
    let mut open = app.define_combos_dialog_open;
    let mut close = false;
    let mut apply = false;
    let mut add = false;
    let mut add_code = false;
    let mut remove: Option<usize> = None;

    egui::Window::new("Load Combinations…")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(700.0)
        .show(ctx, |ui| {
            ui.label("Combinations name the equations the model is checked against in later phases.");
            ui.separator();
            egui::ScrollArea::vertical().max_height(300.0).id_salt("combo_rows").show(ui, |ui| {
                egui::Grid::new("combo_grid").striped(true).num_columns(4).show(ui, |ui| {
                    ui.strong("Combination");
                    ui.strong("Equation");
                    ui.strong("Type");
                    ui.strong("");
                    ui.end_row();
                    for (index, combo) in app.combo_draft.iter_mut().enumerate() {
                        ui.add(egui::TextEdit::singleline(&mut combo.name).desired_width(150.0));
                        ui.add(egui::TextEdit::singleline(&mut combo.equation).desired_width(240.0));
                        egui::ComboBox::from_id_salt(("combo_type", index))
                            .selected_text(combo.combo_type.label())
                            .show_ui(ui, |ui| {
                                for combo_type in [
                                    ComboType::LinearAdd,
                                    ComboType::Envelope,
                                    ComboType::Ssrs,
                                    ComboType::Abs,
                                    ComboType::Cqc,
                                ] {
                                    ui.selectable_value(&mut combo.combo_type, combo_type, combo_type.label());
                                }
                            });
                        if ui.small_button("✕").on_hover_text("Delete this combination").clicked() {
                            remove = Some(index);
                        }
                        ui.end_row();
                    }
                });
            });
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("New:");
                ui.add(egui::TextEdit::singleline(&mut app.combo_new_name).hint_text("1.2D + 1.6L").desired_width(150.0));
                ui.add(
                    egui::TextEdit::singleline(&mut app.combo_new_equation)
                        .hint_text("1.2*DEAD + 1.6*LIVE")
                        .desired_width(240.0),
                );
                if ui.button("Add").clicked() {
                    add = true;
                }
                if ui.button("Add ASCE 7 defaults").clicked() {
                    add_code = true;
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
        });

    if let Some(index) = remove {
        app.combo_draft.remove(index);
    }
    if add {
        app.add_combo();
    }
    if add_code {
        app.add_code_combos();
    }
    if close {
        open = false;
    }
    if apply {
        if app.apply_combos() {
            open = false;
        }
    }
    app.define_combos_dialog_open = open;
}
