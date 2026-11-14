//! Materials, laid out like standard: a Define Materials list with add/copy/
//! modify/delete, an Add New Material Property chooser (region → type →
//! standard → grade) and a Material Property Data editor with weight/mass
//! input, derived shear modulus, design data and damping data.

use crate::app::OpenStructApp;
use crate::material_library::{self, REGIONS};
use eframe::egui;
use openstruct::materials::MaterialType;

pub fn open_materials_dialog(app: &mut OpenStructApp, ctx: &egui::Context) {
    materials_list(app, ctx);
    add_material_window(app, ctx);
    material_editor_window(app, ctx);
}

fn materials_list(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.define_materials_dialog_open {
        return;
    }
    let mut open = app.define_materials_dialog_open;
    let mut close = false;
    let mut apply = false;
    let mut add_new = false;
    let mut add_copy = false;
    let mut modify = false;
    let mut remove = false;
    let mut assign = false;

    let names: Vec<String> = app.material_draft.iter().map(|(_, material)| material.name.clone()).collect();
    let colors: Vec<[u8; 3]> = app.material_draft.iter().map(|(_, material)| material.color).collect();
    let selected = app.material_selected.min(names.len().saturating_sub(1));

    egui::Window::new("Define Materials")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(500.0)
        .show(ctx, |ui| {
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("Materials").strong());
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("material_list")
                            .max_height(190.0)
                            .min_scrolled_width(200.0)
                            .show(ui, |ui| {
                                for (index, name) in names.iter().enumerate() {
                                    ui.horizontal(|ui| {
                                        let color = colors.get(index).copied().unwrap_or([158, 158, 158]);
                                        let (swatch, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                                        ui.painter().rect_filled(
                                            swatch,
                                            egui::CornerRadius::same(2),
                                            egui::Color32::from_rgb(color[0], color[1], color[2]),
                                        );
                                        if ui.selectable_label(index == selected, name).clicked() {
                                            app.material_selected = index;
                                        }
                                    });
                                }
                            });
                    });
                });
                ui.separator();
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("Click to:").strong());
                    if ui.button("Add New Material…").clicked() {
                        add_new = true;
                    }
                    if ui.button("Add Copy of Material…").clicked() {
                        add_copy = true;
                    }
                    if ui.button("Modify/Show Material…").clicked() {
                        modify = true;
                    }
                    if ui.add_enabled(names.len() > 1, egui::Button::new("Delete Material")).clicked() {
                        remove = true;
                    }
                    ui.separator();
                    let saved = app.material_draft.get(selected).map(|(id, _)| *id != 0).unwrap_or(false);
                    let has_selection = !app.selected_elements.is_empty();
                    if ui
                        .add_enabled(saved && has_selection, egui::Button::new("Assign to selected frames"))
                        .on_hover_text("Assign the selected material to the frames selected in the model")
                        .clicked()
                    {
                        assign = true;
                    }
                });
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("OK").clicked() {
                    apply = true;
                }
                if ui.button("Cancel").clicked() {
                    close = true;
                }
            });
            ui.label(
                egui::RichText::new(
                    "Materials store E and G in Pa, density in kg/m³ and strengths in Pa. A material still used by a frame or area cannot be deleted.",
                )
                .weak(),
            );
        });

    if add_new {
        app.open_new_material();
    }
    if add_copy {
        app.copy_selected_material();
    }
    if modify {
        app.open_material_editor();
    }
    if remove {
        app.delete_selected_material();
    }
    if assign {
        if let Some((id, _)) = app.material_draft.get(selected) {
            let id = *id;
            app.assign_material(id);
        }
    }
    if apply && app.apply_materials() {
        open = false;
    }
    if close {
        open = false;
    }
    app.define_materials_dialog_open = open;
}

