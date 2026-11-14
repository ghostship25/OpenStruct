//! Floating dialogs: grid definition, stories, edit operations, command palette,
//! shortcuts and about.

mod new_model_dialog;
mod phase3;
mod phase4;

use crate::app::{EditMode, OpenStructApp, SelectScope};
use crate::canvas::{BubbleLoc, GridSpec, RefPlane, RefPlaneAxis};
use crate::commands::Cmd;
use eframe::egui;
use openstruct::areas::AreaKind;
use openstruct::edit::{AlignAxis, MirrorAxis};
use openstruct::model::Story;

pub fn windows(app: &mut OpenStructApp, ctx: &egui::Context) {
    new_model_dialog::new_model_window(app, ctx);
    grid_window(app, ctx);
    ref_plane_window(app, ctx);
    story_window(app, ctx);
    edit_window(app, ctx);
    align_window(app, ctx);
    grid_members_window(app, ctx);
    typed_coord_window(app, ctx);
    select_property_window(app, ctx);
    group_window(app, ctx);
    area_section_window(app, ctx);
    display_options_window(app, ctx);
    palette_window(app, ctx);
    shortcuts_window(app, ctx);
    about_window(app, ctx);
    phase3::open_materials_dialog(app, ctx);
    phase3::open_frame_sections_dialog(app, ctx);
    phase3::open_section_designer_dialog(app, ctx);
    phase3::open_patterns_dialog(app, ctx);
    phase3::open_cases_dialog(app, ctx);
    phase3::open_combos_dialog(app, ctx);
    phase3::open_preferences_dialog(app, ctx);
    phase3::open_frame_loads_dialog(app, ctx);
    phase3::open_area_loads_dialog(app, ctx);
    phase3::open_frame_releases_dialog(app, ctx);
    phase3::open_joint_springs_dialog(app, ctx);
    phase3::open_temperature_loads_dialog(app, ctx);
    phase3::open_diaphragm_dialog(app, ctx);
    phase4::windows(app, ctx);
}

fn typed_coord_window(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.typed_coord_dialog_open {
        return;
    }
    let (u_label, v_label) = app.active_work_plane().axis_labels();
    let plane = app.work_plane_name();
    let mut open = app.typed_coord_dialog_open;
    let mut add_joint = false;
    let mut add_frame = false;
    let mut close = false;

    egui::Window::new("Type Coordinates").open(&mut open).collapsible(false).resizable(false).default_width(390.0).show(ctx, |ui| {
        ui.label(format!("Plane-local coordinates on {plane}. Values are stored in metres."));
        ui.separator();
        ui.strong("Joint");
        ui.horizontal(|ui| {
            ui.label(format!("{u_label} (m)"));
            ui.add(egui::DragValue::new(&mut app.typed_coord_u).speed(0.1));
            ui.label(format!("{v_label} (m)"));
            ui.add(egui::DragValue::new(&mut app.typed_coord_v).speed(0.1));
        });
        ui.separator();
        ui.strong("Frame");
        ui.label("Second end; the first end uses the joint coordinates above.");
        ui.horizontal(|ui| {
            ui.label(format!("{u_label}₂ (m)"));
            ui.add(egui::DragValue::new(&mut app.typed_coord_u2).speed(0.1));
            ui.label(format!("{v_label}₂ (m)"));
            ui.add(egui::DragValue::new(&mut app.typed_coord_v2).speed(0.1));
        });
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("Create Joint").clicked() {
                add_joint = true;
            }
            if ui.button("Create Frame").clicked() {
                add_frame = true;
            }
            if ui.button("Close").clicked() {
                close = true;
            }
        });
        ui.label(egui::RichText::new("Typed geometry uses the active story or reference plane and is one undo step.").weak());
    });
    if close {
        open = false;
    }

    if add_joint {
        app.create_typed_joint();
    } else if add_frame {
        app.create_typed_frame();
    }
    app.typed_coord_dialog_open = open;
}

fn select_property_window(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.select_property_dialog_open {
        return;
    }
    let materials: Vec<(usize, String)> = app.model.materials.iter().map(|(&id, material)| (id, material.name.clone())).collect();
    let sections: Vec<(usize, String)> = app.model.sections.iter().map(|(&id, section)| (id, section.name.clone())).collect();
    let mut open = app.select_property_dialog_open;
    let mut apply = false;
    let mut close = false;

    egui::Window::new("Select by Property").open(&mut open).collapsible(false).resizable(false).default_width(420.0).show(ctx, |ui| {
        ui.label("Select objects by object family, level and assigned material or section.");
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Objects:");
            egui::ComboBox::from_id_salt("select_scope").selected_text(app.select_scope.label()).show_ui(ui, |ui| {
                for scope in SelectScope::ALL {
                    ui.selectable_value(&mut app.select_scope, scope, scope.label());
                }
            });
        });
        ui.checkbox(&mut app.select_active_story_only, "Only the active story / work plane");
        ui.horizontal(|ui| {
            ui.label("Material:");
            egui::ComboBox::from_id_salt("select_material").selected_text(material_label(&materials, app.select_material)).show_ui(ui, |ui| {
                ui.selectable_value(&mut app.select_material, 0, "Any material");
                for (id, name) in &materials {
                    ui.selectable_value(&mut app.select_material, *id, name);
                }
            });
        });
        ui.horizontal(|ui| {
            ui.label("Frame section:");
            egui::ComboBox::from_id_salt("select_section").selected_text(section_label(&sections, app.select_section)).show_ui(ui, |ui| {
                ui.selectable_value(&mut app.select_section, 0, "Any section");
                for (id, name) in &sections {
                    ui.selectable_value(&mut app.select_section, *id, name);
                }
            });
        });
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("Select").clicked() {
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
        app.select_by_property();
    } else {
        app.select_property_dialog_open = open;
    }
}

fn material_label(materials: &[(usize, String)], id: usize) -> String {
    if id == 0 { "Any material".to_string() } else { materials.iter().find(|(material, _)| *material == id).map(|(_, name)| name.clone()).unwrap_or_else(|| "Any material".to_string()) }
}

