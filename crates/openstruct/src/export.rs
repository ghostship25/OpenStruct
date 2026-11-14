//! Export Module.
//!
//! Exports structural analysis results to various formats, including CSV, XLSX,
//! and OpenStruct JSON project files for persistence and further processing.

use crate::analysis::Analysis;
use crate::design::composite::{CompositeDesignResults, CompositeDesignStatus};
use crate::design::concrete::{ConcreteDesignResults, ConcreteDesignStatus};
use crate::design::rebar::RebarDetailingResults;
use crate::design::shear_wall::{ShearWallDesignResults, ShearWallDesignStatus};
use crate::design::steel::{DesignResults, DesignStatus};
use crate::loads::LoadType;
use crate::model::Model;
use crate::reporting;
use std::fmt::Write as _;

// -- tabular CSV --------------------------------------------------------------

/// Quote one field when it carries a delimiter, a quote or a line break.
fn csv_field(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn push_csv_line(cells: &[String], csv: &mut String) {
    for (index, cell) in cells.iter().enumerate() {
        if index > 0 {
            csv.push(',');
        }
        csv.push_str(&csv_field(cell));
    }
    csv.push_str("\r\n");
}

/// Render a header row and its rows as CSV text with CRLF line endings.
pub fn rows_to_csv(headers: &[String], rows: &[Vec<String>]) -> String {
    let mut csv = String::new();
    push_csv_line(headers, &mut csv);
    for row in rows {
        push_csv_line(row, &mut csv);
    }
    csv
}

/// Split CSV text into records, honouring quoted fields and either line ending.
/// Empty lines are dropped.
pub fn parse_csv(text: &str) -> Vec<Vec<String>> {
    let mut records: Vec<Vec<String>> = Vec::new();
    let mut record: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '"' if quoted => {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    quoted = false;
                }
            }
            '"' if field.is_empty() => quoted = true,
            ',' if !quoted => record.push(std::mem::take(&mut field)),
            '\r' if !quoted => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                record.push(std::mem::take(&mut field));
                records.push(std::mem::take(&mut record));
            }
            '\n' if !quoted => {
                record.push(std::mem::take(&mut field));
                records.push(std::mem::take(&mut record));
            }
            other => field.push(other),
        }
    }
    if !field.is_empty() || !record.is_empty() {
        record.push(field);
        records.push(record);
    }
    records.retain(|row| !(row.len() == 1 && row[0].trim().is_empty()));
    records
}

/// Rows holding `filter` in any column, case-insensitively; blank keeps all.
pub fn filter_rows(rows: &[Vec<String>], filter: &str) -> Vec<Vec<String>> {
    let needle = filter.trim().to_lowercase();
    if needle.is_empty() {
        return rows.to_vec();
    }
    rows.iter()
        .filter(|row| row.iter().any(|cell| cell.to_lowercase().contains(&needle)))
        .cloned()
        .collect()
}

#[cfg(test)]
mod csv_tests {
    use super::*;

    #[test]
    fn csv_round_trip_keeps_quoted_cells() {
        let headers = vec!["Joint".to_string(), "Load (kN)".to_string()];
        let rows = vec![vec!["N1".to_string(), "1.0, 2.0, 3.0".to_string()]];
        let text = rows_to_csv(&headers, &rows);
        let back = parse_csv(&text);
        assert_eq!(back.len(), 2);
        assert_eq!(back[0], headers);
        assert_eq!(back[1][1], "1.0, 2.0, 3.0");
    }

    #[test]
    fn filter_matches_any_column_case_insensitively() {
        let rows = vec![
            vec!["N1".to_string(), "Base".to_string()],
            vec!["N2".to_string(), "Roof".to_string()],
        ];
        assert_eq!(filter_rows(&rows, "roof").len(), 1);
        assert_eq!(filter_rows(&rows, "n").len(), 2);
        assert_eq!(filter_rows(&rows, "nothing").len(), 0);
        assert_eq!(filter_rows(&rows, "   ").len(), 2);
    }
}

