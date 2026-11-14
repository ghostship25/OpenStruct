//! Properties of Object: context-sensitive panel for the current selection.

use crate::app::OpenStructApp;
use crate::commands::Cmd;
use crate::theme;
use eframe::egui;
use openstruct::elements::CardinalPoint;

pub fn properties(app: &mut OpenStructApp, ui: &mut egui::Ui) {
    let pal = theme::palette(app.dark);
    ui.horizontal(|ui| {
        ui.strong("Properties of Object");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("Delete").clicked() {
                app.run(Cmd::Delete);
            }
        });
    });
    ui.separator();

    egui::ScrollArea::vertical().id_salt("properties_scroll").auto_shrink([false, false]).show(ui, |ui| {
        if app.selected_nodes.is_empty() && app.selected_elements.is_empty() && app.selected_areas.is_empty() {
            ui.label("No object selected.");
            ui.label(egui::RichText::new("Use Select (1) and click a joint or frame, or drag a window.").color(pal.text_dim));
            ui.separator();
            model_summary(app, ui);
            return;
        }

        if !app.selected_nodes.is_empty() {
            ui.heading("Joints");
            let mut ids: Vec<usize> = app.selected_nodes.iter().copied().collect();
            ids.sort_unstable();
            for id in ids.iter().take(8) {
                if let Some(node) = app.model.nodes.get(id) {
                    ui.label(format!("N{id}   ({:.3}, {:.3}, {:.3}) m", node.x, node.y, node.z));
                    ui.label(egui::RichText::new(format!("   {}", app.story_name_at(node.z))).color(pal.text_dim));
                }
            }
            if ids.len() > 8 {
                ui.label(egui::RichText::new(format!("… and {} more", ids.len() - 8)).color(pal.text_dim));
            }
            ui.separator();

            ui.label("Restraint");
            ui.horizontal(|ui| {
                if ui.button("Fixed").clicked() {
                    app.run(Cmd::FixityFixed);
                }
                if ui.button("Pinned").clicked() {
                    app.run(Cmd::FixityPinned);
                }
                if ui.button("Pinned+T").clicked() {
                    app.run(Cmd::FixityPinnedTorsion);
                }
            });
            ui.horizontal(|ui| {
                if ui.button("Roller Z").clicked() {
                    app.run(Cmd::FixityRoller);
                }
                if ui.button("Roller+T").clicked() {
                    app.run(Cmd::FixityRollerTorsion);
                }
                if ui.button("Free").clicked() {
                    app.run(Cmd::FixityFree);
                }
            });
            ui.separator();

            ui.label("Point load (kN)");
            ui.horizontal(|ui| {
                ui.label("FX");
                ui.add(egui::DragValue::new(&mut app.load_value[0]).speed(1.0).range(-1.0e6..=1.0e6));
                ui.label("FY");
                ui.add(egui::DragValue::new(&mut app.load_value[1]).speed(1.0).range(-1.0e6..=1.0e6));
            });
            ui.horizontal(|ui| {
                ui.label("FZ");
                ui.add(egui::DragValue::new(&mut app.load_value[2]).speed(1.0).range(-1.0e6..=1.0e6));
            });
            ui.horizontal(|ui| {
                if ui.button("Assign Load").clicked() {
                    app.run(Cmd::AssignPointLoad);
                }
                if ui.button("Remove Load").clicked() {
                    app.run(Cmd::RemovePointLoad);
                }
            });
            ui.separator();
        }

        if !app.selected_areas.is_empty() {
            ui.heading("Areas");
            let mut ids: Vec<usize> = app.selected_areas.iter().copied().collect();
            ids.sort_unstable();
            for id in ids.iter().take(8) {
                if let Some(area) = app.model.areas.get(id) {
                    ui.label(format!("A{id}   {}   {:.3} m²   t = {:.3} m", area.kind.label(), area.area(&app.model), area.thickness));
                    let joints: Vec<String> = area.node_ids.iter().map(|node| format!("N{node}")).collect();
                    ui.label(egui::RichText::new(format!("   joints: {}", joints.join(", "))).color(pal.text_dim));
                    if let Some(centroid) = area.centroid(&app.model) {
                        ui.label(
                            egui::RichText::new(format!(
                                "   centroid ({:.2}, {:.2}, {:.2}) · {}",
                                centroid[0],
                                centroid[1],
                                centroid[2],
                                app.story_name_at(centroid[2])
                            ))
                            .color(pal.text_dim),
                        );
                    }
                    if !area.is_valid(&app.model) {
                        ui.label(egui::RichText::new("   degenerate polygon").color(pal.warn));
                    }
                }
            }
            if ids.len() > 8 {
                ui.label(egui::RichText::new(format!("… and {} more", ids.len() - 8)).color(pal.text_dim));
            }
            ui.separator();
        }

        if !app.selected_elements.is_empty() {
            ui.heading("Frames");
            let mut ids: Vec<usize> = app.selected_elements.iter().copied().collect();
            ids.sort_unstable();
            for id in ids.iter().take(8) {
                let Some(element) = app.model.elements.get(id) else {
                    continue;
                };
                let length = match (app.model.nodes.get(&element.node_i), app.model.nodes.get(&element.node_j)) {
                    (Some(a), Some(b)) => ((b.x - a.x).powi(2) + (b.y - a.y).powi(2) + (b.z - a.z).powi(2)).sqrt(),
                    _ => 0.0,
                };
                let section = app.model.sections.get(&element.section_id).map(|section| section.name.as_str()).unwrap_or("?");
                ui.label(format!("F{id}   {} → {}   L = {:.3} m   [{}]", element.node_i, element.node_j, length, section));
                ui.label(egui::RichText::new(format!("   insertion: {}", element.cardinal_point.label())).color(pal.text_dim));
                if let Some(node) = app.model.nodes.get(&element.node_i) {
                    ui.label(egui::RichText::new(format!("   {}", app.story_name_at(node.z))).color(pal.text_dim));
                }
            }
            if ids.len() > 8 {
                ui.label(egui::RichText::new(format!("… and {} more", ids.len() - 8)).color(pal.text_dim));
            }
            ui.separator();
            let current = ids
                .first()
                .and_then(|id| app.model.elements.get(id))
                .map(|element| element.cardinal_point)
                .unwrap_or(app.cardinal_point);
            ui.label("Cardinal point for selected frames");
            egui::ComboBox::from_id_salt("bulk_cardinal")
                .width(150.0)
                .selected_text(current.label())
                .show_ui(ui, |ui| {
                    for point in CardinalPoint::ALL {
                        if ui.selectable_label(current == point, point.label()).clicked() {
                            app.assign_cardinal_to_selection(point);
                            ui.close();
                        }
                    }
                });
            ui.label(egui::RichText::new("Changes are one undo step.").weak());
            ui.separator();
        }

        ui.horizontal(|ui| {
            if ui.button("Delete Selection").clicked() {
                app.run(Cmd::Delete);
            }
            if ui.button("Select All").clicked() {
                app.run(Cmd::SelectAll);
            }
            if ui.button("Clear Selection").clicked() {
                app.run(Cmd::DeselectAll);
            }
        });
        ui.separator();
        ui.label(egui::RichText::new("Transform").strong());
        ui.horizontal(|ui| {
            if ui.small_button("Move…").clicked() {
                app.run(Cmd::MoveSelection);
            }
            if ui.small_button("Copy…").clicked() {
                app.run(Cmd::CopySelection);
            }
            if ui.small_button("Mirror…").clicked() {
                app.run(Cmd::MirrorSelection);
            }
            if ui.small_button("Replicate…").clicked() {
                app.run(Cmd::Replicate);
            }
        });

        ui.separator();
        model_summary(app, ui);
    });
}

