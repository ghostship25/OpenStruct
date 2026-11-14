//! Tables dock: joint coordinates, displacements, reactions and frame forces.
//!
//! Values are stored in SI and converted for display, so switching the unit
//! system never touches the model.

use crate::app::{BottomTab, OpenStructApp};
use eframe::egui;
use openstruct::loads::LoadDirection;
use std::collections::HashMap;

/// Height of the table body inside the dock, leaving room for the dock's own
/// toolbar. Fixed so the dock never changes height with its contents.
const DOCK_BODY_HEIGHT: f32 = 186.0;

pub fn tables(app: &mut OpenStructApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        for tab in BottomTab::ALL {
            if ui.selectable_label(app.bottom_tab == tab, tab.title()).clicked() {
                if app.bottom_tab != tab {
                    // Sorting and column filters belong to the table that set them.
                    app.table_sort = None;
                    app.table_filters.clear();
                }
                app.bottom_tab = tab;
            }
        }
        if !app.combo_results.is_empty() {
            ui.separator();
            ui.label("Results:");
            let selected = app
                .result_source
                .and_then(|index| app.combo_results.get(index))
                .map(|combo| combo.name.clone())
                .unwrap_or_else(|| "Static case".to_string());
            egui::ComboBox::from_id_salt("result_source").selected_text(selected).show_ui(ui, |ui| {
                ui.selectable_value(&mut app.result_source, None, "Static case");
                let names: Vec<String> = app.combo_results.iter().map(|combo| combo.name.clone()).collect();
                for (index, name) in names.into_iter().enumerate() {
                    ui.selectable_value(&mut app.result_source, Some(index), name);
                }
            });
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("✕").on_hover_text("Hide the tables panel").clicked() {
                app.show_tables = false;
            }
            if ui.small_button("⭳ CSV").on_hover_text("Export the visible rows of this table to CSV").clicked() {
                export_csv(app);
            }
            if ui.small_button("⭱ CSV").on_hover_text("Import joint coordinates or frame sections from a CSV file").clicked() {
                import_csv(app);
            }
            ui.add(egui::TextEdit::singleline(&mut app.table_filter).hint_text("Filter rows").desired_width(160.0));
        });
    });
    ui.separator();

    // A bounded body: the dock used to size itself to the visible rows, so drawing a
    // joint made it taller and lifted the canvas under the pointer. The height here is
    // constant, so the canvas keeps its place whatever the table holds.
    egui::ScrollArea::both()
        .id_salt("tables_dock_body")
        .auto_shrink([false, false])
        .max_height(DOCK_BODY_HEIGHT)
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
        .show(ui, |ui| crate::tables_view::table_body(app, ui));
}

pub(crate) fn active_results(app: &OpenStructApp) -> Option<&openstruct::analysis::AnalysisResults> {
    match app.result_source {
        Some(index) => app.combo_results.get(index).map(|combo| &combo.results),
        None => app.results.as_ref(),
    }
}

/// Dimensions that rebuild a section: width, depth, web/wall and flange.
fn section_dimensions(section: &openstruct::sections::Section) -> (f64, f64, f64, f64) {
    use openstruct::sections::SectionShape;
    match section.shape {
        Some(SectionShape::Rectangular { width, depth }) => (width, depth, 0.0, 0.0),
        Some(SectionShape::Circular { diameter }) => (diameter, diameter, 0.0, 0.0),
        Some(SectionShape::ISection { depth, width, web, flange })
        | Some(SectionShape::Channel { depth, width, web, flange })
        | Some(SectionShape::Tee { depth, width, web, flange }) => (width, depth, web, flange),
        Some(SectionShape::Box { depth, width, wall }) => (width, depth, wall, 0.0),
        Some(SectionShape::Pipe { diameter, wall }) => (diameter, diameter, wall, 0.0),
        Some(SectionShape::Angle { depth, width, thickness }) => (width, depth, thickness, 0.0),
        None => (section.width, section.depth, 0.0, 0.0),
    }
}

