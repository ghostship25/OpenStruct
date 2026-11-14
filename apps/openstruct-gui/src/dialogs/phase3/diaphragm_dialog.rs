use crate::app::OpenStructApp;
use eframe::egui;

/// Assign diaphragm releases/fixity to the selected frames.
/// A diaphragm models a rigid horizontal diaphragm that connects frames at a story level,
/// releasing rotational dofs but keeping shear/axial transferred.
pub fn open_diaphragm_dialog(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.diaphragm_dialog_open {
        return;
    }
    let mut open = app.diaphragm_dialog_open;
    let mut apply = false;
    let mut close = false;
    egui::Window::new("Diaphragm Assignment…")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(400.0)
        .show(ctx, |ui| {
            ui.label(
                "A diaphragm releases rotational dofs (RX, RY, RZ) at each end while keeping axial and shear dof active. \
                 This models a rigid horizontal diaphragm plate at the story level."
            );
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("All released").clicked() {
                    app.diaphragm_release_draft = [true; 12];
                }
                if ui.button("All fixed").clicked() {
                    app.diaphragm_release_draft = [false; 12];
                }
            });
            ui.separator();
            egui::Grid::new("diaphragm_grid").striped(true).num_columns(3).show(ui, |ui| {
                ui.strong("Dof");
                ui.strong("Released");
                ui.end_row();
                const DOFS: [(&str, usize); 6] = [
                    ("Axial (U1)", 0),
                    ("Shear V2", 1),
                    ("Shear V3", 2),
                    ("Torsion (R1)", 3),
                    ("Moment M2", 4),
                    ("Moment M3", 5),
                ];
                for (label, dof) in DOFS {
                    ui.label(label);
                    ui.checkbox(&mut app.diaphragm_release_draft[dof], "");
                    ui.end_row();
                }
            });
            ui.separator();
            ui.label(
                egui::RichText::new(format!(
                    "Applies to {} selected frame(s). Re-run the analysis (F5) after applying.",
                    app.selected_elements.len()
                ))
                .weak(),
            );
            ui.horizontal(|ui| {
                if ui.add_enabled(!app.selected_elements.is_empty(), egui::Button::new("Apply")).clicked() {
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
        app.apply_diaphragm();
        open = app.diaphragm_dialog_open;
    }
    app.diaphragm_dialog_open = open;
}