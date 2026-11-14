//! Phase 4 and 5 dialogs: analysis options, model checking, diaphragms and
//! their assignment, results setup, story results, frame diagrams, area
//! contours, member design, detailing, reports, documentation and verification
//! cases.
//!
//! The windows stay thin: each one reads the model and the stored analysis
//! results and calls the library for the work (checks, design, reports). The
//! only state a window keeps is what it must show while the user edits it.

use crate::app::OpenStructApp;
use crate::commands::Cmd;
use eframe::egui;
use openstruct::analysis::{Analysis, AnalysisResults, Extremes};
use openstruct::check::{check_model, CheckReport, CheckSeverity};
use openstruct::design::codes::DesignCode;
use openstruct::design::concrete::ConcreteStrength;
use openstruct::design::steel::DesignStatus;
use openstruct::elements::Element;
use openstruct::loads::{DistributedLoad, LoadCase, LoadDirection, LoadType, NodalLoad};
use openstruct::materials::{Material, MaterialType};
use openstruct::model::Diaphragm;
use openstruct::reporting::{self, HtmlReport, ReportConfig};
use openstruct::sections::Section;

/// Show every phase 4/5 window.
pub fn windows(app: &mut OpenStructApp, ctx: &egui::Context) {
    e2k_import_report_window(app, ctx);
    export_e2k_window(app, ctx);
    diaphragms_window(app, ctx);
    assign_diaphragm_window(app, ctx);
    analysis_options_window(app, ctx);
    model_check_window(app, ctx);
    results_setup_window(app, ctx);
    story_results_window(app, ctx);
    frame_diagrams_window(app, ctx);
    area_contours_window(app, ctx);
    steel_design_window(app, ctx);
    concrete_design_window(app, ctx);
    composite_design_window(app, ctx);
    shear_wall_design_window(app, ctx);
    detailing_window(app, ctx);
    reports_window(app, ctx);
    documentation_window(app, ctx);
    verification_window(app, ctx);
}

/// Largest magnitude in an envelope field, zero when it never received a value.
fn peak(extremes: &Extremes) -> f64 {
    let high = if extremes.max.is_finite() { extremes.max.abs() } else { 0.0 };
    let low = if extremes.min.is_finite() { extremes.min.abs() } else { 0.0 };
    high.max(low)
}

/// Envelope values for a member: axial, strong moment, weak moment, shear.
fn member_forces(app: &OpenStructApp, element_id: usize) -> Option<(f64, f64, f64, f64)> {
    let envelope = app.member_envelopes.iter().find(|envelope| envelope.element_id == element_id)?;
    let strong = peak(&envelope.moment_z_i).max(peak(&envelope.moment_z_j));
    let weak = peak(&envelope.moment_y_i).max(peak(&envelope.moment_y_j));
    let shear = peak(&envelope.shear_y).max(peak(&envelope.shear_z));
    Some((peak(&envelope.axial), strong, weak, shear))
}

fn member_length(app: &OpenStructApp, element: &Element) -> f64 {
    let (Some(a), Some(b)) = (app.model.nodes.get(&element.node_i), app.model.nodes.get(&element.node_j)) else {
        return 0.0;
    };
    ((b.x - a.x).powi(2) + (b.y - a.y).powi(2) + (b.z - a.z).powi(2)).sqrt()
}

fn status_colour(pass: bool) -> egui::Color32 {
    if pass {
        egui::Color32::from_rgb(46, 160, 67)
    } else {
        egui::Color32::from_rgb(220, 80, 70)
    }
}

fn component_label(component: usize) -> &'static str {
    match component {
        0 => "M3 — strong-axis moment",
        1 => "M2 — weak-axis moment",
        2 => "V2 — shear",
        3 => "N — axial force",
        _ => "M3 — strong-axis moment",
    }
}

/// End ordinates of one diagram component for a member.
fn diagram_values(app: &OpenStructApp, element_id: usize, component: usize) -> Option<(f64, f64)> {
    let envelope = app.member_envelopes.iter().find(|envelope| envelope.element_id == element_id)?;
    Some(match component {
        1 => (envelope.moment_y_i.max, envelope.moment_y_j.max),
        2 => (envelope.shear_y.max, envelope.shear_y.min),
        3 => (envelope.axial.min, envelope.axial.max),
        _ => (envelope.moment_z_i.max, envelope.moment_z_j.max),
    })
}

// -- file ---------------------------------------------------------------------

/// What an E2K `.e2k` import read, and the one thing to do next: analyse it.
fn e2k_import_report_window(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.e2k_report_dialog_open {
        return;
    }
    let Some(report) = app.import_e2k_report.clone() else {
        app.e2k_report_dialog_open = false;
        return;
    };
    let mut open = app.e2k_report_dialog_open;
    let mut run = false;

    egui::Window::new("E2K Import Report")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(520.0)
        .show(ctx, |ui| {
            ui.label(format!("Read with the file's own units: {}.", report.units));
            ui.separator();
            egui::Grid::new("e2k_import_counts").num_columns(2).striped(true).spacing([16.0, 4.0]).show(ui, |ui| {
                for (label, value) in [
                    ("Stories", report.stories),
                    ("Materials", report.materials),
                    ("Frame sections", report.sections),
                    ("Joints", report.points),
                    ("Frames", report.frames),
                    ("Areas", report.areas),
                    ("Load cases", report.load_cases),
                    ("Load combinations", report.load_combinations),
                    ("Joint loads", report.nodal_loads),
                    ("Frame loads", report.frame_loads),
                    ("Area loads", report.area_loads),
                ] {
                    ui.label(label);
                    ui.label(value.to_string());
                    ui.end_row();
                }
            });
            if let Some(master) = &report.master_story {
                ui.label(format!("Master story from the file: {master}."));
            }
            if !report.warnings.is_empty() {
                ui.separator();
                ui.label(egui::RichText::new("What the import did not do").strong());
                for warning in &report.warnings {
                    ui.label(egui::RichText::new(format!("• {warning}")).weak());
                }
            }
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Run Analysis").clicked() {
                    run = true;
                }
                if ui.button("Close").clicked() {
                    app.e2k_report_dialog_open = false;
                }
            });
            ui.label(
                egui::RichText::new("The imported frames, supports and loads are in the model; Save As writes them as .ostrut.")
                    .weak(),
            );
        });

    if run {
        app.e2k_report_dialog_open = false;
        app.run(Cmd::RunAnalysis);
    } else {
        app.e2k_report_dialog_open = open;
    }
}

fn export_e2k_window(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.export_e2k_dialog_open {
        return;
    }
    let mut open = app.export_e2k_dialog_open;
    let mut export = false;
    let mut close = false;

    egui::Window::new("Export E2K Text")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(440.0)
        .show(ctx, |ui| {
            ui.label("Write the whole model as E2K text: materials, sections, joints and supports, frames, stories, diaphragms and load cases with their loads.");
            ui.separator();
            ui.label(format!(
                "{} joint(s) · {} frame(s) · {} area(s) · {} load case(s) · {} diaphragm(s)",
                app.model.nodes.len(),
                app.model.elements.len(),
                app.model.areas.len(),
                app.model.load_cases.len(),
                app.model.diaphragms.len()
            ));
            ui.label(egui::RichText::new("The same file opens again through Import → E2K text, so it is a readable archive of the model.").weak());
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Choose file and export…").clicked() {
                    export = true;
                }
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
        });

    if export {
        let Some(mut path) = rfd::FileDialog::new().add_filter("E2K text", &["e2k", "txt"]).save_file() else {
            return;
        };
        if path.extension().is_none() {
            path.set_extension("e2k");
        }
        let text = openstruct::e2k_text::export_e2k(&app.model);
        match std::fs::write(&path, text) {
            Ok(()) => app.status_text = format!("Exported the model to {}.", path.display()),
            Err(err) => app.status_text = format!("Export failed: {err}"),
        }
        close = true;
    }
    if close {
        open = false;
    }
    app.export_e2k_dialog_open = open;
}