/// Shape family of a stored frame section, for the property table.
fn section_shape_label(section: &openstruct::sections::Section) -> &'static str {
    use openstruct::sections::SectionShape;
    if !section.designer.is_empty() {
        return "Custom (drawn)";
    }
    match section.shape {
        Some(SectionShape::Rectangular { .. }) => "Rectangular",
        Some(SectionShape::Circular { .. }) => "Circular",
        Some(SectionShape::ISection { .. }) => "I / wide flange",
        Some(SectionShape::Channel { .. }) => "Channel",
        Some(SectionShape::Tee { .. }) => "Tee",
        Some(SectionShape::Box { .. }) => "Box",
        Some(SectionShape::Pipe { .. }) => "Pipe",
        Some(SectionShape::Angle { .. }) => "Angle",
        None => "Legacy",
    }
}

/// Display label of a load's case id.
fn case_label(app: &OpenStructApp, case_id: Option<usize>) -> String {
    match case_id {
        None => "All cases".to_string(),
        Some(id) => app
            .model
            .load_cases
            .iter()
            .find(|case| case.id == id)
            .map(|case| case.name.clone())
            .unwrap_or_else(|| format!("Case {id} (missing)")),
    }
}

/// Export the visible rows of the active table as CSV.
fn export_csv(app: &mut OpenStructApp) {
    let (headers, all_rows) = csv_table(app);
    let rows = crate::tables_view::visible_rows(app, &headers, all_rows);
    if rows.is_empty() {
        app.status_text = "Nothing to export in this table.".to_string();
        return;
    }
    let default_name = format!("openstruct_{}.csv", app.bottom_tab.title().to_lowercase().replace(' ', "_"));
    let Some(path) = rfd::FileDialog::new()
        .add_filter("CSV", &["csv"])
        .set_file_name(&default_name)
        .save_file()
    else {
        return;
    };
    let text = openstruct::export::rows_to_csv(&headers, &rows);
    match std::fs::write(&path, text) {
        Ok(()) => app.status_text = format!("Exported {} row(s) to {}.", rows.len(), path.display()),
        Err(error) => app.status_text = format!("Export failed: {error}"),
    }
}

/// Import a CSV file back into the model. Joint coordinates and frame sections
/// carry model data, so only those two tables accept an import.
fn import_csv(app: &mut OpenStructApp) {
    let Some(path) = rfd::FileDialog::new().add_filter("CSV", &["csv"]).pick_file() else {
        return;
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            app.status_text = format!("Import failed: {error}");
            return;
        }
    };
    let records = openstruct::export::parse_csv(&text);
    if records.is_empty() {
        app.status_text = "Import failed: the file is empty.".to_string();
        return;
    }
    let (imported, skipped) = match app.bottom_tab {
        BottomTab::Joints => match openstruct::import::parse_joints(&text, app.units) {
            Ok(parsed) if parsed.rows.is_empty() => {
                app.status_text = format!("No joint rows in {} ({} skipped).", path.display(), parsed.skipped);
                return;
            }
            Ok(parsed) => {
                let skipped = parsed.skipped;
                app.push_undo();
                let (imported, loads) = parsed.apply(&mut app.model);
                for (node, load) in loads {
                    app.nodal_loads.insert(node, load);
                }
                (imported, skipped)
            }
            Err(message) => {
                app.status_text = message;
                return;
            }
        },
        BottomTab::Sections => match openstruct::import::parse_sections(&text, app.units) {
            Ok(parsed) if parsed.rows.is_empty() => {
                app.status_text = format!("No section rows in {} ({} skipped).", path.display(), parsed.skipped);
                return;
            }
            Ok(parsed) => {
                let skipped = parsed.skipped;
                app.push_undo();
                (parsed.apply(&mut app.model), skipped)
            }
            Err(message) => {
                app.status_text = message;
                return;
            }
        },
        other => {
            app.status_text = format!("{} row(s) read, but {} has no CSV import; use Joint Coordinates or Frame Sections.", records.len() - 1, other.title());
            return;
        }
    };
    app.dirty = true;
    app.results = None;
    app.status_text = format!("Imported {imported} row(s) from {} ({skipped} skipped).", path.display());
}