fn add_material_window(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.new_material_open {
        return;
    }
    let mut open = app.new_material_open;
    let mut create = false;

    let material_type = app.new_material_type;
    let standards = material_library::standards(app.new_material_region, material_type);
    let grades = material_library::grades(app.new_material_region, material_type, app.new_material_standard);
    if app.new_material_standard >= standards.len() {
        app.new_material_standard = 0;
    }
    if app.new_material_grade >= grades.len() {
        app.new_material_grade = 0;
    }

    egui::Window::new("Add New Material Property")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(420.0)
        .show(ctx, |ui| {
            egui::Grid::new("add_material_grid").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                ui.label("Region:");
                egui::ComboBox::from_id_salt("new_material_region")
                    .selected_text(REGIONS[app.new_material_region.min(REGIONS.len() - 1)])
                    .show_ui(ui, |ui| {
                        for (index, name) in REGIONS.iter().enumerate() {
                            if ui.selectable_value(&mut app.new_material_region, index, *name).clicked() {
                                app.new_material_standard = 0;
                                app.new_material_grade = 0;
                            }
                        }
                    });
                ui.end_row();

                ui.label("Material Type:");
                egui::ComboBox::from_id_salt("new_material_type")
                    .selected_text(material_type.label())
                    .show_ui(ui, |ui| {
                        for option in MaterialType::ALL {
                            if ui.selectable_value(&mut app.new_material_type, option, option.label()).clicked() {
                                app.new_material_standard = 0;
                                app.new_material_grade = 0;
                            }
                        }
                    });
                ui.end_row();

                ui.label("Standard:");
                egui::ComboBox::from_id_salt("new_material_standard")
                    .selected_text(standards.get(app.new_material_standard).copied().unwrap_or("—"))
                    .show_ui(ui, |ui| {
                        for (index, name) in standards.iter().enumerate() {
                            ui.selectable_value(&mut app.new_material_standard, index, *name);
                        }
                    });
                ui.end_row();

                ui.label("Grade:");
                egui::ComboBox::from_id_salt("new_material_grade")
                    .selected_text(grades.get(app.new_material_grade).map(|grade| grade.name).unwrap_or("—"))
                    .show_ui(ui, |ui| {
                        for (index, grade) in grades.iter().enumerate() {
                            ui.selectable_value(&mut app.new_material_grade, index, grade.name);
                        }
                    });
                ui.end_row();
            });

            ui.separator();
            if let Some(grade) = grades.get(app.new_material_grade) {
                let strength = if material_type.is_concrete_like() {
                    format!("f'c {:.0} MPa", grade.fy / 1.0e6)
                } else {
                    format!("Fy {:.0} MPa · Fu {:.0} MPa", grade.fy / 1.0e6, grade.fu / 1.0e6)
                };
                ui.label(
                    egui::RichText::new(format!(
                        "E {:.0} MPa · ν {:.2} · α {:.3e} 1/°C · ρ {:.0} kg/m³ · {strength}",
                        grade.e / 1.0e6,
                        grade.nu,
                        grade.alpha,
                        grade.rho
                    ))
                    .weak(),
                );
            }
            ui.label(egui::RichText::new(format!("Weight density {:.3} kN/m³.", 9.806_65 / 1000.0 * grades.get(app.new_material_grade).map(|g| g.rho).unwrap_or(0.0))).weak());

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("OK").clicked() {
                    create = true;
                }
                if ui.button("Cancel").clicked() {
                    app.new_material_open = false;
                }
            });
        });

    if create {
        app.create_material_from_grade();
        app.new_material_open = false;
    } else {
        app.new_material_open = open;
    }
}