fn section_label(sections: &[(usize, String)], id: usize) -> String {
    if id == 0 { "Any section".to_string() } else { sections.iter().find(|(section, _)| *section == id).map(|(_, name)| name.clone()).unwrap_or_else(|| "Any section".to_string()) }
}

fn group_window(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.group_dialog_open {
        return;
    }
    let mut open = app.group_dialog_open;
    let mut choice = app.group_choice;
    let mut close = false;
    let mut select = false;
    let mut replace = false;
    let mut create = false;
    let mut delete = false;

    egui::Window::new("Groups").open(&mut open).collapsible(false).resizable(true).default_width(480.0).show(ctx, |ui| {
        ui.label("Named selection sets are saved with the model and participate in undo/redo.");
        ui.separator();
        if app.groups.is_empty() {
            ui.label(egui::RichText::new("No groups yet. Select objects and create one below.").weak());
        } else {
            egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                for (index, group) in app.groups.iter().enumerate() {
                    let label = format!("{}  ·  {} object(s)", group.name, group.count());
                    if ui.selectable_label(choice == index, label).clicked() {
                        choice = index;
                    }
                }
            });
        }
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Name:");
            ui.add(egui::TextEdit::singleline(&mut app.group_name).desired_width(180.0).hint_text("Group name"));
            if ui.button("Create / update from selection").clicked() {
                create = true;
            }
        });
        ui.horizontal(|ui| {
            if ui.add_enabled(!app.groups.is_empty(), egui::Button::new("Select group")).clicked() {
                select = true;
            }
            if ui.add_enabled(!app.groups.is_empty(), egui::Button::new("Replace with selection")).clicked() {
                replace = true;
            }
            if ui.add_enabled(!app.groups.is_empty(), egui::Button::new("Delete group")).clicked() {
                delete = true;
            }
            if ui.button("Close").clicked() {
                close = true;
            }
        });
        ui.label(egui::RichText::new("Select objects first, then create a group; selecting a group replaces the current selection.").weak());
    });
    if close {
        open = false;
    }

    app.group_choice = choice.min(app.groups.len().saturating_sub(1));
    if create {
        app.create_group_from_selection();
    } else if replace {
        app.replace_group_from_selection();
    } else if select {
        app.select_group();
    } else if delete {
        app.delete_group();
    }
    app.group_dialog_open = open;
}

fn area_section_window(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.area_section_dialog_open {
        return;
    }
    let materials: Vec<(usize, String)> = app.model.materials.iter().map(|(&id, material)| (id, material.name.clone())).collect();
    let mut open = app.area_section_dialog_open;
    let mut apply = false;
    let mut close = false;

    egui::Window::new("Area Sections").open(&mut open).collapsible(false).resizable(false).default_width(390.0).show(ctx, |ui| {
        ui.label(format!("{} area(s) selected.", app.selected_areas.len()));
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Kind:");
            egui::ComboBox::from_id_salt("area_section_kind").selected_text(app.area_section_kind.label()).show_ui(ui, |ui| {
                ui.selectable_value(&mut app.area_section_kind, AreaKind::Slab, "Slab");
                ui.selectable_value(&mut app.area_section_kind, AreaKind::Wall, "Wall");
                ui.selectable_value(&mut app.area_section_kind, AreaKind::Deck, "Deck");
            });
        });
        ui.horizontal(|ui| {
            ui.label("Thickness (m):");
            ui.add(egui::DragValue::new(&mut app.area_section_thickness).range(0.001..=10.0).speed(0.01));
        });
        ui.horizontal(|ui| {
            ui.label("Material:");
            egui::ComboBox::from_id_salt("area_section_material").selected_text(material_label(&materials, app.area_section_material)).show_ui(ui, |ui| {
                for (id, name) in &materials {
                    ui.selectable_value(&mut app.area_section_material, *id, name);
                }
            });
        });
        ui.separator();
        ui.label(egui::RichText::new("The assignment is one undo step and is saved with the model.").weak());
        ui.horizontal(|ui| {
            if ui.button("Apply to selection").clicked() {
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
        app.apply_area_sections();
    } else {
        app.area_section_dialog_open = open;
    }
}

fn display_options_window(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.display_options_open {
        return;
    }
    let mut open = app.display_options_open;
    let mut close = false;
    egui::Window::new("Display Options").open(&mut open).collapsible(false).resizable(false).default_width(360.0).show(ctx, |ui| {
        ui.label("Choose which model aids and objects the 2D and 3D views draw.");
        ui.separator();
        ui.checkbox(&mut app.show_grid, "Grid and story outlines");
        ui.checkbox(&mut app.show_joints, "Joints");
        ui.checkbox(&mut app.show_frames, "Frames");
        ui.checkbox(&mut app.show_areas, "Areas and walls");
        ui.checkbox(&mut app.show_labels, "Object and story labels");
        ui.checkbox(&mut app.show_supports, "Support symbols");
        ui.checkbox(&mut app.show_dimensions, "Grid and story dimensions");
        ui.checkbox(&mut app.show_ref_planes, "Reference planes");
        ui.checkbox(&mut app.show_loads, "Load symbols");
        ui.add_enabled_ui(app.show_loads, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                ui.vertical(|ui| {
                    ui.checkbox(&mut app.show_frame_loads, "Frame loads in the 3D view");
                    ui.checkbox(&mut app.show_area_loads, "Area loads in the 3D view");
                    ui.add(egui::Slider::new(&mut app.load_symbol_scale, 0.3..=3.0).text("symbol size"));
                });
            });
        });
        ui.checkbox(&mut app.show_deformed, "Deformed shape");
        ui.separator();
        if ui.button("Close").clicked() {
            close = true;
        }
    });
    if close {
        open = false;
    }
    app.display_options_open = open;
}