// -- diaphragms ---------------------------------------------------------------

fn diaphragms_window(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.define_diaphragms_dialog_open {
        return;
    }
    let mut open = app.define_diaphragms_dialog_open;
    let mut apply = false;
    let mut close = false;
    let mut add = false;
    let mut remove: Option<usize> = None;
    let levels: Vec<(String, f64)> = app.model.stories.iter().map(|story| (story.name.clone(), story.elevation)).collect();
    let active_level = app.model.stories.get(app.active_story).map(|story| story.elevation).unwrap_or(0.0);
    // Joint counts per draft row, read before the draft is borrowed mutably in the grid.
    let joint_counts: Vec<usize> = app
        .diaphragm_draft
        .iter()
        .map(|diaphragm| app.model.nodes.values().filter(|node| node.diaphragm == Some(diaphragm.id)).count())
        .collect();

    egui::Window::new("Define Diaphragms")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(540.0)
        .show(ctx, |ui| {
            ui.label("A diaphragm ties the joints on one floor so the level moves as a rigid plate. Assign joints to it with Assign → Diaphragms.");
            ui.separator();
            if app.diaphragm_draft.is_empty() {
                ui.label(egui::RichText::new("No diaphragms defined yet.").weak());
            }
            egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                egui::Grid::new("diaphragm_grid").striped(true).num_columns(5).show(ui, |ui| {
                    ui.strong("Name");
                    ui.strong("Level (m)");
                    ui.strong("Out of plane");
                    ui.strong("Joints");
                    ui.strong("");
                    ui.end_row();
                    for (index, diaphragm) in app.diaphragm_draft.iter_mut().enumerate() {
                        ui.add(egui::TextEdit::singleline(&mut diaphragm.name).desired_width(130.0));
                        ui.horizontal(|ui| {
                            ui.add(egui::DragValue::new(&mut diaphragm.story).speed(0.1).range(-1000.0..=1000.0));
                            if !levels.is_empty() {
                                egui::ComboBox::from_id_salt(("diaphragm_level", index))
                                    .selected_text("level")
                                    .width(58.0)
                                    .show_ui(ui, |ui| {
                                        for (name, elevation) in &levels {
                                            if ui.selectable_label(false, format!("{name} ({elevation:.2})")).clicked() {
                                                diaphragm.story = *elevation;
                                            }
                                        }
                                    });
                            }
                        });
                        ui.checkbox(&mut diaphragm.out_of_plane, "");
                        ui.label(format!("{}", joint_counts.get(index).copied().unwrap_or(0)));
                        if ui.small_button("✕").on_hover_text("Remove this diaphragm").clicked() {
                            remove = Some(index);
                        }
                        ui.end_row();
                    }
                });
            });
            ui.horizontal(|ui| {
                if ui.button("+ Add diaphragm").clicked() {
                    add = true;
                }
                if ui.button("Apply").clicked() {
                    apply = true;
                }
                if ui.button("Cancel").clicked() {
                    close = true;
                }
            });
            ui.label(egui::RichText::new("Removing a diaphragm clears it from the joints that used it. The rigid-plate constraint is checked by Check Model; condensing it into the stiffness matrix is still on the Phase.md list.").weak());
        });

    if let Some(index) = remove {
        app.diaphragm_draft.remove(index);
    }
    if add {
        let id = app.diaphragm_draft.iter().map(|diaphragm| diaphragm.id).max().unwrap_or(0) + 1;
        app.diaphragm_draft.push(Diaphragm::new(id, &format!("D{id}"), active_level));
    }
    if apply {
        app.push_undo();
        let mut draft = app.diaphragm_draft.clone();
        for (index, diaphragm) in draft.iter_mut().enumerate() {
            diaphragm.id = index + 1;
            let name = diaphragm.name.trim().to_string();
            diaphragm.name = if name.is_empty() { format!("D{}", index + 1) } else { name };
        }
        let valid: Vec<usize> = draft.iter().map(|diaphragm| diaphragm.id).collect();
        for node in app.model.nodes.values_mut() {
            if let Some(id) = node.diaphragm {
                if !valid.contains(&id) {
                    node.diaphragm = None;
                }
            }
        }
        app.model.diaphragms = draft.clone();
        app.diaphragm_draft = draft;
        app.dirty = true;
        app.status_text = format!("{} diaphragm(s) defined.", app.model.diaphragms.len());
        close = true;
    }
    if close {
        open = false;
    }
    app.define_diaphragms_dialog_open = open;
}

