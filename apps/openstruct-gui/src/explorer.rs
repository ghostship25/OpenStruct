//! Model Explorer: stories, objects, properties, loads and results.

use crate::app::{BottomTab, OpenStructApp};
use crate::commands::Cmd;
use crate::theme;
use eframe::egui;

pub fn explorer(app: &mut OpenStructApp, ui: &mut egui::Ui) {
    let pal = theme::palette(app.dark);
    ui.horizontal(|ui| {
        ui.strong("Model Explorer");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("Fit").on_hover_text("Zoom to the active story / model").clicked() {
                app.pending_fit = Some(());
            }
        });
    });
    ui.separator();

    egui::ScrollArea::vertical().id_salt("explorer_scroll").auto_shrink([false, false]).show(ui, |ui| {
        egui::CollapsingHeader::new(format!("Model: {}", app.model.name)).default_open(true).show(ui, |ui| {
            egui::CollapsingHeader::new(format!("Stories ({})", app.model.stories.len())).default_open(true).show(ui, |ui| {
                let stories: Vec<(usize, String, f64)> =
                    app.model.stories.iter().enumerate().map(|(index, story)| (index, story.name.clone(), story.elevation)).collect();
                for (index, name, elevation) in stories {
                    let joints = app.model.nodes.values().filter(|node| (node.z - elevation).abs() < 1e-6).count();
                    let frames = app
                        .model
                        .elements
                        .values()
                        .filter(|element| match (app.model.nodes.get(&element.node_i), app.model.nodes.get(&element.node_j)) {
                            (Some(a), Some(b)) => (a.z - elevation).abs() < 1e-6 && (b.z - elevation).abs() < 1e-6,
                            _ => false,
                        })
                        .count();
                    let master = if app.master_story == Some(index) { "  ·  master" } else { "" };
                    let label = format!("{name}  @ {elevation:.2} m  ·  {joints}J / {frames}F{master}");
                    if ui.selectable_label(app.active_story == index, label).clicked() {
                        app.active_story = index;
                        app.draw_frame_start = None;
                        app.area_points.clear();
                        app.draw_wall_start = None;
                        app.pending_fit = Some(());
                        app.status_text = format!("Active story: {}.", app.active_story_name());
                    }
                }
                if ui.button("Story manager…").clicked() {
                    app.run(Cmd::DefineStories);
                }
            });

            egui::CollapsingHeader::new("Structure").default_open(true).show(ui, |ui| {
                if ui.selectable_label(app.bottom_tab == BottomTab::Joints, format!("Joints ({})", app.model.nodes.len())).clicked() {
                    app.bottom_tab = BottomTab::Joints;
                    app.show_tables = true;
                }
                if ui.selectable_label(app.bottom_tab == BottomTab::Forces, format!("Frames ({})", app.model.elements.len())).clicked() {
                    app.bottom_tab = BottomTab::Forces;
                    app.show_tables = true;
                }
                egui::CollapsingHeader::new(format!("Areas ({})", app.model.areas.len())).default_open(true).show(ui, |ui| {
                    let mut areas: Vec<(usize, String)> = app
                        .model
                        .areas
                        .iter()
                        .map(|(&id, area)| (id, format!("A{id}  ·  {}  ·  {:.2} m²  ·  t {:.2} m", area.kind.label(), area.area(&app.model), area.thickness)))
                        .collect();
                    areas.sort_by_key(|(id, _)| *id);
                    if areas.is_empty() {
                        ui.label(egui::RichText::new("Draw areas with tool 4.").color(pal.text_dim));
                    }
                    for (id, label) in areas {
                        if ui.selectable_label(app.selected_areas.contains(&id), label).clicked() {
                            app.selected_nodes.clear();
                            app.selected_elements.clear();
                            app.selected_areas.clear();
                            app.selected_areas.insert(id);
                            app.view_tab = crate::app::ViewTab::Plan;
                            app.status_text = format!("Selected area A{id}.");
                        }
                    }
                    ui.label(egui::RichText::new("Areas are display-only until the shell solver (Phase 4).").color(pal.text_dim));
                });
            });

            egui::CollapsingHeader::new(format!("Materials ({})", app.model.materials.len())).default_open(false).show(ui, |ui| {
                let mut materials: Vec<_> = app.model.materials.values().collect();
                materials.sort_by(|a, b| a.name.cmp(&b.name));
                for material in materials {
                    ui.label(format!("{}  ·  E = {:.2e} Pa", material.name, material.e));
                }
                if ui.button("Define materials…").clicked() {
                    app.run(Cmd::DefineMaterials);
                }
            });

            egui::CollapsingHeader::new(format!("Sections ({})", app.model.sections.len())).default_open(false).show(ui, |ui| {
                let mut sections: Vec<_> = app.model.sections.values().collect();
                sections.sort_by(|a, b| a.name.cmp(&b.name));
                for section in sections {
                    ui.label(format!("{}  ·  A = {:.4} m²  ·  Iy = {:.3e}", section.name, section.area, section.iy));
                }
                if ui.button("Define sections…").clicked() {
                    app.run(Cmd::DefineFrameSections);
                }
            });

            egui::CollapsingHeader::new("Loads").default_open(true).show(ui, |ui| {
                if ui
                    .selectable_label(false, format!("Point loads ({})", app.nodal_loads.len()))
                    .on_hover_text("Select the loaded joints in the plan view")
                    .clicked()
                {
                    app.selected_nodes = app.nodal_loads.keys().copied().collect();
                    app.selected_elements.clear();
                    app.selected_areas.clear();
                    app.view_tab = crate::app::ViewTab::Plan;
                    app.status_text = format!("Selected {} loaded joint(s).", app.selected_nodes.len());
                }
                ui.label(egui::RichText::new("Pattern: DEAD  ·  Case: CASE 1").color(pal.text_dim));
            });

            egui::CollapsingHeader::new("Analysis Results").default_open(true).show(ui, |ui| {
                match &app.results {
                    Some(results) => {
                        ui.label(egui::RichText::new("Solved").color(pal.ok));
                        ui.label(
                            egui::RichText::new(format!("{} joints  ·  {} frames", results.node_ids.len(), results.element_ids.len())).color(pal.text_dim),
                        );
                    }
                    None => {
                        ui.label(egui::RichText::new("Not run").color(pal.text_dim));
                    }
                }
                if ui.button("Run analysis (F5)").clicked() {
                    app.run(Cmd::RunAnalysis);
                }
            });
        });
    });
}