fn grid_window(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.grid_dialog_open {
        return;
    }
    let mut open = app.grid_dialog_open;
    let mut apply = false;
    egui::Window::new("Grid System Data").open(&mut open).collapsible(false).resizable(true).default_width(780.0).show(ctx, |ui| {
        egui::ScrollArea::vertical().max_height(440.0).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label("Grid System Name:");
            ui.add(egui::TextEdit::singleline(&mut app.grid_name).desired_width(160.0));
        });
        egui::Grid::new("grid_system_origin").num_columns(4).spacing([12.0, 6.0]).show(ui, |ui| {
            ui.label("System Origin — Global X:");
            ui.add(egui::DragValue::new(&mut app.grid_origin_x).speed(0.1).suffix(" m"));
            ui.label("Global Y:");
            ui.add(egui::DragValue::new(&mut app.grid_origin_y).speed(0.1).suffix(" m"));
            ui.end_row();
            ui.label("Rotation:");
            ui.add(
                egui::DragValue::new(&mut app.grid_rotation_deg)
                    .speed(1.0)
                    .range(-360.0..=360.0)
                    .suffix(" deg"),
            )
            .on_hover_text("Rotation of the grid about its origin, counter-clockwise");
            ui.label("Bubble size:");
            ui.add(
                egui::DragValue::new(&mut app.grid_bubble_mm)
                    .speed(25.0)
                    .range(100.0..=10_000.0)
                    .suffix(" mm"),
            )
            .on_hover_text("Radius of the grid label bubbles");
            ui.end_row();
        });
        ui.separator();
        ui.horizontal(|ui| {
            ui.radio_value(&mut app.grid_cylindrical, false, "Rectangular");
            ui.radio_value(&mut app.grid_cylindrical, true, "Cylindrical");
        });
        ui.separator();
        if app.grid_cylindrical {
            ui.horizontal(|ui| {
                ui.label("Centre X (m):");
                ui.add(egui::DragValue::new(&mut app.cyl_center_x).speed(0.5));
                ui.label("Y:");
                ui.add(egui::DragValue::new(&mut app.cyl_center_y).speed(0.5));
            });
            ui.horizontal(|ui| {
                ui.label("Ring spacing (m):");
                ui.add(egui::DragValue::new(&mut app.cyl_ring_spacing).range(0.5..=50.0).speed(0.5));
            });
            ui.horizontal(|ui| {
                ui.label("Circumferential rings:");
                ui.add(egui::DragValue::new(&mut app.cyl_rings).range(1..=40).speed(1.0));
            });
            ui.horizontal(|ui| {
                ui.label("Radial lines:");
                ui.add(egui::DragValue::new(&mut app.cyl_angle_count).range(3..=180).speed(1.0));
            });
            ui.horizontal(|ui| {
                ui.label("Start angle (°):");
                ui.add(egui::DragValue::new(&mut app.cyl_start_angle).range(-180.0..=180.0).speed(5.0));
            });
            ui.horizontal(|ui| {
                ui.label("Radial step (°):");
                ui.add(egui::DragValue::new(&mut app.cyl_angle_step).range(1.0..=180.0).speed(1.0));
            });
            ui.label(egui::RichText::new("Rings are labelled R1…Rn and radial lines A1…An, starting at the start angle.").weak());
        } else {
            ui.horizontal(|ui| {
                ui.radio_value(&mut app.grid_display_as_spacing, false, "Display Grid Data as Ordinates");
                ui.radio_value(&mut app.grid_display_as_spacing, true, "Display Grid Data as Spacing");
            });
            if app.grid_draft_xs.len() < 2 {
                app.grid_draft_xs = vec![0.0, app.grid_spacing.max(0.5)];
            }
            if app.grid_draft_ys.len() < 2 {
                app.grid_draft_ys = vec![0.0, app.grid_spacing.max(0.5)];
            }
            let spacing = app.grid_display_as_spacing;
            let step = app.grid_spacing.max(0.5);
            let mut xs = std::mem::take(&mut app.grid_draft_xs);
            let mut ys = std::mem::take(&mut app.grid_draft_ys);
            let mut x_visible = std::mem::take(&mut app.grid_draft_x_visible);
            let mut y_visible = std::mem::take(&mut app.grid_draft_y_visible);
            let mut x_bubble = std::mem::take(&mut app.grid_draft_x_bubble);
            let mut y_bubble = std::mem::take(&mut app.grid_draft_y_bubble);
            ui.horizontal_top(|ui| {
                grid_line_table(ui, "X Grid Data", &mut xs, &mut x_visible, &mut x_bubble, spacing, step, 300.0);
                grid_line_table(ui, "Y Grid Data", &mut ys, &mut y_visible, &mut y_bubble, spacing, step, 300.0);
            });
            app.grid_draft_x_visible = x_visible;
            app.grid_draft_y_visible = y_visible;
            app.grid_draft_x_bubble = x_bubble;
            app.grid_draft_y_bubble = y_bubble;
            app.grid_draft_xs = xs;
            app.grid_draft_ys = ys;
        }
        ui.separator();
        ui.label(egui::RichText::new("Grid labels").strong());
        ui.label(egui::RichText::new("Blank labels use the default numbering; custom labels appear in plan, elevation and 3D.").weak());
        if app.grid_cylindrical {
            let ring_count = app.cyl_rings.max(1) as usize;
            let radial_count = app.cyl_angle_count.max(1) as usize;
            app.cyl_ring_labels.resize(ring_count, String::new());
            app.cyl_radial_labels.resize(radial_count, String::new());
            egui::ScrollArea::vertical().max_height(170.0).id_salt("cyl_grid_labels").show(ui, |ui| {
                egui::Grid::new("cyl_grid_label_grid").num_columns(2).striped(true).show(ui, |ui| {
                    for (index, label) in app.cyl_ring_labels.iter_mut().enumerate() {
                        ui.label(format!("Ring {}", index + 1));
                        ui.add(egui::TextEdit::singleline(label).desired_width(130.0).hint_text(format!("R{}", index + 1)));
                        ui.end_row();
                    }
                    for (index, label) in app.cyl_radial_labels.iter_mut().enumerate() {
                        ui.label(format!("Radial {}", index + 1));
                        ui.add(egui::TextEdit::singleline(label).desired_width(130.0).hint_text(format!("A{}", index + 1)));
                        ui.end_row();
                    }
                });
            });
        } else {
            let x_count = app.grid_draft_xs.len().max(2);
            let y_count = app.grid_draft_ys.len().max(2);
            app.grid_x_labels.resize(x_count, String::new());
            app.grid_y_labels.resize(y_count, String::new());
            egui::ScrollArea::vertical().max_height(170.0).id_salt("rect_grid_labels").show(ui, |ui| {
                egui::Grid::new("rect_grid_label_grid").num_columns(2).striped(true).show(ui, |ui| {
                    for (index, label) in app.grid_x_labels.iter_mut().enumerate() {
                        ui.label(format!("Line at constant X ({})", index + 1));
                        ui.add(egui::TextEdit::singleline(label).desired_width(130.0).hint_text(format!("{}", index + 1)));
                        ui.end_row();
                    }
                    for (index, label) in app.grid_y_labels.iter_mut().enumerate() {
                        ui.label(format!("Line at constant Y ({})", crate::canvas::default_axis_label(index)));
                        ui.add(egui::TextEdit::singleline(label).desired_width(130.0).hint_text(crate::canvas::default_axis_label(index)));
                        ui.end_row();
                    }
                });
            });
        }
        });
        ui.separator();
        ui.label(egui::RichText::new("Applying a new grid replaces the current grid lines. Existing objects are kept.").weak());
        ui.horizontal(|ui| {
            if ui.button("Apply").clicked() {
                apply = true;
            }
            if ui.button("Cancel").clicked() {
                app.grid_dialog_open = false;
            }
        });
    });

    if apply {
        app.push_undo();
        if app.grid_cylindrical {
            let rings = app.cyl_rings.max(1) as usize;
            let lines = app.cyl_angle_count.max(3) as usize;
            let spacing = app.cyl_ring_spacing.max(0.5);
            let mut grid = GridSpec::cylindrical((app.cyl_center_x, app.cyl_center_y), spacing, rings, app.cyl_start_angle, app.cyl_angle_step.max(1.0), lines);
            if let Some(cylindrical) = grid.cylindrical.as_mut() {
                cylindrical.ring_labels = trimmed_labels(&app.cyl_ring_labels, rings);
                cylindrical.radial_labels = trimmed_labels(&app.cyl_radial_labels, lines);
            }
            app.grid = grid;
            app.status_text = format!("Cylindrical grid: {rings} rings at {spacing:.2} m, {lines} radial lines at {:.1}°.", app.cyl_angle_step.max(1.0));
        } else {
            let mut xs = app.grid_draft_xs.clone();
            let mut ys = app.grid_draft_ys.clone();
            xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            ys.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            if xs.len() < 2 {
                xs = vec![0.0, app.grid_spacing.max(0.5)];
            }
            if ys.len() < 2 {
                ys = vec![0.0, app.grid_spacing.max(0.5)];
            }
            let mut grid = GridSpec::from_ordinates(xs, ys);
            grid.x_labels = trimmed_labels(&app.grid_x_labels, grid.xs.len());
            grid.y_labels = trimmed_labels(&app.grid_y_labels, grid.ys.len());
            grid.x_visible = padded_flags(&app.grid_draft_x_visible, grid.xs.len());
            grid.y_visible = padded_flags(&app.grid_draft_y_visible, grid.ys.len());
            grid.x_bubble = padded_bubbles(&app.grid_draft_x_bubble, grid.xs.len());
            grid.y_bubble = padded_bubbles(&app.grid_draft_y_bubble, grid.ys.len());
            grid.name = if app.grid_name.trim().is_empty() {
                crate::canvas::default_grid_name()
            } else {
                app.grid_name.trim().to_string()
            };
            grid.origin = (app.grid_origin_x, app.grid_origin_y);
            grid.rotation_deg = app.grid_rotation_deg;
            grid.bubble_size = (app.grid_bubble_mm / 1000.0).max(0.05);
            grid.display_as_spacing = app.grid_display_as_spacing;
            app.grid_spacing = grid.xs.windows(2).next().map(|pair| pair[1] - pair[0]).unwrap_or(5.0);
            app.grid_nx = grid.xs.len().saturating_sub(1) as i32;
            app.grid_ny = grid.ys.len().saturating_sub(1) as i32;
            app.status_text = format!(
                "Grid {}: {} × {} lines, origin ({:.2}, {:.2}) m, rotated {:.1}°.",
                grid.name,
                grid.xs.len(),
                grid.ys.len(),
                grid.origin.0,
                grid.origin.1,
                grid.rotation_deg
            );
            app.grid = grid;
        }
        app.pending_fit = Some(());
        app.grid_dialog_open = false;
    } else {
        app.grid_dialog_open = open;
    }
}