fn assign_diaphragm_window(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.assign_diaphragm_dialog_open {
        return;
    }
    let mut open = app.assign_diaphragm_dialog_open;
    let mut assign = false;
    let mut clear = false;
    let mut close = false;
    let diaphragms: Vec<(usize, String, f64)> = app
        .model
        .diaphragms
        .iter()
        .map(|diaphragm| (diaphragm.id, diaphragm.name.clone(), diaphragm.story))
        .collect();
    let selected = app.selected_nodes.len();
    let choice = app.assign_diaphragm_choice;

    egui::Window::new("Assign Diaphragm")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(430.0)
        .show(ctx, |ui| {
            ui.label(format!("{selected} joint(s) selected."));
            ui.separator();
            if diaphragms.is_empty() {
                ui.label(egui::RichText::new("Define a diaphragm first (Define → Diaphragms).").weak());
            } else {
                ui.horizontal(|ui| {
                    ui.label("Diaphragm:");
                    egui::ComboBox::from_id_salt("assign_diaphragm").selected_text(
                        diaphragms
                            .iter()
                            .find(|(id, _, _)| *id == choice)
                            .map(|(_, name, _)| name.clone())
                            .unwrap_or_else(|| "Choose…".to_string()),
                    )
                    .show_ui(ui, |ui| {
                        for (id, name, level) in &diaphragms {
                            ui.selectable_value(&mut app.assign_diaphragm_choice, *id, format!("{name}  (z = {level:.2} m)"));
                        }
                    });
                });
                ui.label(egui::RichText::new("Joints on another level are reported by Check Model.").weak());
            }
            ui.separator();
            ui.horizontal(|ui| {
                if ui.add_enabled(selected > 0 && !diaphragms.is_empty(), egui::Button::new("Assign to selection")).clicked() {
                    assign = true;
                }
                if ui.add_enabled(selected > 0, egui::Button::new("Clear assignment")).clicked() {
                    clear = true;
                }
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
        });

    if assign {
        app.push_undo();
        let id = app.assign_diaphragm_choice;
        for node_id in &app.selected_nodes {
            if let Some(node) = app.model.nodes.get_mut(node_id) {
                node.diaphragm = Some(id);
            }
        }
        app.dirty = true;
        app.status_text = format!("Diaphragm assigned to {} joint(s).", app.selected_nodes.len());
        close = true;
    }
    if clear {
        app.push_undo();
        for node_id in &app.selected_nodes {
            if let Some(node) = app.model.nodes.get_mut(node_id) {
                node.diaphragm = None;
            }
        }
        app.dirty = true;
        app.status_text = format!("Diaphragm cleared on {} joint(s).", app.selected_nodes.len());
        close = true;
    }
    if close {
        open = false;
    }
    app.assign_diaphragm_dialog_open = open;
}

// -- analysis ----------------------------------------------------------------

fn analysis_options_window(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.analysis_options_dialog_open {
        return;
    }
    let mut open = app.analysis_options_dialog_open;
    let mut close = false;

    egui::Window::new("Analysis Options")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(460.0)
        .show(ctx, |ui| {
            ui.label("How Run Analysis (F5) behaves for this session.");
            ui.separator();
            ui.checkbox(&mut app.check_before_solve, "Check the model first and stop on errors");
            ui.checkbox(&mut app.solve_combinations, "Solve the load combinations and build design envelopes");
            ui.checkbox(&mut app.auto_deformed, "Show the deformed shape after solving");
            ui.separator();
            ui.label(egui::RichText::new("The linear static solver assembles the 3D frame stiffness matrix and solves it directly. Shells, P-Delta and modal cases are not part of this solve; areas are reported and skipped.").weak());
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Run now").clicked() {
                    close = true;
                    app.run_analysis();
                }
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
        });

    if close {
        open = false;
    }
    app.analysis_options_dialog_open = open;
}

fn model_check_window(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.model_check_dialog_open {
        return;
    }
    let mut open = app.model_check_dialog_open;
    let mut rerun = false;
    let mut close = false;

    egui::Window::new("Model Check")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(640.0)
        .show(ctx, |ui| {
            let report = app.model_check_report.as_ref();
            match report {
                None => {
                    ui.label("No check has run yet.");
                }
                Some(report) => {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("{} error(s)", report.errors()))
                                .color(status_colour(report.errors() == 0)),
                        );
                        ui.label(format!("· {} warning(s)", report.warnings()));
                        ui.label(format!("· {} note(s)", report.infos()));
                        ui.separator();
                        ui.label(if report.can_analyse() { "Ready to analyse." } else { "Fix the errors before analysing." });
                    });
                    ui.separator();
                    egui::ScrollArea::vertical().max_height(380.0).show(ui, |ui| {
                        for severity in [CheckSeverity::Error, CheckSeverity::Warning, CheckSeverity::Info] {
                            let items = report.by_severity(severity);
                            if items.is_empty() {
                                continue;
                            }
                            ui.label(egui::RichText::new(severity.label()).strong());
                            egui::Grid::new(("check_grid", severity.label())).striped(true).num_columns(3).show(ui, |ui| {
                                for item in items {
                                    ui.label(egui::RichText::new(item.category.label()).weak());
                                    ui.label(&item.object);
                                    ui.label(&item.message);
                                    ui.end_row();
                                }
                            });
                            ui.add_space(6.0);
                        }
                        if report.items.is_empty() {
                            ui.label(egui::RichText::new("No problems found.").weak());
                        }
                    });
                }
            }
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Run check").clicked() {
                    rerun = true;
                }
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
        });

    if rerun {
        let report: CheckReport = check_model(&app.model);
        app.status_text = format!("Model check: {}", report.summary());
        app.model_check_report = Some(report);
    }
    if close {
        open = false;
    }
    app.model_check_dialog_open = open;
}

fn results_setup_window(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.results_setup_dialog_open {
        return;
    }
    let mut open = app.results_setup_dialog_open;
    let mut close = false;
    let combos: Vec<(usize, String)> = app
        .combo_results
        .iter()
        .enumerate()
        .map(|(index, combo)| (index, combo.name.clone()))
        .collect();

    egui::Window::new("Results Setup")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(460.0)
        .show(ctx, |ui| {
            ui.label("Choose which solved set the Displacements, Reactions and Forces tables read.");
            ui.separator();
            let current = match app.result_source {
                None => "First load case".to_string(),
                Some(index) => combos
                    .iter()
                    .find(|(position, _)| *position == index)
                    .map(|(_, name)| format!("Combination {name}"))
                    .unwrap_or_else(|| "First load case".to_string()),
            };
            ui.label(format!("Tables currently show: {current}"));
            ui.horizontal(|ui| {
                ui.label("Source:");
                egui::ComboBox::from_id_salt("results_source").selected_text(current).show_ui(ui, |ui| {
                    ui.selectable_value(&mut app.result_source, None, "First load case");
                    for (index, name) in &combos {
                        ui.selectable_value(&mut app.result_source, Some(*index), format!("Combination {name}"));
                    }
                });
            });
            if combos.is_empty() {
                ui.label(egui::RichText::new("No combinations are solved yet; they appear here after Run Analysis.").weak());
            }
            ui.separator();
            ui.checkbox(&mut app.show_deformed, "Show the deformed shape");
            ui.checkbox(&mut app.show_tables, "Show the tables panel");
            ui.add(egui::Slider::new(&mut app.load_symbol_scale, 0.3..=3.0).text("load symbol size"));
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
        });

    if close {
        open = false;
    }
    app.results_setup_dialog_open = open;
}

/// Story summaries: joints, frames, peak displacement and drift to the level below.
fn story_rows(app: &OpenStructApp) -> Vec<(String, f64, usize, usize, f64, f64, f64, f64)> {
    let mut rows = Vec::new();
    let mut stories: Vec<(String, f64)> = app.model.stories.iter().map(|story| (story.name.clone(), story.elevation)).collect();
    stories.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    let displacement = |node_id: usize| -> [f64; 6] {
        let Some(results) = app.results.as_ref() else {
            return [0.0; 6];
        };
        results.displacement_of(node_id).unwrap_or([0.0; 6])
    };

    let horizontal = |node_id: usize| -> f64 {
        let [ux, uy, ..] = displacement(node_id);
        (ux * ux + uy * uy).sqrt()
    };

    for (index, (name, elevation)) in stories.iter().enumerate() {
        let joints: Vec<usize> = app
            .model
            .nodes
            .values()
            .filter(|node| (node.z - elevation).abs() < 1.0e-3)
            .map(|node| node.id)
            .collect();
        let joint_count = joints.len();
        let frame_count = app
            .model
            .elements
            .values()
            .filter(|element| {
                let mid = {
                    let (Some(a), Some(b)) = (app.model.nodes.get(&element.node_i), app.model.nodes.get(&element.node_j)) else {
                        return false;
                    };
                    (a.z + b.z) / 2.0
                };
                (mid - elevation).abs() < 1.0e-3
            })
            .count();
        let peak_vertical = joints
            .iter()
            .map(|id| displacement(*id)[2].abs())
            .fold(0.0_f64, f64::max);
        let peak_horizontal = joints.iter().map(|id| horizontal(*id)).fold(0.0_f64, f64::max);
        let drift = if index == 0 {
            0.0
        } else {
            let below = stories[index - 1].1;
            let height = elevation - below;
            if height > 0.0 {
                let below_peak = app
                    .model
                    .nodes
                    .values()
                    .filter(|node| (node.z - below).abs() < 1.0e-3)
                    .map(|node| horizontal(node.id))
                    .fold(0.0_f64, f64::max);
                (peak_horizontal - below_peak).abs() / height
            } else {
                0.0
            }
        };
        rows.push((name.clone(), *elevation, joint_count, frame_count, peak_horizontal, peak_vertical, drift, peak_vertical.max(peak_horizontal)));
    }
    rows
}