/// The active table's visible rows, matching what the grid displays.
pub(crate) fn csv_table(app: &OpenStructApp) -> (Vec<String>, Vec<Vec<String>>) {
    let units = app.units;
    let headers: Vec<String>;
    let mut rows: Vec<Vec<String>> = Vec::new();

    match app.bottom_tab {
        BottomTab::Joints => {
            let mut ids: Vec<usize> = app.model.nodes.keys().copied().collect();
            ids.sort_unstable();
            headers = vec![
                "Joint".into(),
                format!("X ({})", units.len_label()),
                format!("Y ({})", units.len_label()),
                format!("Z ({})", units.len_label()),
                "Restraints".into(),
                "Springs".into(),
                "Load (kN)".into(),
            ];
            const DOF_LABELS: [&str; 6] = ["UX", "UY", "UZ", "RX", "RY", "RZ"];
            for id in ids {
                let Some(node) = app.model.nodes.get(&id) else {
                    continue;
                };
                let key = format!("N{id}");
                let load = app.nodal_loads.get(&id);
                let restraints: String = node.restrained.iter().map(|&r| if r { 'T' } else { '-' }).collect();
                let springs: String = if node.has_springs() {
                    node.springs
                        .iter()
                        .enumerate()
                        .filter(|&(_, &stiffness)| stiffness > 0.0)
                        .map(|(dof, stiffness)| format!("{} {:.3e}", DOF_LABELS[dof], stiffness))
                        .collect::<Vec<_>>()
                        .join(", ")
                } else {
                    "—".to_string()
                };
                rows.push(vec![
                    key,
                    format!("{:.3}", node.x * units.len_factor()),
                    format!("{:.3}", node.y * units.len_factor()),
                    format!("{:.3}", node.z * units.len_factor()),
                    restraints,
                    springs,
                    match load {
                        Some(load) => format!("{:.1}, {:.1}, {:.1}", load[0], load[1], load[2]),
                        None => "—".into(),
                    },
                ]);
            }
        }
        BottomTab::Displacements => {
            headers = vec![
                "Joint".into(),
                format!("UX ({})", units.disp_label()),
                format!("UY ({})", units.disp_label()),
                format!("UZ ({})", units.disp_label()),
                "RX (rad)".into(),
                "RY (rad)".into(),
                "RZ (rad)".into(),
            ];
            if let Some(results) = active_results(app) {
                for (index, &id) in results.node_ids.iter().enumerate() {
                    let base = 6 * index;
                    if base + 5 >= results.displacements.len() {
                        break;
                    }
                    let key = format!("N{id}");
                    let mut row = vec![key];
                    for k in 0..6 {
                        let value = results.displacements[base + k];
                        if k < 3 {
                            row.push(format!("{:.3}", value * units.disp_factor()));
                        } else {
                            row.push(format!("{value:.3e}"));
                        }
                    }
                    rows.push(row);
                }
            }
        }
        BottomTab::Reactions => {
            headers = vec![
                "Joint".into(),
                format!("FX ({})", units.force_label()),
                format!("FY ({})", units.force_label()),
                format!("FZ ({})", units.force_label()),
                format!("MX ({})", units.moment_label()),
                format!("MY ({})", units.moment_label()),
                format!("MZ ({})", units.moment_label()),
            ];
            let mut restrained: Vec<usize> = app.model.nodes.iter().filter(|(_, node)| node.has_support()).map(|(&id, _)| id).collect();
            restrained.sort_unstable();
            if let Some(results) = active_results(app) {
                let index: HashMap<usize, usize> = results.node_ids.iter().enumerate().map(|(i, &id)| (id, i)).collect();
                let force = units.force_factor();
                let moment = units.moment_factor();
                for id in restrained {
                    let Some(&position) = index.get(&id) else {
                        continue;
                    };
                    let key = format!("N{id}");
                    let base = 6 * position;
                    if base + 5 >= results.reactions.len() {
                        continue;
                    }
                    let mut row = vec![key];
                    for k in 0..3 {
                        row.push(format!("{:.2}", results.reactions[base + k] / 1000.0 * force));
                    }
                    for k in 3..6 {
                        row.push(format!("{:.2}", results.reactions[base + k] / 1.0e6 * moment));
                    }
                    rows.push(row);
                }
            }
        }
        BottomTab::Forces => {
            headers = vec![
                "Frame".into(),
                "Type".into(),
                format!("Axial ({})", units.force_label()),
                format!("My,i ({})", units.moment_label()),
                format!("My,j ({})", units.moment_label()),
                format!("Mz,i ({})", units.moment_label()),
                format!("Mz,j ({})", units.moment_label()),
            ];
            if let Some(results) = active_results(app) {
                let force = units.force_factor();
                let moment = units.moment_factor();
                for (index, &eid) in results.element_ids.iter().enumerate() {
                    let base = 12 * index;
                    if base + 11 >= results.member_forces.len() {
                        break;
                    }
                    let key = format!("F{eid}");
                    let element_type = app.model.elements.get(&eid).map(|element| format!("{:?}", element.element_type)).unwrap_or_default();
                    let axial = (results.member_forces[base] + results.member_forces[base + 6]) * 0.5 / 1000.0;
                    rows.push(vec![
                        key,
                        element_type,
                        format!("{:.2}", axial * force),
                        format!("{:.2}", results.member_forces[base + 4] / 1000.0 * moment),
                        format!("{:.2}", results.member_forces[base + 10] / 1000.0 * moment),
                        format!("{:.2}", results.member_forces[base + 5] / 1000.0 * moment),
                        format!("{:.2}", results.member_forces[base + 11] / 1000.0 * moment),
                    ]);
                }
            }
        }
        BottomTab::FrameLoads => {
            headers = vec![
                "Frame".into(),
                "Case".into(),
                "Direction".into(),
                format!("wx ({})", units.dist_label()),
                format!("wy ({})", units.dist_label()),
                format!("wz ({})", units.dist_label()),
                "Projected".into(),
            ];
            let dist = units.dist_factor();
            let mut loads = app.model.frame_loads.clone();
            loads.sort_by_key(|load| load.element_id);
            for load in loads {
                let case = case_label(app, load.case_id);
                let key = format!("F{}", load.element_id);
                rows.push(vec![
                    key,
                    case,
                    match load.direction {
                        LoadDirection::Global => "Global".into(),
                        LoadDirection::Local => "Local".into(),
                    },
                    format!("{:.2}", load.start[0] / 1000.0 * dist),
                    format!("{:.2}", load.start[1] / 1000.0 * dist),
                    format!("{:.2}", load.start[2] / 1000.0 * dist),
                    if load.projected { "Yes".into() } else { "No".into() },
                ]);
            }
        }
        BottomTab::AreaLoads => {
            headers = vec![
                "Area".into(),
                "Case".into(),
                "Direction".into(),
                format!("Pressure ({})", units.pressure_label()),
            ];
            let pressure = units.pressure_factor();
            let mut loads = app.model.area_loads.clone();
            loads.sort_by_key(|load| load.area_id);
            for load in loads {
                let case = case_label(app, load.case_id);
                let key = format!("A{}", load.area_id);
                rows.push(vec![
                    key,
                    case,
                    match load.direction {
                        LoadDirection::Global => "Global".into(),
                        LoadDirection::Local => "Local".into(),
                    },
                    format!("{:.3}", load.pressure / 1000.0 * pressure),
                ]);
            }
        }
        BottomTab::TemperatureLoads => {
            headers = vec![
                "Frame".into(),
                "Case".into(),
                "ΔT (°C)".into(),
                "Initial strain".into(),
                format!("Axial force ({})", units.force_label()),
            ];
            let mut loads = app.model.temperature_loads.clone();
            loads.sort_by_key(|load| load.element_id);
            for load in loads {
                let Some(element) = app.model.elements.get(&load.element_id) else {
                    continue;
                };
                let (Some(material), Some(section)) = (
                    app.model.materials.get(&element.material_id),
                    app.model.sections.get(&element.section_id),
                ) else {
                    continue;
                };
                let axial = material.e * section.area * section.modifiers.area * load.axial_strain(material.alpha);
                rows.push(vec![
                    format!("F{}", load.element_id),
                    case_label(app, load.case_id),
                    format!("{:.2}", load.delta_t),
                    format!("{:.2e}", load.strain),
                    format!("{:.2}", axial / 1000.0 * units.force_factor()),
                ]);
            }
        }
        BottomTab::Sections => {
            let area = units.area_factor();
            let inertia = units.inertia_factor();
            let modulus = units.section_modulus_factor();
            headers = vec![
                "Section".into(),
                "Shape".into(),
                format!("Width ({})", units.len_label()),
                format!("Depth ({})", units.len_label()),
                format!("Web/Wall ({})", units.len_label()),
                format!("Flange ({})", units.len_label()),
                "Drawn".into(),
                format!("A ({})", units.area_label()),
                format!("Iy ({})", units.inertia_label()),
                format!("Iz ({})", units.inertia_label()),
                format!("J ({})", units.inertia_label()),
                format!("Sy ({})", units.section_modulus_label()),
                format!("Sz ({})", units.section_modulus_label()),
            ];
            let mut sections: Vec<(usize, &openstruct::sections::Section)> = app.model.sections.iter().map(|(&id, section)| (id, section)).collect();
            sections.sort_by_key(|entry| entry.0);
            for (id, section) in sections {
                let (sy, sz) = section.section_moduli();
                let (width, depth, web, flange) = section_dimensions(section);
                rows.push(vec![
                    format!("S{id}  {}", section.name),
                    section_shape_label(section).to_string(),
                    format!("{:.4}", units.length_to_display(width)),
                    format!("{:.4}", units.length_to_display(depth)),
                    format!("{:.4}", units.length_to_display(web)),
                    format!("{:.4}", units.length_to_display(flange)),
                    if section.designer.is_empty() {
                        String::new()
                    } else {
                        serde_json::to_string(&section.designer).unwrap_or_default()
                    },
                    format!("{:.4e}", section.area * area),
                    format!("{:.4e}", section.iy * inertia),
                    format!("{:.4e}", section.iz * inertia),
                    format!("{:.4e}", section.j * inertia),
                    format!("{:.4e}", sy * modulus),
                    format!("{:.4e}", sz * modulus),
                ]);
            }
        }
        BottomTab::Combinations => {
            headers = vec![
                "Combination".into(),
                "Type".into(),
                "Equation".into(),
                "Status".into(),
                format!("Max |disp| ({})", units.disp_label()),
                format!("Max reaction ({})", units.force_label()),
            ];
            for combo in &app.combo_results {
                rows.push(vec![
                    combo.name.clone(),
                    combo.combo_type.label().to_string(),
                    combo.equation.clone(),
                    combo.status.clone(),
                    format!("{:.3}", combo.max_displacement * units.disp_factor()),
                    format!("{:.2}", combo.max_reaction / 1000.0 * units.force_factor()),
                ]);
            }
        }
        BottomTab::Envelope => {
            let force = units.force_factor();
            let moment = units.moment_factor();
            headers = vec![
                "Frame".into(),
                format!("Axial max ({})", units.force_label()),
                format!("Axial min ({})", units.force_label()),
                format!("My,i max ({})", units.moment_label()),
                format!("My,i min ({})", units.moment_label()),
                format!("My,j max ({})", units.moment_label()),
                format!("My,j min ({})", units.moment_label()),
                format!("Mz,i max ({})", units.moment_label()),
                format!("Mz,i min ({})", units.moment_label()),
                format!("Mz,j max ({})", units.moment_label()),
                format!("Mz,j min ({})", units.moment_label()),
            ];
            for envelope in &app.member_envelopes {
                let mut row = vec![format!("F{}", envelope.element_id)];
                for (component, factor) in [
                    (envelope.axial, force),
                    (envelope.moment_y_i, moment),
                    (envelope.moment_y_j, moment),
                    (envelope.moment_z_i, moment),
                    (envelope.moment_z_j, moment),
                ] {
                    row.push(format!("{:.2}", component.max / 1000.0 * factor));
                    row.push(format!("{:.2}", component.min / 1000.0 * factor));
                }
                rows.push(row);
            }
        }
    }

    (headers, rows)
}

pub(crate) fn hint(ui: &mut egui::Ui) {
    ui.label("Run analysis first (F5, the toolbar Run button, or Analyze → Run Analysis).");
}