/// One standard grid data table: line numbers, ordinates or spacings, add/delete/sort.
fn grid_line_table(
    ui: &mut egui::Ui,
    title: &str,
    ordinates: &mut Vec<f64>,
    visible: &mut Vec<bool>,
    bubble: &mut Vec<BubbleLoc>,
    spacing: bool,
    default_step: f64,
    width: f32,
) {
    visible.resize(ordinates.len(), true);
    bubble.resize(ordinates.len(), BubbleLoc::Both);
    ui.vertical(|ui| {
        ui.set_width(width);
        ui.label(egui::RichText::new(title).strong());
        egui::Frame::group(ui.style()).show(ui, |ui| {
            egui::Grid::new(title).striped(true).num_columns(5).spacing([8.0, 4.0]).show(ui, |ui| {
                ui.strong("Line");
                ui.strong(if spacing { "Spacing (m)" } else { "Ordinate (m)" });
                ui.strong("Visible").on_hover_text("Hidden lines are not drawn; snapping still uses them");
                ui.strong("Bubble");
                ui.strong("");
                ui.end_row();
                let mut remove: Option<usize> = None;
                for index in 0..ordinates.len() {
                    ui.label(format!("{}", index + 1));
                    if spacing {
                        let previous = if index == 0 { 0.0 } else { ordinates[index - 1] };
                        let mut gap = ordinates[index] - previous;
                        if ui
                            .add(egui::DragValue::new(&mut gap).speed(0.1).range(0.0..=200.0))
                            .on_hover_text(if index == 0 {
                                "Offset of the first line from the grid origin"
                            } else {
                                "Distance from the line above"
                            })
                            .changed()
                        {
                            ordinates[index] = previous + gap;
                        }
                    } else {
                        ui.add(egui::DragValue::new(&mut ordinates[index]).speed(0.1).range(-1000.0..=1000.0));
                    }
                    ui.checkbox(&mut visible[index], "")
                        .on_hover_text("Hidden lines are not drawn; snapping still uses them");
                    egui::ComboBox::from_id_salt((title, "bubble", index))
                        .width(72.0)
                        .selected_text(bubble[index].label())
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut bubble[index], BubbleLoc::Both, "Both");
                            ui.selectable_value(&mut bubble[index], BubbleLoc::Start, "Start");
                            ui.selectable_value(&mut bubble[index], BubbleLoc::End, "End");
                        });
                    if ordinates.len() > 2 && ui.small_button("✕").on_hover_text("Delete this line").clicked() {
                        remove = Some(index);
                    }
                    ui.end_row();
                }
                if let Some(index) = remove {
                    ordinates.remove(index);
                    if index < visible.len() {
                        visible.remove(index);
                    }
                    if index < bubble.len() {
                        bubble.remove(index);
                    }
                }
            });
            ui.horizontal(|ui| {
                if ui.button("Add").clicked() {
                    let step = ordinates
                        .windows(2)
                        .next()
                        .map(|pair| pair[1] - pair[0])
                        .unwrap_or(default_step)
                        .max(0.5);
                    let next = ordinates.last().copied().unwrap_or(0.0) + step;
                    ordinates.push(next);
                    visible.push(true);
                    bubble.push(BubbleLoc::Both);
                }
                if ui.button("Sort").clicked() {
                    ordinates.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                }
            });
        });
    });
}