fn material_editor_window(app: &mut OpenStructApp, ctx: &egui::Context) {
    let Some(index) = app.material_editor else {
        return;
    };
    if app.material_edit.is_none() {
        app.material_editor = None;
        return;
    }
    let mut open = true;
    let mut commit = false;
    let mut cancel = false;

    egui::Window::new("Material Property Data")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(470.0)
        .show(ctx, |ui| {
            let Some(material) = app.material_edit.as_mut() else {
                return;
            };
            // G follows E and ν until the user types a value of its own.
            let e_before = material.e;
            let nu_before = material.nu;
            let derived_before = material.shear_modulus();
            let g_followed_e = (material.g - derived_before).abs() <= derived_before.abs() * 1e-9 + 1.0;

            ui.label(egui::RichText::new("General Data").strong());
            egui::Grid::new("material_general").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                ui.label("Material Name:");
                ui.add(egui::TextEdit::singleline(&mut material.name).desired_width(230.0));
                ui.end_row();

                ui.label("Material Type:");
                egui::ComboBox::from_id_salt("material_type")
                    .selected_text(material.material_type.label())
                    .show_ui(ui, |ui| {
                        for option in MaterialType::ALL {
                            ui.selectable_value(&mut material.material_type, option, option.label());
                        }
                    });
                ui.end_row();

                ui.label("Directional Symmetry Type:");
                ui.label("Isotropic");
                ui.end_row();

                ui.label("Display Color:");
                ui.color_edit_button_srgb(&mut material.color);
                ui.end_row();

                ui.label("Material Notes:");
                ui.add(
                    egui::TextEdit::multiline(&mut material.notes)
                        .desired_rows(2)
                        .desired_width(230.0)
                        .hint_text("Notes kept with this material"),
                );
                ui.end_row();
            });

            ui.separator();
            ui.label(egui::RichText::new("Material Weight and Mass").strong());
            ui.horizontal(|ui| {
                ui.radio_value(&mut app.material_mass_input, true, "Specify Mass Density");
                ui.radio_value(&mut app.material_mass_input, false, "Specify Weight Density");
            });
            egui::Grid::new("material_mass").num_columns(3).spacing([12.0, 6.0]).show(ui, |ui| {
                if app.material_mass_input {
                    ui.label("Mass per Unit Volume");
                    let mut mass = material.rho;
                    if ui
                        .add(egui::DragValue::new(&mut mass).speed(10.0).range(0.0..=25_000.0).max_decimals(3))
                        .changed()
                    {
                        material.rho = mass;
                    }
                    ui.label("kg/m³");
                    ui.end_row();
                    ui.label("Weight per Unit Volume");
                    ui.label(format!("{:.4}", material.weight_density_kn()));
                    ui.label("kN/m³");
                    ui.end_row();
                } else {
                    ui.label("Weight per Unit Volume");
                    let mut weight = material.weight_density_kn();
                    if ui
                        .add(egui::DragValue::new(&mut weight).speed(0.1).range(0.0..=250.0).max_decimals(4))
                        .changed()
                    {
                        material.set_weight_density_kn(weight);
                    }
                    ui.label("kN/m³");
                    ui.end_row();
                    ui.label("Mass per Unit Volume");
                    ui.label(format!("{:.3}", material.rho));
                    ui.label("kg/m³");
                    ui.end_row();
                }
            });

            ui.separator();
            ui.label(egui::RichText::new("Mechanical Property Data").strong());
            egui::Grid::new("material_mechanical").num_columns(3).spacing([12.0, 6.0]).show(ui, |ui| {
                ui.label("Modulus of Elasticity, E");
                let mut e_mpa = material.e / 1.0e6;
                if ui
                    .add(egui::DragValue::new(&mut e_mpa).speed(1000.0).range(1.0..=600_000.0).max_decimals(2))
                    .changed()
                {
                    material.e = e_mpa * 1.0e6;
                }
                ui.label("MPa");
                ui.end_row();

                ui.label("Poisson's Ratio, U");
                ui.add(egui::DragValue::new(&mut material.nu).speed(0.005).range(0.0..=0.49).max_decimals(3));
                ui.label("");
                ui.end_row();

                ui.label("Coefficient of Thermal Expansion, A");
                ui.add(egui::DragValue::new(&mut material.alpha).speed(1.0e-7).range(0.0..=5.0e-5).max_decimals(9));
                ui.label("1/°C");
                ui.end_row();

                ui.label("Shear Modulus, G");
                let mut g_mpa = material.g / 1.0e6;
                if ui
                    .add(egui::DragValue::new(&mut g_mpa).speed(1000.0).range(0.1..=300_000.0).max_decimals(2))
                    .changed()
                {
                    material.g = g_mpa * 1.0e6;
                }
                ui.horizontal(|ui| {
                    ui.label("MPa");
                    if ui
                        .small_button("↺")
                        .on_hover_text("Recalculate G from E and ν")
                        .clicked()
                    {
                        material.g = material.shear_modulus();
                    }
                });
                ui.end_row();
            });
            if g_followed_e && (material.e != e_before || material.nu != nu_before) {
                material.g = material.shear_modulus();
            }
            ui.label(
                egui::RichText::new(
                    "G follows E and ν unless you type a value of your own; ↺ restores the elastic value. All properties are stored with the material on OK.",
                )
                .weak(),
            );

            ui.separator();
            ui.label(egui::RichText::new("Design Property Data").strong());
            if ui.button("Modify/Show Material Property Design Data…").clicked() {
                app.material_show_design = !app.material_show_design;
            }
            if app.material_show_design {
                let concrete = material.material_type.is_concrete_like();
                egui::Grid::new("material_design").num_columns(3).spacing([12.0, 6.0]).show(ui, |ui| {
                    ui.label(if concrete { "Compressive strength, f'c" } else { "Yield strength, Fy" });
                    let mut strength = material.fy / 1.0e6;
                    if ui
                        .add(egui::DragValue::new(&mut strength).speed(1.0).range(0.0..=2000.0).max_decimals(2))
                        .changed()
                    {
                        material.fy = strength * 1.0e6;
                    }
                    ui.label("MPa");
                    ui.end_row();

                    ui.label("Ultimate strength, Fu");
                    let mut ultimate = material.fu / 1.0e6;
                    if ui
                        .add(egui::DragValue::new(&mut ultimate).speed(1.0).range(0.0..=2500.0).max_decimals(2))
                        .changed()
                    {
                        material.fu = ultimate * 1.0e6;
                    }
                    ui.label("MPa");
                    ui.end_row();
                });
            }

            ui.separator();
            ui.label(egui::RichText::new("Advanced Material Property Data").strong());
            if ui.button("Material Damping Properties…").clicked() {
                app.material_show_damping = !app.material_show_damping;
            }
            if app.material_show_damping {
                egui::Grid::new("material_damping").num_columns(3).spacing([12.0, 6.0]).show(ui, |ui| {
                    ui.label("Modal damping ratio");
                    ui.add(
                        egui::DragValue::new(&mut material.damping.modal_ratio)
                            .speed(0.005)
                            .range(0.0..=1.0)
                            .max_decimals(4),
                    );
                    ui.label("");
                    ui.end_row();
                    for (index, label) in ["U1", "U2", "U3", "R1", "R2", "R3"].iter().enumerate() {
                        ui.label(format!("Viscous damping, {label}"));
                        ui.add(
                            egui::DragValue::new(&mut material.damping.viscous[index])
                                .speed(0.5)
                                .range(0.0..=10_000.0)
                                .max_decimals(3),
                        );
                        ui.label("");
                        ui.end_row();
                    }
                });
            }

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("OK").clicked() {
                    commit = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        });

    if commit {
        if let Some(mut material) = app.material_edit.clone() {
            if material.g <= 0.0 {
                material.g = material.shear_modulus();
            }
            if let Some(entry) = app.material_draft.get_mut(index) {
                entry.1 = material;
            }
            app.material_selected = index;
        }
        app.material_edit = None;
        app.material_editor = None;
    } else if cancel || !open {
        app.material_edit = None;
        app.material_editor = None;
    }
}
