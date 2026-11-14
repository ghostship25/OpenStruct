use crate::app::{OpenStructApp, UnitSystem};
use eframe::egui;

/// Global display units, theme and the current design-code defaults.
pub fn open_preferences_dialog(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.preferences_dialog_open {
        return;
    }
    let mut open = app.preferences_dialog_open;
    let mut close = false;

    egui::Window::new("Preferences…")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(520.0)
        .show(ctx, |ui| {
            ui.label("Units convert displayed values only; the model is always stored in SI.");
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Units:");
                for units in UnitSystem::ALL {
                    ui.selectable_value(&mut app.units, units, units.label());
                }
            });
            ui.label(
                egui::RichText::new(format!(
                    "Lengths in {}, forces in {}, moments in {}, pressures in {}.",
                    app.units.len_label(),
                    app.units.force_label(),
                    app.units.moment_label(),
                    app.units.pressure_label()
                ))
                .weak(),
            );
            ui.separator();
            ui.checkbox(&mut app.dark, "Dark theme");
            ui.label(
                egui::RichText::new("Values can be typed with units — 5 ft, 3000 mm, 12 kN/m, 2 kPa — and are converted to SI on entry.")
                    .weak(),
            );
            ui.separator();
            ui.label("Design codes: AISC 360-10 · ACI 318-14");
            ui.label("Default file format: .ostrut JSON");
            ui.separator();
            if ui.button("Close").clicked() {
                close = true;
            }
        });

    if close {
        open = false;
    }
    app.preferences_dialog_open = open;
}