fn story_results_window(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.story_results_dialog_open {
        return;
    }
    let mut open = app.story_results_dialog_open;
    let mut close = false;
    let rows = story_rows(app);
    let has_results = app.results.is_some();

    egui::Window::new("Story Results")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(720.0)
        .show(ctx, |ui| {
            ui.label("Per-level counts and peak displacements, taken from the current solved case.");
            if !has_results {
                ui.label(egui::RichText::new("No results yet — run the analysis (F5).").weak());
            }
            ui.separator();
            egui::Grid::new("story_results_grid").striped(true).num_columns(7).show(ui, |ui| {
                ui.strong("Story");
                ui.strong("Level (m)");
                ui.strong("Joints");
                ui.strong("Frames");
                ui.strong("Max horizontal (m)");
                ui.strong("Max vertical (m)");
                ui.strong("Drift / height");
                ui.end_row();
                for (name, elevation, joints, frames, horizontal, vertical, drift, _) in &rows {
                    ui.label(name);
                    ui.label(format!("{elevation:.3}"));
                    ui.label(format!("{joints}"));
                    ui.label(format!("{frames}"));
                    ui.label(format!("{horizontal:.3e}"));
                    ui.label(format!("{vertical:.3e}"));
                    ui.label(format!("{:.4}", drift * 1000.0 / 1000.0));
                    ui.end_row();
                }
            });
            if rows.is_empty() {
                ui.label(egui::RichText::new("The model has no stories.").weak());
            }
            ui.separator();
            ui.label(egui::RichText::new("Drift is the change in the peak horizontal displacement from the level below, divided by the story height.").weak());
            ui.horizontal(|ui| {
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
        });

    if close {
        open = false;
    }
    app.story_results_dialog_open = open;
}

fn frame_diagrams_window(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.frame_diagrams_dialog_open {
        return;
    }
    let mut open = app.frame_diagrams_dialog_open;
    let mut close = false;
    let mut members: Vec<usize> = app.selected_elements.iter().copied().collect();
    members.sort_unstable();
    if members.is_empty() {
        let mut all: Vec<usize> = app.model.elements.keys().copied().collect();
        all.sort_unstable();
        members = all.into_iter().take(8).collect();
    }
    let component = app.diagram_component;

    egui::Window::new("Frame Diagrams")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(560.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Component:");
                egui::ComboBox::from_id_salt("diagram_component")
                    .selected_text(component_label(component))
                    .show_ui(ui, |ui| {
                        for index in 0..4 {
                            ui.selectable_value(&mut app.diagram_component, index, component_label(index));
                        }
                    });
            });
            ui.label(egui::RichText::new("Ordinates are the envelope end values; without member loads the diagram is straight between them.").weak());
            ui.separator();
            if app.member_envelopes.is_empty() {
                ui.label(egui::RichText::new("No envelopes yet — run the analysis with combinations.").weak());
            }
            egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                for element_id in &members {
                    let Some((i_value, j_value)) = diagram_values(app, *element_id, component) else {
                        continue;
                    };
                    ui.label(format!(
                        "Frame {element_id}   i = {i_value:.3e}   j = {j_value:.3e}"
                    ));
                    let (response, painter) = ui.allocate_painter(egui::vec2(ui.available_width(), 70.0), egui::Sense::hover());
                    let rect = response.rect;
                    painter.rect_filled(rect, 2.0, ui.visuals().faint_bg_color);
                    let span = i_value.abs().max(j_value.abs()).max(1.0e-12);
                    let mid_y = rect.center().y;
                    let half = rect.height() * 0.4;
                    let start = egui::pos2(rect.left() + 6.0, mid_y - (i_value / span) as f32 * half);
                    let end = egui::pos2(rect.right() - 6.0, mid_y - (j_value / span) as f32 * half);
                    painter.line_segment(
                        [egui::pos2(rect.left() + 6.0, mid_y), egui::pos2(rect.right() - 6.0, mid_y)],
                        egui::Stroke::new(1.0, ui.visuals().weak_text_color()),
                    );
                    painter.line_segment([start, end], egui::Stroke::new(2.0, ui.visuals().selection.bg_fill));
                    ui.add_space(4.0);
                }
            });
            ui.horizontal(|ui| {
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
        });

    if close {
        open = false;
    }
    app.frame_diagrams_dialog_open = open;
}

fn area_contours_window(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.area_contours_dialog_open {
        return;
    }
    let mut open = app.area_contours_dialog_open;
    let mut close = false;
    let mut rows: Vec<(usize, String, f64, f64)> = Vec::new();
    let mut peak_pressure = 0.0_f64;
    for area in app.model.areas.values() {
        let pressure: f64 = app
            .model
            .area_loads
            .iter()
            .filter(|load| load.area_id == area.id)
            .map(|load| load.pressure.abs())
            .sum();
        peak_pressure = peak_pressure.max(pressure);
        rows.push((area.id, area.kind.label().to_string(), area.thickness, pressure));
    }
    rows.sort_by(|a, b| a.0.cmp(&b.0));

    egui::Window::new("Area Contours")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(520.0)
        .show(ctx, |ui| {
            ui.label("Assigned pressure per area, shaded from light to dark across the model.");
            ui.separator();
            if rows.is_empty() {
                ui.label(egui::RichText::new("The model has no areas.").weak());
            } else {
                egui::Grid::new("area_contour_grid").striped(true).num_columns(4).show(ui, |ui| {
                    ui.strong("");
                    ui.strong("Area");
                    ui.strong("Thickness (m)");
                    ui.strong("Pressure (Pa)");
                    ui.end_row();
                    for (id, kind, thickness, pressure) in &rows {
                        let share = if peak_pressure > 0.0 { (pressure / peak_pressure).clamp(0.0, 1.0) as f32 } else { 0.0 };
                        let colour = egui::Color32::from_rgb(
                            (70.0 + 150.0 * share) as u8,
                            (130.0 + 60.0 * (1.0 - share)) as u8,
                            (200.0 * (1.0 - share) + 40.0) as u8,
                        );
                        let (response, painter) = ui.allocate_painter(egui::vec2(18.0, 12.0), egui::Sense::hover());
                        painter.rect_filled(response.rect, 2.0, colour);
                        ui.label(format!("{id} · {kind}"));
                        ui.label(format!("{thickness:.3}"));
                        ui.label(format!("{pressure:.0}"));
                        ui.end_row();
                    }
                });
            }
            ui.separator();
            ui.label(egui::RichText::new("The linear run solves frames only, so these are the assigned pressures rather than a shell stress contour. Shell elements are a Phase.md item.").weak());
            ui.horizontal(|ui| {
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
        });

    if close {
        open = false;
    }
    app.area_contours_dialog_open = open;
}

// -- design ------------------------------------------------------------------

struct SteelRow {
    id: usize,
    section: String,
    axial: f64,
    strong: f64,
    util: f64,
    status: DesignStatus,
}

/// Story whose level the frame reaches (its higher joint), which carries the
/// splice settings used by the steel checks.
fn member_story<'a>(app: &'a OpenStructApp, element: &Element) -> Option<&'a openstruct::model::Story> {
    let a = app.model.nodes.get(&element.node_i)?;
    let b = app.model.nodes.get(&element.node_j)?;
    let z = a.z.max(b.z);
    app.model.story_index_at(z, 1e-6).and_then(|index| app.model.stories.get(index))
}