fn load_type_label(pattern: &LoadType) -> &'static str {
    match pattern {
        LoadType::Dead => "Dead",
        LoadType::Live => "Live",
        LoadType::Wind => "Wind",
        LoadType::Seismic => "Seismic",
        LoadType::Snow => "Snow",
        LoadType::Other(_) => "Other",
    }
}

fn steel_status_label(status: &DesignStatus) -> &'static str {
    match status {
        DesignStatus::Pass => "PASS",
        DesignStatus::Fail => "FAIL",
        DesignStatus::Pending => "PENDING",
    }
}

fn concrete_status_label(status: &ConcreteDesignStatus) -> &'static str {
    match status {
        ConcreteDesignStatus::Pass => "PASS",
        ConcreteDesignStatus::Fail => "FAIL",
        ConcreteDesignStatus::Pending => "PENDING",
    }
}

fn composite_status_label(status: &CompositeDesignStatus) -> &'static str {
    match status {
        CompositeDesignStatus::Pass => "PASS",
        CompositeDesignStatus::Fail => "FAIL",
        CompositeDesignStatus::Pending => "PENDING",
    }
}

fn shear_wall_status_label(status: &ShearWallDesignStatus) -> &'static str {
    match status {
        ShearWallDesignStatus::Pass => "PASS",
        ShearWallDesignStatus::Fail => "FAIL",
        ShearWallDesignStatus::Pending => "PENDING",
    }
}