fn model_summary(app: &mut OpenStructApp, ui: &mut egui::Ui) {
    let pal = theme::palette(app.dark);
    ui.heading("Model");
    ui.label(format!("Name: {}", app.model.name));
    ui.label(format!(
        "Joints: {}   Frames: {}   Areas: {}   Loaded joints: {}   Stories: {}",
        app.model.nodes.len(),
        app.model.elements.len(),
        app.model.areas.len(),
        app.nodal_loads.len(),
        app.model.stories.len()
    ));
    ui.label(egui::RichText::new(format!("Unit system: {}", app.units.label())).color(pal.text_dim));
    ui.separator();

    ui.label("Stories");
    for (index, story) in app.model.stories.iter().enumerate() {
        let joints = app.model.nodes.values().filter(|node| (node.z - story.elevation).abs() < 1e-6).count();
        let master = if app.master_story == Some(index) { "  ·  master" } else { "" };
        ui.label(format!("  {}  @ {:.2} m  ·  {} joints{}", story.name, story.elevation, joints, master));
    }
    ui.separator();
    ui.label("Materials");
    for material in app.model.materials.values() {
        ui.label(format!("  {}  ·  E = {:.3e} Pa  ·  ν = {:.2}", material.name, material.e, material.nu));
    }
    ui.separator();
    ui.label("Frame sections");
    for section in app.model.sections.values() {
        ui.label(format!("  {}  ·  A = {:.4} m²  ·  Iy = {:.3e}  ·  Iz = {:.3e}", section.name, section.area, section.iy, section.iz));
    }
}