fn steel_rows(app: &OpenStructApp, code: DesignCode) -> Vec<SteelRow> {
    let mut rows = Vec::new();
    let mut ids: Vec<usize> = app.model.elements.keys().copied().collect();
    ids.sort_unstable();
    for id in ids {
        let Some(element) = app.model.elements.get(&id) else { continue };
        let (Some(section), Some(material)) = (app.model.sections.get(&element.section_id), app.model.materials.get(&element.material_id)) else {
            continue;
        };
        let forces = member_forces(app, id).unwrap_or((0.0, 0.0, 0.0, 0.0));
        let length = member_length(app, element).max(0.1);
        let story = member_story(app, element);
        let result = openstruct::design::steel::design_member_with_code(
            &format!("F{id}"),
            section,
            material,
            forces.0,
            forces.2,
            forces.1,
            forces.3,
            length,
            code,
            story.map(|story| story.splice_story).unwrap_or(false),
            story.map(|story| story.splice_height).unwrap_or(0.0),
            story.map(|story| story.elevation).unwrap_or(0.0),
        );
        rows.push(SteelRow {
            id,
            section: section.name.clone(),
            axial: forces.0,
            strong: forces.1,
            util: result.util_ratio,
            status: result.status,
        });
    }
    rows
}

fn steel_design_window(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.steel_design_dialog_open {
        return;
    }
    let mut open = app.steel_design_dialog_open;
    let mut close = false;
    let code = app.design_code;
    let rows = steel_rows(app, code);
    let failing = rows.iter().filter(|row| row.status == DesignStatus::Fail).count();
    let critical = rows.iter().map(|row| row.util).fold(0.0_f64, f64::max);

    egui::Window::new("Steel Design")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(620.0)
        .show(ctx, |ui| {
            if rows.is_empty() {
                ui.label(egui::RichText::new("No members with a section and material yet.").weak());
            } else {
                ui.horizontal(|ui| {
                    ui.label("Design code:");
                    egui::ComboBox::from_id_salt("steel_design_code")
                        .selected_text(code.label())
                        .show_ui(ui, |ui| {
                            for option in DesignCode::ALL {
                                ui.selectable_value(&mut app.design_code, option, option.label());
                            }
                        });
                });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(format!("{} member(s)", rows.len()));
                    ui.separator();
                    ui.label(format!("max utilisation {critical:.3}"));
                    ui.separator();
                    ui.label(egui::RichText::new(format!("{failing} failing")).color(status_colour(failing == 0)));
                });
                ui.separator();
                egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
                    egui::Grid::new("steel_design_grid").striped(true).num_columns(6).show(ui, |ui| {
                        ui.strong("Frame");
                        ui.strong("Section");
                        ui.strong("P (N)");
                        ui.strong("M3 (N·m)");
                        ui.strong("Utilisation");
                        ui.strong("Status");
                        ui.end_row();
                        for row in &rows {
                            ui.label(format!("{}", row.id));
                            ui.label(&row.section);
                            ui.label(format!("{:.0}", row.axial));
                            ui.label(format!("{:.0}", row.strong));
                            ui.label(format!("{:.3}", row.util));
                            let pass = row.status != DesignStatus::Fail;
                            ui.label(egui::RichText::new(if pass { "PASS" } else { "FAIL" }).color(status_colour(pass)));
                            ui.end_row();
                        }
                    });
                });
            }
            ui.separator();
            ui.label(egui::RichText::new(format!("{} interaction with the current envelopes; unbraced length is the member length.", code.label())).weak());
            ui.horizontal(|ui| {
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
        });

    if close {
        open = false;
    }
    app.steel_design_dialog_open = open;
}

fn concrete_design_window(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.concrete_design_dialog_open {
        return;
    }
    let mut open = app.concrete_design_dialog_open;
    let mut close = false;
    let code = app.concrete_design_code;
    let mut rows: Vec<(usize, String, f64, f64, f64, bool)> = Vec::new();
    let mut ids: Vec<usize> = app.model.elements.keys().copied().collect();
    ids.sort_unstable();
    for id in ids {
        let Some(element) = app.model.elements.get(&id) else { continue };
        let (Some(section), Some(material)) = (app.model.sections.get(&element.section_id), app.model.materials.get(&element.material_id)) else {
            continue;
        };
        if material.material_type != MaterialType::Concrete {
            continue;
        }
        let forces = member_forces(app, id).unwrap_or((0.0, 0.0, 0.0, 0.0));
        let result = openstruct::design::concrete::design_concrete_member_with_code(
            &format!("F{id}"),
            section,
            material,
            forces.0,
            forces.2,
            forces.1,
            forces.3,
            app.concrete_strength,
            code,
        );
        rows.push((id, section.name.clone(), forces.0, forces.1, result.util_ratio, result.status != openstruct::design::concrete::ConcreteDesignStatus::Fail));
    }
    let failing = rows.iter().filter(|row| !row.5).count();
    let strength = app.concrete_strength;

    egui::Window::new("Concrete Design")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(620.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("f'c:");
                egui::ComboBox::from_id_salt("concrete_strength")
                    .selected_text(format!("{strength:?}"))
                    .show_ui(ui, |ui| {
                        for option in [
                            ConcreteStrength::C2500,
                            ConcreteStrength::C3000,
                            ConcreteStrength::C3500,
                            ConcreteStrength::C4000,
                            ConcreteStrength::C5000,
                            ConcreteStrength::C6000,
                        ] {
                            ui.selectable_value(&mut app.concrete_strength, option, format!("{option:?}"));
                        }
                    });
                ui.separator();
                ui.label("Code:");
                egui::ComboBox::from_id_salt("concrete_design_code")
                    .selected_text(code.label())
                    .show_ui(ui, |ui| {
                        for option in DesignCode::ALL {
                            ui.selectable_value(&mut app.concrete_design_code, option, option.label());
                        }
                    });
                ui.separator();
                ui.label(egui::RichText::new(format!("{failing} failing")).color(status_colour(failing == 0)));
            });
            ui.separator();
            if rows.is_empty() {
                ui.label(egui::RichText::new("No members with a section and material yet.").weak());
            } else {
                egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
                    egui::Grid::new("concrete_design_grid").striped(true).num_columns(5).show(ui, |ui| {
                        ui.strong("Frame");
                        ui.strong("Section");
                        ui.strong("P (N)");
                        ui.strong("Utilisation");
                        ui.strong("Status");
                        ui.end_row();
                        for (id, section, axial, _, util, pass) in &rows {
                            ui.label(format!("{id}"));
                            ui.label(section);
                            ui.label(format!("{axial:.0}"));
                            ui.label(format!("{util:.3}"));
                            ui.label(egui::RichText::new(if *pass { "PASS" } else { "FAIL" }).color(status_colour(*pass)));
                            ui.end_row();
                        }
                    });
                });
            }
            ui.separator();
            ui.label(egui::RichText::new(format!("{} interaction for rectangular members with the current envelopes.", code.label())).weak());
            ui.horizontal(|ui| {
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
        });

    if close {
        open = false;
    }
    app.concrete_design_dialog_open = open;
}

