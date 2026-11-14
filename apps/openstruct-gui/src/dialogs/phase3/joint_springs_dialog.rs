use crate::app::OpenStructApp;
use eframe::egui;

/// Global dof label and the unit of its spring stiffness.
const DOFS: [(&str, &str); 6] = [
    ("UX", "N/m"),
    ("UY", "N/m"),
    ("UZ", "N/m"),
    ("RX", "N·m/rad"),
    ("RY", "N·m/rad"),
    ("RZ", "N·m/rad"),
];

/// Assign semi-rigid support springs to the selected joints.
pub fn open_joint_springs_dialog(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.joint_springs_dialog_open {
        return;
    }
    let mut open = app.joint_springs_dialog_open;
    let mut apply = false;
    let mut close = false;
    egui::Window::new("Joint Springs…")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(360.0)
        .show(ctx, |ui| {
            ui.label("A support spring adds stiffness to a joint dof. Leave a dof at 0 for no spring; a rigid restraint on the same dof takes precedence.");
            ui.separator();
            egui::Grid::new("joint_spring_grid").striped(true).num_columns(3).show(ui, |ui| {
                ui.strong("Dof");
                ui.strong("Stiffness");
                ui.strong("Unit");
                ui.end_row();
                for (index, (label, unit)) in DOFS.iter().enumerate() {
                    ui.label(*label);
                    ui.add(
                        egui::DragValue::new(&mut app.spring_draft[index])
                            .speed(1.0e5)
                            .range(0.0..=1.0e16)
                            .max_decimals(0)
                            .suffix(""),
                    );
                    ui.label(*unit);
                    ui.end_row();
                }
            });
            ui.horizontal(|ui| {
                if ui.button("Clear springs").clicked() {
                    app.spring_draft = [0.0; 6];
                }
                if ui.button("1e8 on translations").clicked() {
                    for dof in 0..3 {
                        app.spring_draft[dof] = 1.0e8;
                    }
                }
            });
            ui.separator();
            ui.label(
                egui::RichText::new(format!(
                    "Applies to {} selected joint(s). Re-run the analysis (F5) after applying.",
                    app.selected_nodes.len()
                ))
                .weak(),
            );
            ui.horizontal(|ui| {
                if ui.add_enabled(!app.selected_nodes.is_empty(), egui::Button::new("Apply")).clicked() {
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
        app.apply_joint_springs();
        open = app.joint_springs_dialog_open;
    }
    app.joint_springs_dialog_open = open;
}