/// Labels trimmed and padded to the grid line count; empty entries keep defaults.
fn trimmed_labels(labels: &[String], count: usize) -> Vec<String> {
    (0..count).map(|index| labels.get(index).map(|label| label.trim().to_string()).unwrap_or_default()).collect()
}

/// Display flags padded to the grid line count; missing entries stay visible.
fn padded_flags(flags: &[bool], count: usize) -> Vec<bool> {
    (0..count).map(|index| flags.get(index).copied().unwrap_or(true)).collect()
}

/// Bubble locations padded to the grid line count; missing entries use Both.
fn padded_bubbles(bubbles: &[BubbleLoc], count: usize) -> Vec<BubbleLoc> {
    (0..count).map(|index| bubbles.get(index).copied().unwrap_or_default()).collect()
}

fn ref_plane_window(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.ref_dialog_open {
        return;
    }
    let mut open = app.ref_dialog_open;
    let mut apply = false;
    let mut remove: Option<usize> = None;

    egui::Window::new("Reference Planes").open(&mut open).collapsible(false).resizable(true).default_width(520.0).show(ctx, |ui| {
        ui.label("Reference planes are drawing aids on the datum; they are not structural objects.");
        ui.separator();
        egui::Grid::new("ref_plane_grid").striped(true).num_columns(4).show(ui, |ui| {
            ui.strong("Name");
            ui.strong("Normal");
            ui.strong("Offset (m)");
            ui.strong("");
            ui.end_row();
            for (index, plane) in app.ref_draft.iter_mut().enumerate() {
                ui.add(egui::TextEdit::singleline(&mut plane.name).desired_width(140.0));
                egui::ComboBox::from_id_salt(("ref_axis", index)).selected_text(plane.axis.label()).show_ui(ui, |ui| {
                    ui.selectable_value(&mut plane.axis, RefPlaneAxis::X, "X");
                    ui.selectable_value(&mut plane.axis, RefPlaneAxis::Y, "Y");
                    ui.selectable_value(&mut plane.axis, RefPlaneAxis::Z, "Z");
                });
                ui.add(egui::DragValue::new(&mut plane.offset).speed(0.1));
                if ui.small_button("✕").on_hover_text("Remove this reference plane").clicked() {
                    remove = Some(index);
                }
                ui.end_row();
            }
        });
        if ui.button("+ Add plane").clicked() {
            let number = app.ref_draft.len() + 1;
            let axis = app.ref_draft.last().map(|plane| plane.axis).unwrap_or(RefPlaneAxis::X);
            app.ref_draft.push(RefPlane::new(format!("Ref {number}"), axis, 0.0));
        }
        if let Some(index) = remove {
            app.ref_draft.remove(index);
        }
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("Apply").clicked() {
                apply = true;
            }
            if ui.button("Cancel").clicked() {
                app.ref_dialog_open = false;
            }
        });
        ui.label(egui::RichText::new("X and Y planes draw as dashed lines in plan. Every plane draws in 3D.").weak());
    });

    if apply {
        app.push_undo();
        for (index, plane) in app.ref_draft.iter_mut().enumerate() {
            plane.name = plane.name.trim().to_string();
            if plane.name.is_empty() {
                plane.name = format!("Ref {}", index + 1);
            }
        }
        app.ref_planes = app.ref_draft.clone();
        if let Some(work) = app.work_plane {
            if !app.ref_planes.iter().any(|plane| plane.axis == work.axis && (plane.offset - work.offset).abs() < 1e-9) {
                app.work_plane = None;
            }
        }
        app.status_text = format!("{} reference plane(s) defined.", app.ref_planes.len());
        app.ref_dialog_open = false;
    } else {
        app.ref_dialog_open = open;
    }
}