fn composite_design_window(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.composite_design_dialog_open {
        return;
    }
    let mut open = app.composite_design_dialog_open;
    let mut close = false;
    let mut rows: Vec<(usize, String, f64, bool)> = Vec::new();
    let concrete = app.model.materials.values().find(|material| material.fy <= 0.0).cloned();
    let mut ids: Vec<usize> = app.model.elements.keys().copied().collect();
    ids.sort_unstable();
    if let Some(concrete) = &concrete {
        for id in ids {
            let Some(element) = app.model.elements.get(&id) else { continue };
            let Some(section) = app.model.sections.get(&element.section_id) else { continue };
            let forces = member_forces(app, id).unwrap_or((0.0, 0.0, 0.0, 0.0));
            let result = openstruct::design::composite::design_composite_beam(
                &format!("F{id}"),
                section,
                concrete,
                app.composite_slab_width,
                app.composite_slab_thickness,
                0.15,
                forces.3,
            );
            rows.push((
                id,
                section.name.clone(),
                result.util_ratio,
                result.status != openstruct::design::composite::CompositeDesignStatus::Fail,
            ));
        }
    }
    let failing = rows.iter().filter(|row| !row.3).count();

    egui::Window::new("Composite Beam Design")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(560.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Slab width (m):");
                ui.add(egui::DragValue::new(&mut app.composite_slab_width).speed(0.1).range(0.3..=20.0));
                ui.label("Slab thickness (m):");
                ui.add(egui::DragValue::new(&mut app.composite_slab_thickness).speed(0.01).range(0.05..=1.0));
            });
            ui.separator();
            if concrete.is_none() {
                ui.label(egui::RichText::new("Add a concrete material to design composite beams.").weak());
            } else if rows.is_empty() {
                ui.label(egui::RichText::new("No members with a section yet.").weak());
            } else {
                ui.label(egui::RichText::new(format!("{failing} failing of {} member(s).", rows.len())).color(status_colour(failing == 0)));
                ui.separator();
                egui::Grid::new("composite_design_grid").striped(true).num_columns(4).show(ui, |ui| {
                    ui.strong("Frame");
                    ui.strong("Section");
                    ui.strong("Utilisation");
                    ui.strong("Status");
                    ui.end_row();
                    for (id, section, util, pass) in &rows {
                        ui.label(format!("{id}"));
                        ui.label(section);
                        ui.label(format!("{util:.3}"));
                        ui.label(egui::RichText::new(if *pass { "PASS" } else { "FAIL" }).color(status_colour(*pass)));
                        ui.end_row();
                    }
                });
            }
            ui.separator();
            ui.label(egui::RichText::new("AISC 360 composite beam with headed studs at 150 mm and the declared slab.").weak());
            ui.horizontal(|ui| {
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
        });

    if close {
        open = false;
    }
    app.composite_design_dialog_open = open;
}

fn shear_wall_design_window(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.shear_wall_design_dialog_open {
        return;
    }
    let mut open = app.shear_wall_design_dialog_open;
    let mut close = false;
    let mut rows: Vec<(usize, f64, f64, f64, bool)> = Vec::new();
    let mut areas: Vec<usize> = app.model.areas.keys().copied().collect();
    areas.sort_unstable();
    for id in areas {
        let Some(area) = app.model.areas.get(&id) else { continue };
        if !matches!(area.kind, openstruct::areas::AreaKind::Wall) {
            continue;
        }
        let Some(material) = app.model.materials.get(&area.material_id) else { continue };
        let width = area.node_ids.len() as f64;
        let (Some(first), Some(last)) = (area.node_ids.first(), area.node_ids.last()) else {
            continue;
        };
        let (Some(a), Some(b)) = (app.model.nodes.get(first), app.model.nodes.get(last)) else {
            continue;
        };
        let height = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2) + (b.z - a.z).powi(2)).sqrt().max(0.1);
        let section = Section::rectangular(&format!("WALL {id}"), area.thickness.max(0.05), width.max(1.0));
        let result = openstruct::design::shear_wall::design_shear_wall(
            &format!("A{id}"),
            &section,
            material,
            width.max(1.0),
            height,
            0.0,
            0.0,
            0.0,
            0.0,
        );
        rows.push((
            id,
            height,
            result.util_ratio,
            width,
            result.status != openstruct::design::shear_wall::ShearWallDesignStatus::Fail,
        ));
    }

    egui::Window::new("Shear Wall Design")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(540.0)
        .show(ctx, |ui| {
            if rows.is_empty() {
                ui.label(egui::RichText::new("No wall areas in the model. Draw a wall, then design it here.").weak());
            } else {
                egui::Grid::new("shear_wall_grid").striped(true).num_columns(5).show(ui, |ui| {
                    ui.strong("Area");
                    ui.strong("Width (m)");
                    ui.strong("Height (m)");
                    ui.strong("Utilisation");
                    ui.strong("Status");
                    ui.end_row();
                    for (id, height, util, width, pass) in &rows {
                        ui.label(format!("{id}"));
                        ui.label(format!("{width:.2}"));
                        ui.label(format!("{height:.2}"));
                        ui.label(format!("{util:.3}"));
                        ui.label(egui::RichText::new(if *pass { "PASS" } else { "FAIL" }).color(status_colour(*pass)));
                        ui.end_row();
                    }
                });
            }
            ui.separator();
            ui.label(egui::RichText::new("Capacity check per ACI 318 with the wall's own geometry. Wall forces need the shell solver, so they are zero here and the check reports the section capacity.").weak());
            ui.horizontal(|ui| {
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
        });

    if close {
        open = false;
    }
    app.shear_wall_design_dialog_open = open;
}

fn detailing_window(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.detailing_dialog_open {
        return;
    }
    let mut open = app.detailing_dialog_open;
    let mut close = false;
    let mut rows: Vec<(usize, String, f64, f64, f64)> = Vec::new();
    let mut ids: Vec<usize> = app.model.elements.keys().copied().collect();
    ids.sort_unstable();
    for id in ids {
        let Some(element) = app.model.elements.get(&id) else { continue };
        let (Some(section), Some(material)) = (app.model.sections.get(&element.section_id), app.model.materials.get(&element.material_id)) else {
            continue;
        };
        let forces = member_forces(app, id).unwrap_or((0.0, 0.0, 0.0, 0.0));
        // Required steel from the governing moment, then the bar count for a
        // 20 mm bar; both are first-pass detailing values, not a substitute for
        // the design check.
        let lever = (section.depth * 0.9).max(0.05);
        let fy = if material.fy > 0.0 { material.fy } else { 420.0e6 };
        let required = (forces.1 / (0.9 * fy * lever)).max(0.0) * 1.0e6;
        let area_bar = std::f64::consts::PI * (10.0_f64).powi(2);
        let bars = (required / area_bar).ceil().max(2.0);
        let spacing = (1000.0 / bars.max(1.0)).clamp(75.0, 300.0);
        rows.push((id, section.name.clone(), required, bars, spacing));
    }

    egui::Window::new("Detailing")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(620.0)
        .show(ctx, |ui| {
            ui.label("First-pass bar schedule from the governing moments and the member sections.");
            ui.separator();
            if rows.is_empty() {
                ui.label(egui::RichText::new("No members to detail yet.").weak());
            } else {
                egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
                    egui::Grid::new("detailing_grid").striped(true).num_columns(5).show(ui, |ui| {
                        ui.strong("Frame");
                        ui.strong("Section");
                        ui.strong("As required (mm²)");
                        ui.strong("20 mm bars");
                        ui.strong("Stirrups (mm)");
                        ui.end_row();
                        for (id, section, required, bars, spacing) in &rows {
                            ui.label(format!("{id}"));
                            ui.label(section);
                            ui.label(format!("{required:.0}"));
                            ui.label(format!("{bars:.0}"));
                            ui.label(format!("{spacing:.0}"));
                            ui.end_row();
                        }
                    });
                });
            }
            ui.separator();
            ui.label(egui::RichText::new("As = M / (0.9 fy d) with d = 0.9 h; stirrups hold 1000 mm² of — the spacing shown is the pitch for a 20 mm bar. Run Concrete Design for the governing check.").weak());
            ui.horizontal(|ui| {
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
        });

    if close {
        open = false;
    }
    app.detailing_dialog_open = open;
}

// -- reports and help ---------------------------------------------------------

fn reports_window(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.reports_dialog_open {
        return;
    }
    let mut open = app.reports_dialog_open;
    let mut close = false;
    let mut build = false;
    let mut save = false;

    egui::Window::new("Reports")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(620.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Title:");
                ui.add(egui::TextEdit::singleline(&mut app.report_title).desired_width(280.0));
            });
            ui.checkbox(&mut app.report_include_design, "Include the design summaries");
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Build report").clicked() {
                    build = true;
                }
                if ui.add_enabled(app.report_html.is_some(), egui::Button::new("Save HTML…")).clicked() {
                    save = true;
                }
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
            ui.separator();
            match app.report_html.as_ref() {
                None => {
                    ui.label(egui::RichText::new("Build the report to preview it here.").weak());
                }
                Some(html) => {
                    ui.label(format!("{} characters of HTML.", html.chars().count()));
                    let mut preview = html.clone();
                    egui::ScrollArea::vertical().max_height(340.0).show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut preview)
                                .desired_width(f32::INFINITY)
                                .font(egui::TextStyle::Monospace),
                        );
                    });
                }
            }
        });

    if build {
        let mut analysis = Analysis::new(app.model.clone());
        analysis.load_cases = app.model.load_cases.clone();
        if analysis.load_cases.is_empty() {
            analysis.load_cases = vec![openstruct::analysis::default_case()];
        }
        let config = ReportConfig {
            title: app.report_title.clone(),
            project_name: app.model.name.clone(),
            date: None,
            scale: None,
            include_analysis: true,
            include_steel: app.report_include_design,
            include_concrete: app.report_include_design,
            include_composite: false,
            include_shear_wall: false,
            include_rebar: false,
            css_class: String::new(),
        };
        let data = reporting::generate_report(&analysis, config, None, None, None, None, None);
        let html = HtmlReport::new(data).render();
        app.status_text = format!("Report built: {} member(s), {} joint(s).", app.model.elements.len(), app.model.nodes.len());
        app.report_html = Some(html);
    }
    if save {
        if let Some(html) = app.report_html.clone() {
            if let Some(mut path) = rfd::FileDialog::new().add_filter("HTML", &["html"]).save_file() {
                if path.extension().is_none() {
                    path.set_extension("html");
                }
                match std::fs::write(&path, html) {
                    Ok(()) => app.status_text = format!("Report saved to {}.", path.display()),
                    Err(err) => app.status_text = format!("Report save failed: {err}"),
                }
            }
        }
    }
    if close {
        open = false;
    }
    app.reports_dialog_open = open;
}

