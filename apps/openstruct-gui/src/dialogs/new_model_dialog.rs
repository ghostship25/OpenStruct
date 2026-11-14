//! New-model workflow, following standard "Model Initialization" and "New
//! Model Quick Templates" dialogs: units, region, section database, steel and
//! concrete design codes, grid dimensions, story dimensions and a first
//! structural template.

use crate::app::{NewModelTemplate, OpenStructApp, UnitSystem};
use eframe::egui;
use openstruct::design::codes::DesignCode;

const REGIONS: [&str; 4] = ["India", "United States", "Europe", "Nepal"];
const STEEL_DATABASES: [&str; 3] = ["Indian", "AISC", "Euro"];

pub fn new_model_window(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.new_model_open {
        return;
    }
    let mut open = app.new_model_open;
    let mut build = false;

    egui::Window::new("New Model")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(560.0)
        .show(ctx, |ui| {
            ui.label(egui::RichText::new("Model initialization").strong());
            ui.group(|ui| {
                egui::Grid::new("nm_init").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                    ui.label("Display units:");
                    egui::ComboBox::from_id_salt("nm_units")
                        .selected_text(if app.nm_units == UnitSystem::Metric { "Metric SI" } else { "Imperial" })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut app.nm_units, UnitSystem::Metric, "Metric SI");
                            ui.selectable_value(&mut app.nm_units, UnitSystem::Imperial, "Imperial");
                        });
                    ui.end_row();

                    ui.label("Region for default materials:");
                    egui::ComboBox::from_id_salt("nm_region")
                        .selected_text(REGIONS[app.nm_region.min(REGIONS.len() - 1)])
                        .show_ui(ui, |ui| {
                            for (index, name) in REGIONS.iter().enumerate() {
                                ui.selectable_value(&mut app.nm_region, index, *name);
                            }
                        });
                    ui.end_row();

                    ui.label("Steel section database:");
                    egui::ComboBox::from_id_salt("nm_steel_db")
                        .selected_text(STEEL_DATABASES[app.nm_steel_db.min(STEEL_DATABASES.len() - 1)])
                        .show_ui(ui, |ui| {
                            for (index, name) in STEEL_DATABASES.iter().enumerate() {
                                ui.selectable_value(&mut app.nm_steel_db, index, *name);
                            }
                        });
                    ui.end_row();

                    ui.label("Steel design code:");
                    egui::ComboBox::from_id_salt("nm_steel_code")
                        .selected_text(app.nm_steel_code.label())
                        .show_ui(ui, |ui| {
                            for option in DesignCode::ALL {
                                ui.selectable_value(&mut app.nm_steel_code, option, option.label());
                            }
                        });
                    ui.end_row();

                    ui.label("Concrete design code:");
                    egui::ComboBox::from_id_salt("nm_concrete_code")
                        .selected_text(app.nm_concrete_code.label())
                        .show_ui(ui, |ui| {
                            for option in DesignCode::ALL {
                                ui.selectable_value(&mut app.nm_concrete_code, option, option.label());
                            }
                        });
                    ui.end_row();
                });
            });

            ui.add_space(8.0);
            ui.label(egui::RichText::new("Grid dimensions (plan)").strong());
            ui.group(|ui| {
                egui::Grid::new("nm_grid").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                    ui.label("Number of grid bays in X:");
                    ui.add(egui::DragValue::new(&mut app.nm_nx).range(1..=40));
                    ui.end_row();
                    ui.label("Number of grid bays in Y:");
                    ui.add(egui::DragValue::new(&mut app.nm_ny).range(1..=40));
                    ui.end_row();
                    ui.label("Spacing in X (m):");
                    ui.add(egui::DragValue::new(&mut app.nm_dx).range(0.5..=50.0).speed(0.25));
                    ui.end_row();
                    ui.label("Spacing in Y (m):");
                    ui.add(egui::DragValue::new(&mut app.nm_dy).range(0.5..=50.0).speed(0.25));
                    ui.end_row();
                });
            });

            ui.add_space(8.0);
            ui.label(egui::RichText::new("Story dimensions").strong());
            ui.group(|ui| {
                egui::Grid::new("nm_story").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                    ui.label("Number of stories:");
                    ui.add(egui::DragValue::new(&mut app.nm_stories).range(1..=80));
                    ui.end_row();
                    ui.label("Typical story height (m):");
                    ui.add(egui::DragValue::new(&mut app.nm_typical_h).range(1.5..=10.0).speed(0.1));
                    ui.end_row();
                    ui.label("Bottom story height (m):");
                    ui.add(egui::DragValue::new(&mut app.nm_base_h).range(1.5..=10.0).speed(0.1));
                    ui.end_row();
                });
            });

            ui.add_space(8.0);
            ui.label(egui::RichText::new("Add structural objects").strong());
            ui.horizontal(|ui| {
                for template in NewModelTemplate::ALL {
                    let selected = app.nm_template == template;
                    if ui.selectable_label(selected, template.label()).clicked() {
                        app.nm_template = template;
                    }
                }
            });
            ui.label(egui::RichText::new(app.nm_template.description()).weak());

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button("3-storey 3×3 preset").clicked() {
                    app.nm_preset_3x3();
                }
                ui.label(egui::RichText::new("Slab, deck and truss templates are not available yet.").weak());
            });

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("OK").clicked() {
                    build = true;
                }
                if ui.button("Cancel").clicked() {
                    app.new_model_open = false;
                }
            });
        });

    if build {
        app.build_new_model();
        app.new_model_open = false;
    } else {
        app.new_model_open = open;
    }
}