fn align_window(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.align_dialog_open {
        return;
    }
    let mut open = app.align_dialog_open;
    let mut apply = false;

    egui::Window::new("Align Selection").open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
        ui.label("Set one coordinate for every selected joint, frame end and area corner.");
        ui.horizontal(|ui| {
            ui.label("Align to:");
            egui::ComboBox::from_id_salt("align_axis").selected_text(app.align_axis.label()).show_ui(ui, |ui| {
                ui.selectable_value(&mut app.align_axis, AlignAxis::X, "X");
                ui.selectable_value(&mut app.align_axis, AlignAxis::Y, "Y");
                ui.selectable_value(&mut app.align_axis, AlignAxis::Z, "Z");
            });
        });
        ui.checkbox(&mut app.align_average, "Use the average of the selection");
        if !app.align_average {
            ui.horizontal(|ui| {
                ui.label("Target (m):");
                ui.add(egui::DragValue::new(&mut app.align_value).speed(0.1));
            });
        }
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("Apply").clicked() {
                apply = true;
            }
            if ui.button("Cancel").clicked() {
                app.align_dialog_open = false;
            }
        });
    });

    if apply {
        app.apply_align();
    } else {
        app.align_dialog_open = open;
    }
}

fn grid_members_window(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.grid_members_dialog_open {
        return;
    }
    let mut open = app.grid_members_dialog_open;
    let mut apply = false;
    let grid_kind = if app.grid.cylindrical.is_some() { "cylindrical" } else { "rectangular" };

    egui::Window::new("Create Frames / Walls on Grid").open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
        ui.label(format!("Build along selected lines of the {grid_kind} grid on {}.", app.work_plane_name()));
        ui.separator();
        ui.label(egui::RichText::new("Grid lines to use").strong());
        if app.grid.cylindrical.is_some() {
            ui.checkbox(&mut app.grid_members_rings, "Rings (circumferential chords)");
            ui.checkbox(&mut app.grid_members_radials, "Radials (spokes)");
        } else {
            ui.checkbox(&mut app.grid_members_x_lines, "Lines at constant X (1, 2, 3…)");
            ui.checkbox(&mut app.grid_members_y_lines, "Lines at constant Y (A, B, C…)");
        }
        ui.horizontal(|ui| {
            if ui.small_button("Select all").clicked() {
                app.grid_members_x_lines = true;
                app.grid_members_y_lines = true;
                app.grid_members_rings = true;
                app.grid_members_radials = true;
            }
            if ui.small_button("Select none").clicked() {
                app.grid_members_x_lines = false;
                app.grid_members_y_lines = false;
                app.grid_members_rings = false;
                app.grid_members_radials = false;
            }
        });
        ui.separator();
        ui.checkbox(&mut app.grid_members_frames, "Frames at the work-plane elevation");
        ui.checkbox(&mut app.grid_members_walls, "Walls above the grid lines");
        if app.grid_members_walls {
            ui.horizontal(|ui| {
                ui.label("Wall height (m):");
                ui.add(egui::DragValue::new(&mut app.grid_members_wall_height).range(0.5..=50.0).speed(0.1));
            });
        }
        ui.separator();
        ui.label(egui::RichText::new("Existing joints and frames on the same positions are reused; existing walls are not duplicated.").weak());
        ui.horizontal(|ui| {
            if ui.button("Create").clicked() {
                apply = true;
            }
            if ui.button("Cancel").clicked() {
                app.grid_members_dialog_open = false;
            }
        });
    });

    if apply {
        app.create_grid_members();
    } else {
        app.grid_members_dialog_open = open;
    }
}