fn documentation_window(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.documentation_dialog_open {
        return;
    }
    let mut open = app.documentation_dialog_open;
    egui::Window::new("Documentation")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(620.0)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().max_height(460.0).show(ui, |ui| {
                ui.heading("Workflow");
                ui.label("1. Define the grid and the stories, then draw joints, frames and walls on the work plane.");
                ui.label("2. Define materials, frame sections and area sections; assign releases, springs and supports.");
                ui.label("3. Define load patterns, cases and combinations, then assign loads to the selection.");
                ui.label("4. Run Check Model, fix anything it reports, then Run Analysis (F5).");
                ui.label("5. Read displacements, reactions and member forces in the tables, then review the design dialogs.");
                ui.separator();
                ui.heading("Units");
                ui.label("The model is stored in SI: metres, newtons, pascals, kilograms. Point loads typed on the canvas are entered in kN and converted when the analysis runs.");
                ui.separator();
                ui.heading("Files");
                ui.label("Models are saved as .ostrut JSON, which keeps the model, grid, groups and reference planes. File → Export E2K saves the readable E2K text archive.");
                ui.separator();
                ui.heading("Analysis");
                ui.label("The solver assembles the 3D frame stiffness matrix, applies supports, springs and loads, solves it directly and derives reactions and member end forces. Combinations follow their type: linear add, envelope, SRSS, ABS and CQC.");
                ui.label("Area objects are carried in the model and reported, but the linear run solves frames only; shell elements and P-Delta are the remaining Phase.md items.");
                ui.separator();
                ui.heading("Shortcuts");
                ui.label("Ctrl+K command palette · F5 analyse · F6 deformed shape · F7 grid · 1/2/3 select, draw joints, draw frames.");
                ui.separator();
                ui.heading("Clean room");
                ui.label("OpenStruct is an independent implementation. The commercial package it is compared against is used only as a behavioural reference: no binaries, assets or file formats are copied.");
            });
        });
    app.documentation_dialog_open = open;
}

// -- verification -------------------------------------------------------------

/// One analytic comparison: name, computed value, theory, unit, pass flag.
struct VerificationCase {
    name: String,
    computed: f64,
    theory: f64,
    passed: bool,
}

fn simply_supported_deflection() -> Option<VerificationCase> {
    let mut model = openstruct::model::Model::new("verification: simply supported beam");
    let material = model.add_material(Material::steel());
    let section_id = model.add_section(Section::rectangular("RECT300X500", 0.3, 0.5));
    let node_i = model.add_node(0.0, 0.0, 0.0);
    let node_mid = model.add_node(3.0, 0.0, 0.0);
    let node_j = model.add_node(6.0, 0.0, 0.0);
    let left = model.add_element(Element::frame(1, node_i, node_mid, material, section_id));
    let right = model.add_element(Element::frame(2, node_mid, node_j, material, section_id));
    model.nodes.get_mut(&node_i)?.restrained = [true, true, true, true, false, false];
    model.nodes.get_mut(&node_j)?.restrained = [false, true, true, false, false, false];
    const W: f64 = 10_000.0;
    const L: f64 = 6.0;
    for element in [left, right] {
        model.frame_loads.push(DistributedLoad {
            element_id: element,
            direction: LoadDirection::Global,
            start: [0.0, 0.0, -W],
            end: [0.0, 0.0, -W],
            projected: false,
            case_id: None,
        });
    }
    let mut analysis = Analysis::new(model);
    analysis.load_cases = vec![LoadCase {
        id: 1,
        name: "CASE 1".into(),
        pattern: LoadType::Dead,
        self_weight_multiplier: 0.0,
        nodal_loads: Vec::new(),
        distributed_loads: Vec::new(),
        p_delta: false,
    }];
    let results: AnalysisResults = analysis.run_static();
    let deflection = results.displacement_of(node_mid)?[2].abs();
    let section = analysis.model.sections.get(&section_id)?;
    let e = analysis.model.materials.values().next()?.e;
    let theory = 5.0 * W * L.powi(4) / (384.0 * e * section.iy);
    let error = ((deflection - theory) / theory).abs();
    Some(VerificationCase {
        name: "Simply supported beam, UDL: midspan deflection".to_string(),
        computed: deflection,
        theory,
                passed: error < 0.02,
    })
}