/// Export analysis results to CSV format.
pub fn export_to_csv(
    analysis: &Analysis,
    steel_results: Option<&DesignResults>,
    concrete_results: Option<&ConcreteDesignResults>,
    composite_results: Option<&CompositeDesignResults>,
    shear_wall_results: Option<&ShearWallDesignResults>,
    rebar_results: Option<&RebarDetailingResults>,
) -> String {
    let mut csv = String::new();

    csv.push_str("OpenStruct CSV Export\n");
    let _ = writeln!(
        csv,
        "Generated: {}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    );
    let _ = writeln!(
        csv,
        "Analysis: {} load cases, {} elements",
        analysis.load_cases.len(),
        analysis.model.elements.len()
    );
    csv.push('\n');

    csv.push_str("=== Node Displacements ===\n");
    csv.push_str("node_id,ux,uy,uz,rx,ry,rz\n");
    csv.push_str("(no displacement data without analysis results)\n\n");

    csv.push_str("=== Reaction Forces ===\n");
    csv.push_str("node_id,Fx,Fy,Fz,Mx,My,Mz\n");
    csv.push_str("(no reaction data without analysis results)\n\n");

    csv.push_str("=== Member End Forces ===\n");
    csv.push_str("element_id,node_i,node_j,axial,moment_y_i,moment_y_j,moment_z_i,moment_z_j,shear_y,shear_z,twist_i,twist_j\n");
    csv.push_str("(no member force data without analysis results)\n\n");

    if let Some(steel) = steel_results {
        csv.push_str("=== Steel Design Results (AISC 360) ===\n");
        csv.push_str("member_id,section,material,axial_ratio,flexure_weak,flexure_strong,shear_ratio,util_ratio,status\n");
        for member in &steel.members {
            let _ = writeln!(
                csv,
                "{},{},{},{:.4},{:.4},{:.4},{:.4},{:.4},{}",
                member.member_id,
                member.section_name,
                member.material_name,
                member.axial_ratio,
                member.flexure_weak_ratio,
                member.flexure_strong_ratio,
                member.shear_ratio,
                member.util_ratio,
                steel_status_label(&member.status)
            );
        }
        csv.push('\n');
    }

    if let Some(concrete) = concrete_results {
        csv.push_str("=== Concrete Design Results (ACI 318) ===\n");
        csv.push_str("member_id,section,axial_ratio,flexure_weak,flexure_strong,shear_ratio,util_ratio,status\n");
        for member in &concrete.members {
            let _ = writeln!(
                csv,
                "{},{},{:.4},{:.4},{:.4},{:.4},{:.4},{}",
                member.member_id,
                member.section_name,
                member.axial_ratio,
                member.flexure_weak_ratio,
                member.flexure_strong_ratio,
                member.shear_ratio,
                member.util_ratio,
                concrete_status_label(&member.status)
            );
        }
        csv.push('\n');
    }

    if let Some(composite) = composite_results {
        csv.push_str("=== Composite Design Results (AISC 360/G) ===\n");
        csv.push_str("member_id,section,slab_width,slab_thickness,stud_spacing,util_ratio,status\n");
        for member in &composite.members {
            let _ = writeln!(
                csv,
                "{},{},{:.3},{:.3},{:.3},{:.4},{}",
                member.member_id,
                member.section_name,
                member.slab_width,
                member.slab_thickness,
                member.stud_spacing,
                member.util_ratio,
                composite_status_label(&member.status)
            );
        }
        csv.push('\n');
    }

    if let Some(shear_wall) = shear_wall_results {
        csv.push_str("=== Shear Wall Design Results ===\n");
        csv.push_str("wall_id,section,width,height,util_ratio,aspect_ratio,status\n");
        for wall in &shear_wall.walls {
            let _ = writeln!(
                csv,
                "{},{},{:.3},{:.3},{:.4},{},{}",
                wall.wall_id,
                wall.section_name,
                wall.wall_width,
                wall.wall_height,
                wall.util_ratio,
                wall.aspect_ratio,
                shear_wall_status_label(&wall.status)
            );
        }
        csv.push('\n');
    }

    if let Some(rebar) = rebar_results {
        csv.push_str("=== Rebar Detailing Results ===\n");
        csv.push_str("bar_size,ld_required,ld_provided,ratio,status\n");
        for dev in &rebar.development_lengths {
            let _ = writeln!(
                csv,
                "{:?},{:.0},{:.0},{:.3},{:?}",
                dev.bar_size, dev.l_d_required, dev.l_d_provided, dev.ratio, dev.status
            );
        }
        csv.push('\n');
    }

    csv.push_str("\n=== End of CSV Export ===\n");
    csv
}

/// One worksheet of an XLSX workbook.
#[derive(Debug, Clone)]
struct XlsxSheet {
    name: String,
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl XlsxSheet {
    fn new(name: &str, headers: &[&str]) -> Self {
        XlsxSheet {
            name: name.to_string(),
            headers: headers.iter().map(|header| header.to_string()).collect(),
            rows: Vec::new(),
        }
    }

    fn add_row<T: ToString>(&mut self, row: &[T]) {
        self.rows.push(row.iter().map(|cell| cell.to_string()).collect());
    }
}

/// Export analysis results to XLSX format.
///
/// The workbook layout is produced here; binary encoding requires the optional
/// `xlsx` crate, which is not part of this build, so the encoded bytes are
/// returned as an explicit error while the sheet model stays testable.
pub fn export_to_xlsx(
    analysis: &Analysis,
    steel_results: Option<&DesignResults>,
    concrete_results: Option<&ConcreteDesignResults>,
    composite_results: Option<&CompositeDesignResults>,
    shear_wall_results: Option<&ShearWallDesignResults>,
    rebar_results: Option<&RebarDetailingResults>,
) -> Result<Vec<u8>, String> {
    let mut sheets: Vec<XlsxSheet> = Vec::new();

    let mut load_cases_sheet = XlsxSheet::new(
        "Load Cases",
        &["Id", "Name", "Pattern", "Self Weight Mult", "P-Delta"],
    );
    for lc in &analysis.load_cases {
        load_cases_sheet.add_row(&[
            lc.id.to_string(),
            lc.name.clone(),
            load_type_label(&lc.pattern).to_string(),
            lc.self_weight_multiplier.to_string(),
            lc.p_delta.to_string(),
        ]);
    }
    sheets.push(load_cases_sheet);

    let mut displacements_sheet =
        XlsxSheet::new("Displacements", &["Node Id", "Ux", "Uy", "Uz", "Rx", "Ry", "Rz"]);
    displacements_sheet
        .add_row(&vec!["(no displacement data without analysis results)".to_string(); 7]);
    sheets.push(displacements_sheet);

    let mut reactions_sheet =
        XlsxSheet::new("Reactions", &["Node Id", "Fx", "Fy", "Fz", "Mx", "My", "Mz"]);
    reactions_sheet.add_row(&vec!["(no reaction data without analysis results)".to_string(); 7]);
    sheets.push(reactions_sheet);

    let mut member_forces_sheet = XlsxSheet::new(
        "Member Forces",
        &[
            "Element Id", "Node I", "Node J", "Axial", "My I", "My J", "Mz I", "Mz J", "Shear Y",
            "Shear Z", "Twist I", "Twist J",
        ],
    );
    member_forces_sheet
        .add_row(&vec!["(no member force data without analysis results)".to_string(); 12]);
    sheets.push(member_forces_sheet);

    let mut steel_sheet = XlsxSheet::new(
        "Steel Design (AISC 360)",
        &[
            "Member Id", "Section", "Material", "Axial", "Flexure Weak", "Flexure Strong", "Shear",
            "Utilization", "Status",
        ],
    );
    if let Some(steel) = steel_results {
        for member in &steel.members {
            steel_sheet.add_row(&[
                member.member_id.clone(),
                member.section_name.clone(),
                member.material_name.clone(),
                format!("{:.4}", member.axial_ratio),
                format!("{:.4}", member.flexure_weak_ratio),
                format!("{:.4}", member.flexure_strong_ratio),
                format!("{:.4}", member.shear_ratio),
                format!("{:.4}", member.util_ratio),
                steel_status_label(&member.status).to_string(),
            ]);
        }
    } else {
        steel_sheet.add_row(&vec!["(no steel design results)".to_string(); 9]);
    }
    sheets.push(steel_sheet);

    let mut concrete_sheet = XlsxSheet::new(
        "Concrete Design (ACI 318)",
        &[
            "Member Id", "Section", "Axial", "Flexure Weak", "Flexure Strong", "Shear",
            "Utilization", "Status",
        ],
    );
    if let Some(concrete) = concrete_results {
        for member in &concrete.members {
            concrete_sheet.add_row(&[
                member.member_id.clone(),
                member.section_name.clone(),
                format!("{:.4}", member.axial_ratio),
                format!("{:.4}", member.flexure_weak_ratio),
                format!("{:.4}", member.flexure_strong_ratio),
                format!("{:.4}", member.shear_ratio),
                format!("{:.4}", member.util_ratio),
                concrete_status_label(&member.status).to_string(),
            ]);
        }
    } else {
        concrete_sheet.add_row(&vec!["(no concrete design results)".to_string(); 8]);
    }
    sheets.push(concrete_sheet);

    let mut composite_sheet = XlsxSheet::new(
        "Composite Design (AISC 360)",
        &[
            "Member Id", "Section", "Slab Width", "Slab Thickness", "Stud Spacing", "Utilization",
            "Status",
        ],
    );
    if let Some(composite) = composite_results {
        for member in &composite.members {
            composite_sheet.add_row(&[
                member.member_id.clone(),
                member.section_name.clone(),
                format!("{:.3}", member.slab_width),
                format!("{:.3}", member.slab_thickness),
                format!("{:.3}", member.stud_spacing),
                format!("{:.4}", member.util_ratio),
                composite_status_label(&member.status).to_string(),
            ]);
        }
    } else {
        composite_sheet.add_row(&vec!["(no composite design results)".to_string(); 7]);
    }
    sheets.push(composite_sheet);

    let mut shear_wall_sheet = XlsxSheet::new(
        "Shear Wall Design",
        &["Wall Id", "Section", "Width", "Height", "Utilization", "Aspect Ratio", "Status"],
    );
    if let Some(shear_wall) = shear_wall_results {
        for wall in &shear_wall.walls {
            shear_wall_sheet.add_row(&[
                wall.wall_id.clone(),
                wall.section_name.clone(),
                format!("{:.3}", wall.wall_width),
                format!("{:.3}", wall.wall_height),
                format!("{:.4}", wall.util_ratio),
                wall.aspect_ratio.to_string(),
                shear_wall_status_label(&wall.status).to_string(),
            ]);
        }
    } else {
        shear_wall_sheet.add_row(&vec!["(no shear wall design results)".to_string(); 7]);
    }
    sheets.push(shear_wall_sheet);

    let mut rebar_sheet = XlsxSheet::new(
        "Rebar Detailing",
        &["Bar Size", "Ld Required", "Ld Provided", "Ratio", "Status"],
    );
    if let Some(rebar) = rebar_results {
        for dev in &rebar.development_lengths {
            rebar_sheet.add_row(&[
                format!("{:?}", dev.bar_size),
                format!("{:.0}", dev.l_d_required),
                format!("{:.0}", dev.l_d_provided),
                format!("{:.3}", dev.ratio),
                format!("{:?}", dev.status),
            ]);
        }
    } else {
        rebar_sheet.add_row(&vec!["(no rebar design results)".to_string(); 5]);
    }
    sheets.push(rebar_sheet);

    let sheet_count = sheets.len();
    Err(format!(
        "prepared {} worksheets; XLSX binary encoding requires the optional `xlsx` crate",
        sheet_count
    ))
}

/// Export the complete project to the native OpenStruct JSON format.
pub fn export_to_json(
    model: &Model,
    analysis: &Analysis,
    _steel_results: Option<&DesignResults>,
    _concrete_results: Option<&ConcreteDesignResults>,
    _composite_results: Option<&CompositeDesignResults>,
    _shear_wall_results: Option<&ShearWallDesignResults>,
    _rebar_results: Option<&RebarDetailingResults>,
    config: &reporting::ReportConfig,
) -> anyhow::Result<String> {
    let materials: Vec<crate::import::OpenStructMaterial> = model
        .materials
        .iter()
        .map(|(id, material)| crate::import::OpenStructMaterial {
            id: *id,
            mat_type: material.name.clone(),
            youngs_modulus: material.e,
            poisson_ratio: material.nu,
        })
        .collect();

    let sections: Vec<crate::import::OpenStructSection> = model
        .sections
        .iter()
        .map(|(id, section)| crate::import::OpenStructSection {
            id: *id,
            type_name: section.name.clone(),
            width: section.width,
            height: section.depth,
            area: section.area,
        })
        .collect();

    let nodes: Vec<crate::import::OpenStructNode> = model
        .nodes
        .iter()
        .map(|(id, node)| crate::import::OpenStructNode {
            id: *id,
            x: node.x,
            y: node.y,
            z: node.z,
            restrained: node.restrained,
        })
        .collect();

    let elements: Vec<crate::import::OpenStructElement> = model
        .elements
        .iter()
        .map(|(id, element)| crate::import::OpenStructElement {
            id: *id,
            node_i: element.node_i,
            node_j: element.node_j,
            material_id: element.material_id,
            section_id: element.section_id,
            elem_type: "frame".to_string(),
        })
        .collect();

    let load_cases: Vec<crate::import::OpenStructLoadCase> = analysis
        .load_cases
        .iter()
        .map(|lc| crate::import::OpenStructLoadCase {
            id: lc.id,
            name: lc.name.clone(),
            pattern: load_type_label(&lc.pattern).to_string(),
            self_weight_multiplier: lc.self_weight_multiplier,
            nodal_loads: lc
                .nodal_loads
                .iter()
                .map(|nl| crate::import::OpenStructNodalLoad {
                    node_id: nl.node_id,
                    fx: nl.fx,
                    fy: nl.fy,
                    fz: nl.fz,
                    mx: nl.mx,
                    my: nl.my,
                    mz: nl.mz,
                })
                .collect(),
            combination_eq: String::new(),
            p_delta: lc.p_delta,
        })
        .collect();

    let project = crate::import::OpenStructProject {
        metadata: crate::import::OpenStructProjectMetadata {
            name: model.name.clone(),
            description: Some(format!("OpenStruct Project: {}", config.title)),
            created: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        },
        materials,
        sections,
        nodes,
        elements,
        load_cases,
    };

    Ok(serde_json::to_string_pretty(&project)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::Analysis;
    use crate::loads::{LoadCase, LoadType, NodalLoad};
    use crate::model::Model;

    fn sample_analysis(name: &str) -> Analysis {
        let mut model = Model::new(name);
        let mat_id = model.add_material(crate::materials::Material::steel());
        let sec_id =
            model.add_section(crate::sections::Section::rectangular("RECT300X500", 0.3, 0.5));
        let n1 = model.add_node(0.0, 0.0, 0.0);
        let n2 = model.add_node(4.0, 0.0, 0.0);
        model.add_element(crate::elements::Element::frame(1, n1, n2, mat_id, sec_id));
        model.nodes.get_mut(&n1).unwrap().restrained = [true, true, true, true, false, false];
        model.nodes.get_mut(&n2).unwrap().restrained = [true, true, true, false, false, false];

        let mut analysis = Analysis::new(model);
        analysis.load_cases.push(LoadCase {
            id: 1,
            name: "CASE1".into(),
            pattern: LoadType::Live,
            self_weight_multiplier: 0.0,
            nodal_loads: vec![NodalLoad {
                node_id: n2,
                fx: 0.0,
                fy: 0.0,
                fz: -10_000.0,
                mx: 0.0,
                my: 0.0,
                mz: 0.0,
            }],
            distributed_loads: vec![],
            p_delta: false,
        });
        analysis
    }

    #[test]
    fn test_csv_export_basic() {
        let analysis = sample_analysis("csv-test");
        let csv = export_to_csv(&analysis, None, None, None, None, None);
        assert!(csv.contains("OpenStruct CSV Export"));
        assert!(csv.contains("Node Displacements"));
        assert!(csv.contains("Reaction Forces"));
        assert!(csv.contains("Member End Forces"));
    }

    #[test]
    fn test_csv_export_with_steel_results() {
        let analysis = sample_analysis("csv-test2");
        let steel = DesignResults::default();
        let csv = export_to_csv(&analysis, Some(&steel), None, None, None, None);
        assert!(csv.contains("Steel Design Results"));
        assert!(csv.contains("member_id"));
        assert!(csv.contains("axial_ratio"));
    }

    #[test]
    fn test_xlsx_export_reports_missing_crate() {
        let analysis = sample_analysis("xlsx-test");
        let result = export_to_xlsx(&analysis, None, None, None, None, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_json_export_roundtrip_shape() {
        let analysis = sample_analysis("json-test");
        let config = reporting::ReportConfig {
            title: "Test Project".into(),
            ..Default::default()
        };
        let json = export_to_json(&analysis.model, &analysis, None, None, None, None, None, &config)
            .expect("json export");
        assert!(json.contains("json-test"));
        let parsed: crate::import::OpenStructProject = serde_json::from_str(&json).expect("parse");
        assert_eq!(parsed.nodes.len(), 2);
        assert_eq!(parsed.elements.len(), 1);
    }

    #[test]
    fn test_xlsx_sheets_structure() {
        let mut sheet = XlsxSheet::new("Test", &["Header1", "Header2", "Header3"]);
        assert_eq!(sheet.name, "Test");
        assert_eq!(sheet.headers.len(), 3);
        sheet.add_row(&["Cell1", "Cell2", "Cell3"]);
        assert_eq!(sheet.rows.len(), 1);
        assert_eq!(
            sheet.rows[0],
            vec!["Cell1".to_string(), "Cell2".to_string(), "Cell3".to_string()]
        );
    }
}