fn story_window(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.story_dialog_open {
        return;
    }
    let mut open = app.story_dialog_open;
    let mut apply = false;
    let mut add = false;
    let mut remove: Option<usize> = None;
    let mut activate: Option<usize> = None;
    let mut master: Option<usize> = None;

    egui::Window::new("Story Manager")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(760.0)
        .min_width(760.0)
        .show(ctx, |ui| {
        ui.label("Stories give objects their level. The story marked (active) receives new objects; the master story sets the typical story height.");
        ui.separator();
        let next_stories: Vec<Option<(usize, f64)>> = app
            .story_draft
            .iter()
            .map(|story| {
                app.story_draft
                    .iter()
                    .enumerate()
                    .filter(|(_, other)| other.elevation > story.elevation + 1e-6)
                    .min_by(|a, b| a.1.elevation.partial_cmp(&b.1.elevation).unwrap_or(std::cmp::Ordering::Equal))
                    .map(|(index, other)| (index, other.elevation))
            })
            .collect();
        let story_names: Vec<String> = app.story_draft.iter().map(|story| story.name.trim().to_string()).collect();
        let mut height_edits: Vec<(usize, f64)> = Vec::new();
        // Ten Story Data columns, sized to fit the wider default window.
        egui::Grid::new("story_grid").striped(true).num_columns(10).show(ui, |ui| {
                ui.strong("Active").on_hover_text("Story that receives new objects");
                ui.strong("Story");
                ui.strong("Height").on_hover_text("Height to the level above, in metres");
                ui.strong("Elevation").on_hover_text("Elevation in metres");
                ui.strong("Master").on_hover_text("Use this story's height as the typical height");
                ui.strong("Similar To");
                ui.strong("Splice").on_hover_text("Column splices are taken at this level for steel design");
                ui.strong("Splice Ht").on_hover_text("Splice height above the story elevation, in metres");
                ui.strong("Color");
                ui.strong("Objects");
                ui.end_row();
                for (index, story) in app.story_draft.iter_mut().enumerate() {
                    let is_active = app.active_story == index;
                    if ui.radio(is_active, "").clicked() {
                        activate = Some(index);
                    }
                    ui.add_sized(
                        [120.0, ui.spacing().interact_size.y],
                        egui::TextEdit::singleline(&mut story.name),
                    );
                    match next_stories[index] {
                        Some((next_index, next)) => {
                            let mut height = next - story.elevation;
                            if ui
                                .add(
                                    egui::DragValue::new(&mut height)
                                        .speed(0.1)
                                        .range(0.1..=1000.0)
                                        .max_decimals(2),
                                )
                                .on_hover_text("Distance to the level above, in metres; editing moves that level")
                                .changed()
                            {
                                height_edits.push((next_index, story.elevation + height));
                            }
                        }
                        None => {
                            ui.label(egui::RichText::new("— top —").weak());
                        }
                    }
                    ui.add(
                        egui::DragValue::new(&mut story.elevation)
                            .speed(0.1)
                            .range(-1000.0..=1000.0)
                            .max_decimals(2),
                    );
                    let mut is_master = app.story_master_draft == Some(index);
                    if ui
                        .checkbox(&mut is_master, "")
                        .on_hover_text("Use this story's height as the typical height")
                        .clicked()
                    {
                        master = Some(index);
                    }
                    let similar_label = story.similar_to_label().to_string();
                    egui::ComboBox::from_id_salt(("story_similar_to", index))
                        .width(80.0)
                        .selected_text(similar_label)
                        .show_ui(ui, |ui| {
                            if ui.selectable_label(story.similar_to.is_none(), "None").clicked() {
                                story.similar_to = None;
                            }
                            for name in &story_names {
                                if name.is_empty() || *name == story.name {
                                    continue;
                                }
                                let selected = story.similar_to.as_deref() == Some(name.as_str());
                                if ui.selectable_label(selected, name).clicked() {
                                    story.similar_to = Some(name.clone());
                                }
                            }
                        });
                    ui.checkbox(&mut story.splice_story, "")
                        .on_hover_text("Column splices are taken at this level for steel design");
                    ui.add_enabled(
                        story.splice_story,
                        egui::DragValue::new(&mut story.splice_height)
                            .speed(0.05)
                            .range(0.0..=20.0)
                            .max_decimals(2),
                    )
                    .on_hover_text("Splice height above the story elevation, in metres");
                    ui.color_edit_button_srgb(&mut story.color)
                        .on_hover_text("Story colour shown in the table");
                    let count = app.model.nodes.values().filter(|node| (node.z - story.elevation).abs() < 1e-6).count();
                    ui.horizontal(|ui| {
                        ui.label(format!("{count}")).on_hover_text("joints on this level");
                        if ui.small_button("✕").on_hover_text("Remove this story (objects stay at their elevation)").clicked() {
                            remove = Some(index);
                        }
                    });
                    ui.end_row();
                }
                // The base level is a reference line, not an editable story.
                ui.label("");
                ui.label(egui::RichText::new("Base").weak());
                ui.label("");
                ui.label(egui::RichText::new("0.00").weak());
                ui.label(egui::RichText::new("—").weak());
                ui.label(egui::RichText::new("None").weak());
                ui.label(egui::RichText::new("No").weak());
                ui.label(egui::RichText::new("0").weak());
                ui.label("");
                ui.label("");
                ui.end_row();
        });
        ui.label(
            egui::RichText::new(
                "Similar To groups typical stories; a splice story marks where column splices fall for steel design. Both are stored with the model.",
            )
            .weak(),
        );
        for (index, elevation) in height_edits {
            if let Some(story) = app.story_draft.get_mut(index) {
                story.elevation = elevation;
            }
        }
        if let Some(index) = app.story_master_draft {
            if let Some(master) = app.story_draft.get(index) {
                let above = app
                    .story_draft
                    .iter()
                    .filter(|story| story.elevation > master.elevation + 1e-6)
                    .map(|story| story.elevation)
                    .fold(None::<f64>, |best, level| Some(best.map_or(level, |current| current.min(level))));
                if let Some(next) = above {
                    ui.label(format!("Typical height from the master story: {:.2} m.", next - master.elevation));
                }
            }
        }
        if let Some(index) = remove {
            if app.story_draft.len() > 1 {
                app.story_draft.remove(index);
                match app.story_master_draft {
                    Some(current) if current == index => app.story_master_draft = None,
                    Some(current) if current > index => app.story_master_draft = Some(current - 1),
                    _ => {}
                }
                if app.active_story >= app.story_draft.len() {
                    app.active_story = app.story_draft.len().saturating_sub(1);
                }
            } else {
                app.status_text = "A model needs at least one story.".to_string();
            }
        }
        ui.horizontal(|ui| {
            if ui.button("Add story").clicked() {
                add = true;
            }
            if ui.button("Apply").clicked() {
                apply = true;
            }
            if ui.button("Cancel").clicked() {
                app.story_dialog_open = false;
            }
        });
        if add {
            let typical = app
                .story_master_draft
                .and_then(|index| app.story_draft.get(index))
                .and_then(|master| {
                    app.story_draft
                        .iter()
                        .filter(|story| story.elevation > master.elevation + 1e-6)
                        .map(|story| story.elevation - master.elevation)
                        .fold(None::<f64>, |best, height| Some(best.map_or(height, |current| current.min(height))))
                })
                .unwrap_or(3.5);
            let name = if app.new_story_name.trim().is_empty() { format!("Story{}", app.story_draft.len() + 1) } else { app.new_story_name.trim().to_string() };
            app.story_draft.push(Story::new(&name, app.new_story_elevation));
            let top = app.story_draft.iter().map(|story| story.elevation).fold(0.0_f64, f64::max);
            app.new_story_elevation = top + typical;
            app.new_story_name = format!("Story{}", app.story_draft.len() + 1);
        }
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Next story:");
            ui.add(egui::TextEdit::singleline(&mut app.new_story_name).desired_width(110.0));
            ui.add(egui::DragValue::new(&mut app.new_story_elevation).speed(0.1).prefix("z = "));
        });
    });

    if let Some(index) = master {
        app.story_master_draft = Some(index);
    }
    if let Some(index) = activate {
        app.active_story = index.min(app.story_draft.len().saturating_sub(1));
        app.draw_frame_start = None;
        app.pending_fit = Some(());
    }
    if apply {
        app.apply_story_draft();
    } else {
        app.story_dialog_open = open;
    }
}

