//! Generic table body: sortable columns and per-column filters.
//!
//! Rows come from [`crate::tables::csv_table`], so the visible table and the CSV
//! export share one column model and cannot drift apart.

use crate::app::{BottomTab, OpenStructApp};
use crate::tables::{csv_table, hint};
use eframe::egui;

/// Rows as displayed: global filter, per-column filters, then the chosen sort.
pub fn visible_rows(app: &OpenStructApp, headers: &[String], rows: Vec<Vec<String>>) -> Vec<Vec<String>> {
    let mut rows = openstruct::export::filter_rows(&rows, &app.table_filter);
    if app.table_filters.len() == headers.len() {
        rows.retain(|row| {
            app.table_filters.iter().enumerate().all(|(column, filter)| {
                let filter = filter.trim().to_lowercase();
                filter.is_empty() || row.get(column).is_some_and(|cell| cell.to_lowercase().contains(&filter))
            })
        });
    }
    if let Some((column, descending)) = app.table_sort {
        rows.sort_by(|left, right| {
            let left = left.get(column).map(String::as_str).unwrap_or("");
            let right = right.get(column).map(String::as_str).unwrap_or("");
            // Numeric columns sort as numbers; everything else case-insensitively.
            let order = match (left.trim().parse::<f64>(), right.trim().parse::<f64>()) {
                (Ok(left), Ok(right)) => left.partial_cmp(&right).unwrap_or(std::cmp::Ordering::Equal),
                _ => left.to_lowercase().cmp(&right.to_lowercase()),
            };
            if descending { order.reverse() } else { order }
        });
    }
    rows
}

/// Message when the active table has nothing to show yet.
fn empty_message(app: &OpenStructApp) -> Option<String> {
    let has_supports = app.model.nodes.values().any(|node| node.has_support());
    match app.bottom_tab {
        BottomTab::Joints if app.model.nodes.is_empty() => Some("No joints defined; draw joints first.".to_string()),
        BottomTab::FrameLoads if app.model.frame_loads.is_empty() => {
            Some("No frame loads assigned. Select frames and use Assign → Frame Loads.".to_string())
        }
        BottomTab::AreaLoads if app.model.area_loads.is_empty() => {
            Some("No area loads assigned. Select areas and use Assign → Area Loads.".to_string())
        }
        BottomTab::TemperatureLoads if app.model.temperature_loads.is_empty() => {
            Some("No temperature or strain loads assigned. Select frames and use Assign → Temperature / Strain Loads.".to_string())
        }
        BottomTab::Sections if app.model.sections.is_empty() => Some("No frame sections defined. Use Define → Frame Sections.".to_string()),
        BottomTab::Reactions if !has_supports => Some("No supports defined. Select joints and use Assign → Joint Restraints.".to_string()),
        BottomTab::Displacements | BottomTab::Forces | BottomTab::Reactions
            if crate::tables::active_results(app).is_none() =>
        {
            Some("Run analysis first (F5) to see these results.".to_string())
        }
        BottomTab::Combinations if app.combo_results.is_empty() => {
            Some("No combination results yet; run the analysis and its combinations.".to_string())
        }
        BottomTab::Envelope if app.member_envelopes.is_empty() => {
            Some("No envelopes yet; run the analysis and its combinations.".to_string())
        }
        _ => None,
    }
}

pub fn table_body(app: &mut OpenStructApp, ui: &mut egui::Ui) {
    if let Some(message) = empty_message(app) {
        let needs_results = message.starts_with("Run analysis");
        ui.label(message);
        if needs_results {
            hint(ui);
        }
        return;
    }
    let (headers, all_rows) = csv_table(app);
    if app.table_filters.len() != headers.len() {
        app.table_filters = vec![String::new(); headers.len()];
    }
    let pal = crate::theme::palette(app.dark);
    let mut sort_request: Option<usize> = None;
    let mut shown = 0usize;
    egui::ScrollArea::both().id_salt("tables_scroll").auto_shrink([false, false]).show(ui, |ui| {
        egui::Grid::new("table_grid").striped(true).num_columns(headers.len()).show(ui, |ui| {
            for (column, header) in headers.iter().enumerate() {
                let suffix = match app.table_sort {
                    Some((sorted, descending)) if sorted == column => {
                        if descending {
                            " ↓"
                        } else {
                            " ↑"
                        }
                    }
                    _ => "",
                };
                if ui
                    .small_button(format!("{header}{suffix}"))
                    .on_hover_text("Click to sort by this column")
                    .clicked()
                {
                    sort_request = Some(column);
                }
            }
            ui.end_row();
            for filter in app.table_filters.iter_mut() {
                ui.add(egui::TextEdit::singleline(filter).hint_text("filter").desired_width(84.0));
            }
            ui.end_row();
            let rows = visible_rows(app, &headers, all_rows);
            shown = rows.len();
            for row in &rows {
                for cell in row {
                    ui.label(cell);
                }
                ui.end_row();
            }
        });
    });
    if let Some(column) = sort_request {
        app.table_sort = match app.table_sort {
            Some((sorted, false)) if sorted == column => Some((column, true)),
            Some((sorted, true)) if sorted == column => None,
            _ => Some((column, false)),
        };
    }
    ui.label(egui::RichText::new(format!("{shown} row(s)")).color(pal.text_dim));
}
