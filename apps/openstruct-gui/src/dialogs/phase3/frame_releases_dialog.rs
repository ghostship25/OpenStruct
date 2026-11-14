use crate::app::OpenStructApp;
use eframe::egui;

/// Local dof label and index for each release row (node i dofs 0..5).
const DOFS: [(&str, usize); 6] = [
    ("Axial (U1)", 0),
    ("Shear V2", 1),
    ("Shear V3", 2),
    ("Torsion (R1)", 3),
    ("Moment M2", 4),
    ("Moment M3", 5),
];

/// Assign end releases and partial fixity to the selected frames.
pub fn open_frame_releases_dialog(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.frame_release_dialog_open {
        return;
    }
    let mut open = app.frame_release_dialog_open;
    let mut apply = false;
    let mut close = false;
    egui::Window::new("Frame Releases / Partial Fixity…")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(560.0)
        .show(ctx, |ui| {
            ui.label("Release a member end dof, then optionally set its partial fixity. Fixity keeps a fraction of the member's own stiffness: 0 is a perfect pin, 1 is rigid. Releases are condensed out of the stiffness matrix, so the member still transfers shear and axial force.");
            ui.horizontal(|ui| {
                if ui.button("All rigid").clicked() {
                    app.release_draft = [false; 12];
                    app.fixity_draft = [0.0; 12];
                }
                if ui.button("Both ends pinned").clicked() {
                    for end in 0..2 {
                        for dof in [4, 5] {
                            app.release_draft[end * 6 + dof] = true;
                            app.fixity_draft[end * 6 + dof] = 0.0;
                        }
                    }
                }
                if ui.button("Both ends fixed").clicked() {
                    app.release_draft = [false; 12];
                    app.fixity_draft = [0.0; 12];
                }
            });
            ui.separator();
            egui::Grid::new("release_grid").striped(true).num_columns(5).show(ui, |ui| {
                ui.strong("Dof");
                ui.strong("End I released");
                ui.strong("Fixity I");
                ui.strong("End J released");
                ui.strong("Fixity J");
                ui.end_row();
                for (label, dof) in DOFS {
                    ui.label(label);
                    for end in 0..2 {
                        let index = end * 6 + dof;
                        ui.checkbox(&mut app.release_draft[index], "");
                        ui.add_enabled(
                            app.release_draft[index],
                            egui::Slider::new(&mut app.fixity_draft[index], 0.0..=0.999).show_value(true),
                        );
                    }
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
        app.apply_frame_releases();
        open = app.frame_release_dialog_open;
    }
    app.frame_release_dialog_open = open;
}