fn edit_window(app: &mut OpenStructApp, ctx: &egui::Context) {
    let Some(mode) = app.edit_mode else {
        return;
    };
    let mut apply = false;
    let mut cancel = false;

    egui::Window::new(mode.title()).collapsible(false).resizable(false).default_width(380.0).show(ctx, |ui| {
        ui.label(format!("{} joint(s), {} frame(s), {} area(s) selected.", app.selected_nodes.len(), app.selected_elements.len(), app.selected_areas.len()));
        ui.separator();

        match mode {
            EditMode::Move | EditMode::Copy | EditMode::Replicate => {
                ui.horizontal(|ui| {
                    ui.label("ΔX (m)");
                    ui.add(egui::DragValue::new(&mut app.edit_dx).speed(0.1));
                    ui.label("ΔY (m)");
                    ui.add(egui::DragValue::new(&mut app.edit_dy).speed(0.1));
                });
                ui.horizontal(|ui| {
                    ui.label("ΔZ (m)");
                    ui.add(egui::DragValue::new(&mut app.edit_dz).speed(0.1));
                });
                if mode == EditMode::Replicate {
                    ui.horizontal(|ui| {
                        ui.label("Copies:");
                        ui.add(egui::DragValue::new(&mut app.edit_copies).range(1..=100).speed(1.0));
                    });
                    ui.label(egui::RichText::new("Copies are spaced by the delta, starting one delta away.").weak());
                }
            }
            EditMode::Mirror => {
                ui.horizontal(|ui| {
                    ui.label("Axis:");
                    ui.radio_value(&mut app.edit_axis, MirrorAxis::X, "X (across X = pivot)");
                    ui.radio_value(&mut app.edit_axis, MirrorAxis::Y, "Y (across Y = pivot)");
                });
                ui.horizontal(|ui| {
                    ui.label("Pivot (m):");
                    ui.add(egui::DragValue::new(&mut app.edit_pivot).speed(0.1));
                });
                ui.label(egui::RichText::new("The mirrored copy is created and selected; the original is kept.").weak());
            }
            EditMode::Rotate => {
                ui.horizontal(|ui| {
                    ui.label("Angle (°)");
                    ui.add(egui::DragValue::new(&mut app.edit_angle).speed(1.0).range(-360.0..=360.0));
                });
                ui.horizontal(|ui| {
                    ui.label("Pivot X (m)");
                    ui.add(egui::DragValue::new(&mut app.edit_pivot).speed(0.1));
                    ui.label("Y");
                    ui.add(egui::DragValue::new(&mut app.edit_pivot_y).speed(0.1));
                });
                ui.label(egui::RichText::new("Positive angles turn counter-clockwise in plan; frames and areas follow their joints.").weak());
            }
        }

        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("Apply").clicked() {
                apply = true;
            }
            if ui.button("Cancel").clicked() {
                cancel = true;
            }
        });
    });

    if apply {
        app.apply_edit_dialog();
    } else if cancel {
        app.edit_mode = None;
    }
}

fn palette_window(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.palette_open {
        return;
    }
    let mut open = app.palette_open;
    let mut chosen: Option<Cmd> = None;

    egui::Window::new("Command Palette").open(&mut open).collapsible(false).resizable(false).default_width(540.0).show(ctx, |ui| {
        let response =
            ui.add(egui::TextEdit::singleline(&mut app.palette_query).hint_text("Type a command — Enter runs the first match").desired_width(f32::INFINITY));
        if !response.has_focus() {
            response.request_focus();
        }
        ui.separator();

        let mut matches: Vec<(i32, Cmd)> = Cmd::ALL.iter().filter_map(|&cmd| cmd.score(&app.palette_query).map(|score| (score, cmd))).collect();
        matches.sort_by(|a, b| b.0.cmp(&a.0));

        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
        egui::ScrollArea::vertical().max_height(330.0).show(ui, |ui| {
            if matches.is_empty() {
                ui.label("No matching command.");
            }
            for (_, cmd) in matches.iter().take(24) {
                let mut text = cmd.label().to_string();
                if let Some(shortcut) = cmd.shortcut() {
                    text.push_str("    ");
                    text.push_str(shortcut);
                }
                if let Some(phase) = cmd.planned() {
                    text.push_str(&format!("   ·  Phase {phase}"));
                }
                if ui.selectable_label(false, text).clicked() {
                    chosen = Some(*cmd);
                }
            }
        });

        if enter {
            chosen = matches.first().map(|(_, cmd)| *cmd);
        }
    });

    if let Some(cmd) = chosen {
        app.palette_open = false;
        app.run(cmd);
    } else {
        app.palette_open = open;
    }
}

fn shortcuts_window(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.shortcuts_open {
        return;
    }
    let mut open = app.shortcuts_open;
    egui::Window::new("Keyboard Shortcuts").open(&mut open).resizable(true).default_width(460.0).show(ctx, |ui| {
        egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
            egui::Grid::new("shortcut_grid").striped(true).num_columns(2).show(ui, |ui| {
                ui.strong("Command");
                ui.strong("Shortcut");
                ui.end_row();
                for cmd in Cmd::ALL {
                    if let Some(shortcut) = cmd.shortcut() {
                        ui.label(cmd.label());
                        ui.label(shortcut);
                        ui.end_row();
                    }
                }
            });
        });
        ui.separator();
        ui.label("Tools: 1 Select · 2 Draw Joints · 3 Draw Frames. Ctrl+K opens the command palette.");
        ui.label("3D: left-drag orbit · Shift-drag/middle-drag pan · wheel zoom.");
    });
    app.shortcuts_open = open;
}

fn about_window(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.about_open {
        return;
    }
    let mut open = app.about_open;
    egui::Window::new("About OpenStruct").open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
        ui.heading("OpenStruct");
        ui.label("Open-source structural analysis — Rust core + egui desktop shell.");
        ui.separator();
        ui.label("Version 0.1.0 · linear-static frame and building analysis");
        ui.label("Core: 3D frame linear static solver, SI units.");
        ui.label("See README.md for capabilities and known limitations.");
        ui.separator();
        ui.label("Open file formats only; no proprietary binaries are required.");
    });
    app.about_open = open;
}