fn cantilever_deflection() -> Option<VerificationCase> {
    let mut model = openstruct::model::Model::new("verification: cantilever");
    let material = model.add_material(Material::steel());
    let section_id = model.add_section(Section::rectangular("RECT300X500", 0.3, 0.5));
    let fixed = model.add_node(0.0, 0.0, 0.0);
    let tip = model.add_node(3.0, 0.0, 0.0);
    model.add_element(Element::frame(1, fixed, tip, material, section_id));
    model.nodes.get_mut(&fixed)?.restrained = [true; 6];
    const P: f64 = 5_000.0;
    const L: f64 = 3.0;
    let case = LoadCase {
        id: 1,
        name: "CASE 1".into(),
        pattern: LoadType::Dead,
        self_weight_multiplier: 0.0,
        nodal_loads: vec![NodalLoad {
            node_id: tip,
            fx: 0.0,
            fy: 0.0,
            fz: -P,
            mx: 0.0,
            my: 0.0,
            mz: 0.0,
        }],
        distributed_loads: Vec::new(),
        p_delta: false,
    };
    let mut analysis = Analysis::new(model);
    analysis.load_cases = vec![case];
    let results = analysis.run_static();
    let deflection = results.displacement_of(tip)?[2].abs();
    let section = analysis.model.sections.get(&section_id)?;
    let e = analysis.model.materials.values().next()?.e;
    let theory = P * L.powi(3) / (3.0 * e * section.iy);
    let error = ((deflection - theory) / theory).abs();
    Some(VerificationCase {
        name: "Cantilever, tip load: tip deflection".to_string(),
        computed: deflection,
        theory,
                passed: error < 0.02,
    })
}

fn axial_bar_extension() -> Option<VerificationCase> {
    let mut model = openstruct::model::Model::new("verification: axial bar");
    let material = model.add_material(Material::steel());
    let section_id = model.add_section(Section::rectangular("BAR", 0.2, 0.2));
    let fixed = model.add_node(0.0, 0.0, 0.0);
    let tip = model.add_node(4.0, 0.0, 0.0);
    model.add_element(Element::frame(1, fixed, tip, material, section_id));
    model.nodes.get_mut(&fixed)?.restrained = [true; 6];
    const P: f64 = 100_000.0;
    const L: f64 = 4.0;
    let mut analysis = Analysis::new(model);
    analysis.load_cases = vec![LoadCase {
        id: 1,
        name: "CASE 1".into(),
        pattern: LoadType::Dead,
        self_weight_multiplier: 0.0,
        nodal_loads: vec![NodalLoad {
            node_id: tip,
            fx: P,
            fy: 0.0,
            fz: 0.0,
            mx: 0.0,
            my: 0.0,
            mz: 0.0,
        }],
        distributed_loads: Vec::new(),
        p_delta: false,
    }];
    let results = analysis.run_static();
    let extension = results.displacement_of(tip)?[0];
    let section = analysis.model.sections.get(&section_id)?;
    let e = analysis.model.materials.values().next()?.e;
    let theory = P * L / (e * section.area);
    let error = if theory.abs() > 0.0 { ((extension - theory) / theory).abs() } else { 1.0 };
    Some(VerificationCase {
        name: "Axial bar, end load: extension".to_string(),
        computed: extension,
        theory,
                passed: error < 0.005,
    })
}

fn equilibrium_check() -> Option<VerificationCase> {
    let mut model = openstruct::model::Model::new("verification: equilibrium");
    let material = model.add_material(Material::steel());
    let section_id = model.add_section(Section::rectangular("RECT300X500", 0.3, 0.5));
    let a = model.add_node(0.0, 0.0, 0.0);
    let b = model.add_node(4.0, 0.0, 0.0);
    model.add_element(Element::frame(1, a, b, material, section_id));
    model.nodes.get_mut(&a)?.restrained = [true; 6];
    const P: f64 = 25_000.0;
    let mut analysis = Analysis::new(model);
    analysis.load_cases = vec![LoadCase {
        id: 1,
        name: "CASE 1".into(),
        pattern: LoadType::Dead,
        self_weight_multiplier: 0.0,
        nodal_loads: vec![NodalLoad {
            node_id: b,
            fx: 0.0,
            fy: 0.0,
            fz: -P,
            mx: 0.0,
            my: 0.0,
            mz: 0.0,
        }],
        distributed_loads: Vec::new(),
        p_delta: false,
    }];
    let results = analysis.run_static();
    let sums = analysis.reaction_sums(&results);
    let error = ((sums[2] - P) / P).abs();
    Some(VerificationCase {
        name: "Support reaction balances the applied load".to_string(),
        computed: sums[2],
        theory: P,
                passed: error < 1.0e-6,
    })
}

fn run_verification_cases() -> Vec<VerificationCase> {
    [
        simply_supported_deflection(),
        cantilever_deflection(),
        axial_bar_extension(),
        equilibrium_check(),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn verification_window(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.verification_dialog_open {
        return;
    }
    let mut open = app.verification_dialog_open;
    let mut run = false;
    let mut close = false;

    egui::Window::new("Verification Cases")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(640.0)
        .show(ctx, |ui| {
            ui.label("Analytic cases with a known closed-form answer, solved by the same engine the model uses.");
            ui.separator();
            if app.verification_rows.is_empty() {
                ui.label(egui::RichText::new("Run the cases to compare the solver against beam theory.").weak());
            } else {
                egui::Grid::new("verification_grid").striped(true).num_columns(4).show(ui, |ui| {
                    ui.strong("Case");
                    ui.strong("Solver");
                    ui.strong("Theory");
                    ui.strong("Result");
                    ui.end_row();
                    for (name, computed, theory, passed) in &app.verification_rows {
                        ui.label(name);
                        ui.label(format!("{computed:.6e}"));
                        ui.label(format!("{theory:.6e}"));
                        ui.label(egui::RichText::new(if *passed { "PASS" } else { "FAIL" }).color(status_colour(*passed)));
                        ui.end_row();
                    }
                });
            }
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Run cases").clicked() {
                    run = true;
                }
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
        });

    if run {
        let cases = run_verification_cases();
        let passed = cases.iter().filter(|case| case.passed).count();
        app.status_text = format!("Verification: {passed} of {} case(s) passed.", cases.len());
        app.verification_rows = cases.into_iter().map(|case| (case.name, case.computed, case.theory, case.passed)).collect();
    }
    if close {
        open = false;
    }
    app.verification_dialog_open = open;
}
