//! Import Module.
//!
//! Imports structural analysis data from E2K text format and native OpenStruct
//! JSON project files, and re-exports projects to JSON.

use crate::analysis::Analysis;
use crate::elements::Element;
use crate::loads::{ComboType, LoadCase, LoadType, NodalLoad};
use crate::materials::Material;
use crate::model::Model;
use crate::sections::Section;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

// -- tabular CSV import -------------------------------------------------------

use crate::sections::DesignerPart;
use crate::units::UnitSystem;

/// One joint row read from the Joint Coordinates table.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedJoint {
    /// Joint number from the file; `None` adds a new joint.
    pub id: Option<usize>,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub restrained: [bool; 6],
    /// Nodal load in kN, exactly as the Joints table shows it.
    pub load: Option<[f64; 3]>,
}

/// Joints read from CSV text, ready to apply to a model.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParsedJoints {
    pub rows: Vec<ParsedJoint>,
    /// Data rows that could not be read.
    pub skipped: usize,
}

impl ParsedJoints {
    /// Update the joints that already exist and add the rest. Returns how many
    /// rows were applied and the nodal loads (kN) keyed by joint.
    pub fn apply(&self, model: &mut Model) -> (usize, Vec<(usize, [f64; 3])>) {
        let mut applied = 0;
        let mut loads: Vec<(usize, [f64; 3])> = Vec::new();
        for row in &self.rows {
            let id = match row.id.filter(|id| model.nodes.contains_key(id)) {
                Some(id) => {
                    if let Some(node) = model.nodes.get_mut(&id) {
                        node.x = row.x;
                        node.y = row.y;
                        node.z = row.z;
                        node.restrained = row.restrained;
                    }
                    id
                }
                None => {
                    let id = model.add_node(row.x, row.y, row.z);
                    if let Some(node) = model.nodes.get_mut(&id) {
                        node.restrained = row.restrained;
                    }
                    id
                }
            };
            if let Some(load) = row.load {
                loads.push((id, load));
            }
            applied += 1;
        }
        (applied, loads)
    }
}

/// One frame section row read from the Frame Sections table.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedSection {
    pub name: String,
    pub shape: String,
    pub width: f64,
    pub depth: f64,
    /// Section Designer parts, when the file carried them.
    pub drawn: Option<Vec<DesignerPart>>,
}

/// Frame sections read from CSV text, ready to apply to a model.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParsedSections {
    pub rows: Vec<ParsedSection>,
    /// Data rows that could not be read.
    pub skipped: usize,
}

impl ParsedSections {
    /// Add every row as a new parametric section; returns how many were added.
    pub fn apply(&self, model: &mut Model) -> usize {
        let mut applied = 0;
        for row in &self.rows {
            let shape = row.shape.to_lowercase();
            let mut section = if shape.contains("circ") || shape.contains("pipe") || shape.contains("tube") {
                Section::circular(&row.name, row.width.max(1.0e-3))
            } else {
                Section::rectangular(&row.name, row.width.max(1.0e-3), row.depth.max(1.0e-3))
            };
            if let Some(parts) = &row.drawn {
                section.designer = parts.clone();
            }
            model.add_section(section);
            applied += 1;
        }
        applied
    }
}

fn csv_number(cell: &str) -> Option<f64> {
    let text = cell.trim();
    if text.is_empty() {
        return None;
    }
    text.parse::<f64>()
        .ok()
        .or_else(|| text.replace([' ', '\u{a0}'], "").parse::<f64>().ok())
}

fn csv_index(cell: &str) -> Option<usize> {
    let digits: String = cell.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

fn is_table_header(cell: &str) -> bool {
    matches!(
        cell.trim().to_lowercase().as_str(),
        "joint" | "frame" | "section" | "member" | "area" | "combination"
    )
}

fn csv_restraints(cell: &str) -> [bool; 6] {
    let mut out = [false; 6];
    let flags: Vec<char> = cell.chars().filter(|c| !c.is_whitespace()).take(6).collect();
    if flags.len() == 6 {
        for (index, flag) in flags.iter().enumerate() {
            out[index] = matches!(flag, 'T' | 't' | '1');
        }
    }
    out
}

fn csv_nodal_load(cell: &str) -> Option<[f64; 3]> {
    let parts: Vec<f64> = cell.split(',').filter_map(csv_number).collect();
    if parts.len() == 3 {
        Some([parts[0], parts[1], parts[2]])
    } else {
        None
    }
}

/// Keep the name from a `S12  NAME` cell, dropping the id prefix.
fn csv_section_name(cell: &str) -> String {
    let text = cell.trim();
    if let Some(rest) = text.strip_prefix('S').or_else(|| text.strip_prefix('s')) {
        let rest = rest.trim_start();
        if rest.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            let name = rest.trim_start_matches(|c: char| c.is_ascii_digit()).trim();
            if !name.is_empty() {
                return name.to_string();
            }
        }
    }
    if text.is_empty() {
        "CSV SECTION".to_string()
    } else {
        text.to_string()
    }
}

/// Read the Joint Coordinates table back from CSV text.
pub fn parse_joints(text: &str, units: UnitSystem) -> Result<ParsedJoints, String> {
    let records = crate::export::parse_csv(text);
    if records.is_empty() {
        return Err("Import failed: the file is empty.".to_string());
    }
    let mut parsed = ParsedJoints::default();
    for record in records {
        let first = record.first().map(String::as_str).unwrap_or("");
        if is_table_header(first) {
            continue;
        }
        if record.len() < 4 {
            parsed.skipped += 1;
            continue;
        }
        let (Some(x), Some(y), Some(z)) = (
            csv_number(&record[1]),
            csv_number(&record[2]),
            csv_number(&record[3]),
        ) else {
            parsed.skipped += 1;
            continue;
        };
        parsed.rows.push(ParsedJoint {
            id: csv_index(first),
            x: units.length_from_display(x),
            y: units.length_from_display(y),
            z: units.length_from_display(z),
            restrained: csv_restraints(record.get(4).map(String::as_str).unwrap_or("")),
            load: record.get(6).and_then(|cell| csv_nodal_load(cell)),
        });
    }
    Ok(parsed)
}

/// Read the Frame Sections table back from CSV text.
pub fn parse_sections(text: &str, units: UnitSystem) -> Result<ParsedSections, String> {
    let records = crate::export::parse_csv(text);
    if records.is_empty() {
        return Err("Import failed: the file is empty.".to_string());
    }
    let mut parsed = ParsedSections::default();
    for record in records {
        let first = record.first().map(String::as_str).unwrap_or("");
        if is_table_header(first) {
            continue;
        }
        if record.len() < 4 {
            parsed.skipped += 1;
            continue;
        }
        let (Some(width), Some(depth)) = (csv_number(&record[2]), csv_number(&record[3])) else {
            parsed.skipped += 1;
            continue;
        };
        parsed.rows.push(ParsedSection {
            name: csv_section_name(first),
            shape: record.get(1).cloned().unwrap_or_default(),
            width: units.length_from_display(width),
            depth: units.length_from_display(depth),
            drawn: record
                .get(6)
                .map(|cell| cell.trim())
                .filter(|cell| !cell.is_empty())
                .and_then(|cell| serde_json::from_str::<Vec<DesignerPart>>(cell).ok()),
        });
    }
    Ok(parsed)
}

#[cfg(test)]
mod csv_import_tests {
    use super::*;

    fn joint_csv() -> String {
        crate::export::rows_to_csv(
            &[
                "Joint".to_string(),
                "X (m)".to_string(),
                "Y (m)".to_string(),
                "Z (m)".to_string(),
                "Restraints".to_string(),
                "Springs".to_string(),
                "Load (kN)".to_string(),
            ],
            &[
                vec![
                    "N1".to_string(),
                    "0.000".to_string(),
                    "0.000".to_string(),
                    "0.000".to_string(),
                    "TTTTTT".to_string(),
                    "—".to_string(),
                    "—".to_string(),
                ],
                vec![
                    "N2".to_string(),
                    "5.000".to_string(),
                    "0.000".to_string(),
                    "3.500".to_string(),
                    "------".to_string(),
                    "—".to_string(),
                    "1.0, 2.0, 3.0".to_string(),
                ],
            ],
        )
    }

    #[test]
    fn joint_csv_round_trips_through_the_model() {
        let parsed = parse_joints(&joint_csv(), UnitSystem::Metric).expect("parse");
        assert_eq!(parsed.rows.len(), 2);
        assert_eq!(parsed.skipped, 0);
        assert!(parsed.rows[0].restrained[0] && parsed.rows[0].restrained[5]);
        assert_eq!(parsed.rows[1].load, Some([1.0, 2.0, 3.0]));

        let mut model = Model::new("csv import");
        let (applied, loads) = parsed.apply(&mut model);
        assert_eq!(applied, 2);
        assert_eq!(loads.len(), 1);
        let loaded = model.nodes.get(&loads[0].0).expect("loaded joint");
        assert!((loaded.x - 5.0).abs() < 1e-9);
        assert!((loaded.z - 3.5).abs() < 1e-9);
        assert!(model.nodes.values().any(|node| node.restrained[0] && node.restrained[5]));
    }

    #[test]
    fn section_csv_adds_parametric_sections() {
        let text = crate::export::rows_to_csv(
            &[
                "Section".to_string(),
                "Shape".to_string(),
                "Width (m)".to_string(),
                "Depth (m)".to_string(),
                "Web/Wall (m)".to_string(),
                "Flange (m)".to_string(),
                "Drawn".to_string(),
            ],
            &[vec![
                "S4  COL 400x400".to_string(),
                "Rectangular".to_string(),
                "0.4000".to_string(),
                "0.4000".to_string(),
                String::new(),
                String::new(),
                String::new(),
            ]],
        );
        let parsed = parse_sections(&text, UnitSystem::Metric).expect("parse");
        assert_eq!(parsed.rows.len(), 1);
        assert_eq!(parsed.rows[0].name, "COL 400x400");

        let mut model = Model::new("csv sections");
        assert_eq!(parsed.apply(&mut model), 1);
        let section = model.sections.values().next().expect("section");
        assert!((section.area - 0.16).abs() < 1e-9);
    }
}

/// E2K import data structures.
///
/// E2K (Element-to-Kayen) is a simple text format for frame structures:
/// nodes (id x y z + 6 restraint flags), elements (i/j nodes, material,
/// section), nodal and distributed loads.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E2KImportData {
    #[serde(rename = "nodes")]
    pub nodes: Vec<E2KNode>,
    #[serde(rename = "elements")]
    pub elements: Vec<E2KElement>,
    #[serde(rename = "load_cases")]
    pub load_cases: Vec<E2KLoadCase>,
    #[serde(rename = "materials")]
    pub materials: Vec<E2KMaterial>,
    #[serde(rename = "sections")]
    pub sections: Vec<E2KSection>,
}

/// E2K Node: id x y z with 6 restraint flags
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E2KNode {
    pub id: usize,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub restrained: [bool; 6],
}

/// E2K Element: id type node_i node_j material_id section_id
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E2KElement {
    pub id: usize,
    pub elem_type: String,
    pub node_i: usize,
    pub node_j: usize,
    pub material_id: usize,
    pub section_id: usize,
}

/// E2K Load Case
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E2KLoadCase {
    pub id: usize,
    pub name: String,
    pub pattern: String,
    #[serde(rename = "nodal_loads")]
    pub nodal_loads: Vec<E2KNodalLoad>,
    #[serde(rename = "distributed_loads")]
    pub distributed_loads: Vec<E2KDistributedLoad>,
}

/// E2K Nodal Load: node_id fx fy fz mx my mz
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E2KNodalLoad {
    pub node_id: usize,
    pub fx: f64,
    pub fy: f64,
    pub fz: f64,
    pub mx: f64,
    pub my: f64,
    pub mz: f64,
}

/// E2K Distributed Load
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E2KDistributedLoad {
    pub loadcase_id: usize,
    pub load_type: String,
    pub start_x: f64,
    pub start_y: f64,
    pub start_z: f64,
    pub end_x: f64,
    pub end_y: f64,
    pub end_z: f64,
    pub magnitude: f64,
}

/// E2K Material: id type youngs_modulus poisson_ratio
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E2KMaterial {
    pub id: usize,
    pub mat_type: String,
    pub youngs_modulus: f64,
    pub poisson_ratio: f64,
}

/// E2K Section: id type width height area
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E2KSection {
    pub id: usize,
    pub sec_type: String,
    pub width: f64,
    pub height: f64,
    pub area: f64,
}

/// Map a load pattern name onto an OpenStruct load type.
pub fn parse_load_pattern(name: &str) -> LoadType {
    match name.trim().to_lowercase().as_str() {
        "dead" | "d" | "sw" => LoadType::Dead,
        "live" | "l" | "reducible live" => LoadType::Live,
        "wind" | "w" => LoadType::Wind,
        "seismic" | "eq" | "quake" | "earthquake" => LoadType::Seismic,
        "snow" | "s" | "roof live" => LoadType::Snow,
        other => LoadType::Other(other.to_string()),
    }
}

/// Import E2K data into an OpenStruct Model + Analysis pair.
pub fn import_e2k(data: &E2KImportData) -> anyhow::Result<(Model, Analysis)> {
    let title = data
        .load_cases
        .first()
        .map_or_else(|| "E2K Import".to_string(), |c| format!("E2K Import - {}", c.name));
    let mut model = Model::new(&title);

    let mut material_ids: HashMap<usize, usize> = HashMap::new();
    for e2k_mat in &data.materials {
        let mut mat = match e2k_mat.mat_type.to_lowercase().as_str() {
            "concrete" => Material::concrete(),
            "aluminium" | "aluminum" => Material::aluminum(),
            _ => Material::steel(),
        };
        mat.name = format!("E2K_{}", e2k_mat.mat_type);
        if e2k_mat.youngs_modulus > 0.0 {
            mat.e = e2k_mat.youngs_modulus;
        }
        if e2k_mat.poisson_ratio > 0.0 {
            mat.nu = e2k_mat.poisson_ratio;
        }
        material_ids.insert(e2k_mat.id, model.add_material(mat));
    }

    let mut section_ids: HashMap<usize, usize> = HashMap::new();
    for e2k_sec in &data.sections {
        let height = if e2k_sec.height > 0.0 { e2k_sec.height } else { e2k_sec.width };
        let section = match e2k_sec.sec_type.to_lowercase().as_str() {
            "circle" | "circular" => {
                Section::circular(&format!("E2K_CIRCLE_{}", e2k_sec.id), e2k_sec.width)
            }
            other => Section::rectangular(
                &format!("E2K_{}_{}", other.to_uppercase(), e2k_sec.id),
                e2k_sec.width,
                height,
            ),
        };
        section_ids.insert(e2k_sec.id, model.add_section(section));
    }

    let mut node_ids: HashMap<usize, usize> = HashMap::new();
    for e2k_node in &data.nodes {
        let new_id = model.add_node(e2k_node.x, e2k_node.y, e2k_node.z);
        if let Some(node) = model.nodes.get_mut(&new_id) {
            node.restrained = e2k_node.restrained;
        }
        node_ids.insert(e2k_node.id, new_id);
    }

    for e2k_elem in &data.elements {
        let ends = (node_ids.get(&e2k_elem.node_i), node_ids.get(&e2k_elem.node_j));
        let (node_i, node_j) = match ends {
            (Some(i), Some(j)) => (*i, *j),
            _ => continue,
        };
        let mat_id = material_ids
            .get(&e2k_elem.material_id)
            .or_else(|| material_ids.values().next())
            .copied()
            .unwrap_or(0);
        let sec_id = section_ids
            .get(&e2k_elem.section_id)
            .or_else(|| section_ids.values().next())
            .copied()
            .unwrap_or(0);
        model.add_element(Element::frame(e2k_elem.id, node_i, node_j, mat_id, sec_id));
    }

    let mut analysis = Analysis::new(model);
    // Imported cases replace the fallback case Analysis::new seeds.
    analysis.load_cases.clear();
    for e2k_lc in &data.load_cases {
        let nodal_loads = e2k_lc
            .nodal_loads
            .iter()
            .map(|nl| NodalLoad {
                node_id: *node_ids.get(&nl.node_id).unwrap_or(&nl.node_id),
                fx: nl.fx,
                fy: nl.fy,
                fz: nl.fz,
                mx: nl.mx,
                my: nl.my,
                mz: nl.mz,
            })
            .collect();
        analysis.load_cases.push(LoadCase {
            id: e2k_lc.id,
            name: e2k_lc.name.clone(),
            pattern: parse_load_pattern(&e2k_lc.pattern),
            self_weight_multiplier: 0.0,
            nodal_loads,
            distributed_loads: vec![],
            p_delta: false,
        });
    }

    let model = analysis.model.clone();
    Ok((model, analysis))
}

/// Import a native OpenStruct JSON project file.
pub fn import_openstruct_json(path: &str) -> anyhow::Result<(Model, Analysis)> {
    let content = fs::read_to_string(path)?;
    import_openstruct_json_str(&content)
}

/// Import a native OpenStruct JSON project from a string.
pub fn import_openstruct_json_str(content: &str) -> anyhow::Result<(Model, Analysis)> {
    let project: OpenStructProject = serde_json::from_str(content)?;
    let mut model = Model::new(&project.metadata.name);

    let mut material_ids: HashMap<usize, usize> = HashMap::new();
    for mat_data in &project.materials {
        let mut mat = match mat_data.mat_type.to_lowercase().as_str() {
            "concrete" => Material::concrete(),
            "aluminium" | "aluminum" => Material::aluminum(),
            _ => Material::steel(),
        };
        mat.name = mat_data.mat_type.clone();
        if mat_data.youngs_modulus > 0.0 {
            mat.e = mat_data.youngs_modulus;
        }
        if mat_data.poisson_ratio > 0.0 {
            mat.nu = mat_data.poisson_ratio;
        }
        material_ids.insert(mat_data.id, model.add_material(mat));
    }

    let mut section_ids: HashMap<usize, usize> = HashMap::new();
    for sec_data in &project.sections {
        let height = if sec_data.height > 0.0 { sec_data.height } else { sec_data.width };
        let lowered = sec_data.type_name.to_lowercase();
        let section = if lowered.contains("circle") || lowered.contains("circ") {
            Section::circular(&sec_data.type_name, sec_data.width)
        } else {
            Section::rectangular(&sec_data.type_name, sec_data.width, height)
        };
        section_ids.insert(sec_data.id, model.add_section(section));
    }

    let mut node_ids: HashMap<usize, usize> = HashMap::new();
    for node_data in &project.nodes {
        let new_id = model.add_node(node_data.x, node_data.y, node_data.z);
        if let Some(node) = model.nodes.get_mut(&new_id) {
            node.restrained = node_data.restrained;
        }
        node_ids.insert(node_data.id, new_id);
    }

    for elem_data in &project.elements {
        let ends = (node_ids.get(&elem_data.node_i), node_ids.get(&elem_data.node_j));
        let (node_i, node_j) = match ends {
            (Some(i), Some(j)) => (*i, *j),
            _ => continue,
        };
        let mat_id = material_ids
            .get(&elem_data.material_id)
            .or_else(|| material_ids.values().next())
            .copied()
            .unwrap_or(0);
        let sec_id = section_ids
            .get(&elem_data.section_id)
            .or_else(|| section_ids.values().next())
            .copied()
            .unwrap_or(0);
        model.add_element(Element::frame(elem_data.id, node_i, node_j, mat_id, sec_id));
    }

    let mut analysis = Analysis::new(model);
    // Imported cases replace the fallback case Analysis::new seeds.
    analysis.load_cases.clear();
    for lc_data in &project.load_cases {
        let nodal_loads = lc_data
            .nodal_loads
            .iter()
            .map(|nl| NodalLoad {
                node_id: *node_ids.get(&nl.node_id).unwrap_or(&nl.node_id),
                fx: nl.fx,
                fy: nl.fy,
                fz: nl.fz,
                mx: nl.mx,
                my: nl.my,
                mz: nl.mz,
            })
            .collect();
        analysis.load_cases.push(LoadCase {
            id: lc_data.id,
            name: lc_data.name.clone(),
            pattern: parse_load_pattern(&lc_data.pattern),
            self_weight_multiplier: lc_data.self_weight_multiplier,
            nodal_loads,
            distributed_loads: vec![],
            p_delta: lc_data.p_delta,
        });
    }

    let model = analysis.model.clone();
    Ok((model, analysis))
}

/// Parse a combination equation such as `1.4*DEAD + 1.7*LIVE`.
pub fn parse_combination_eq(eq: &str) -> (Vec<(f64, String)>, ComboType) {
    let mut terms = Vec::new();
    for part in eq.split('+') {
        let token = part.trim();
        if token.is_empty() {
            continue;
        }
        let (factor, name) = match token.split_once('*') {
            Some((f, n)) => (f.trim().parse::<f64>().unwrap_or(1.0), n.trim().to_string()),
            None => (1.0, token.to_string()),
        };
        terms.push((factor, name));
    }
    let upper = eq.to_uppercase();
    let combo_type = if upper.contains("SRSS") {
        ComboType::Ssrs
    } else if upper.contains("CQC") {
        ComboType::Cqc
    } else if upper.contains("ENVELOPE") {
        ComboType::Envelope
    } else if upper.contains("ABS") {
        ComboType::Abs
    } else {
        ComboType::LinearAdd
    };
    (terms, combo_type)
}

/// Export the complete project state to native OpenStruct JSON.
pub fn export_openstruct_json(
    model: &Model,
    analysis: &Analysis,
    config: &crate::reporting::ReportConfig,
) -> anyhow::Result<String> {
    crate::export::export_to_json(model, analysis, None, None, None, None, None, config)
}

// ---- Native OpenStruct project data structures ----

/// OpenStruct project metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenStructProjectMetadata {
    pub name: String,
    pub description: Option<String>,
    pub created: u64,
}

/// OpenStruct material data for import/export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenStructMaterial {
    pub id: usize,
    pub mat_type: String,
    pub youngs_modulus: f64,
    pub poisson_ratio: f64,
}

/// OpenStruct section data for import/export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenStructSection {
    pub id: usize,
    pub type_name: String,
    pub width: f64,
    pub height: f64,
    #[serde(default)]
    pub area: f64,
}

/// OpenStruct node data for import/export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenStructNode {
    pub id: usize,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    #[serde(default)]
    pub restrained: [bool; 6],
}

/// OpenStruct element data for import/export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenStructElement {
    pub id: usize,
    pub node_i: usize,
    pub node_j: usize,
    pub material_id: usize,
    pub section_id: usize,
    #[serde(default)]
    pub elem_type: String,
}

/// OpenStruct load case data for import/export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenStructLoadCase {
    pub id: usize,
    pub name: String,
    pub pattern: String,
    #[serde(default)]
    pub self_weight_multiplier: f64,
    #[serde(rename = "nodal_loads", default)]
    pub nodal_loads: Vec<OpenStructNodalLoad>,
    #[serde(default)]
    pub combination_eq: String,
    #[serde(default)]
    pub p_delta: bool,
}

/// OpenStruct nodal load data for import/export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenStructNodalLoad {
    pub node_id: usize,
    pub fx: f64,
    pub fy: f64,
    pub fz: f64,
    pub mx: f64,
    pub my: f64,
    pub mz: f64,
}

/// Full OpenStruct project for import/export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenStructProject {
    pub metadata: OpenStructProjectMetadata,
    pub materials: Vec<OpenStructMaterial>,
    pub sections: Vec<OpenStructSection>,
    pub nodes: Vec<OpenStructNode>,
    pub elements: Vec<OpenStructElement>,
    pub load_cases: Vec<OpenStructLoadCase>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_e2k() -> E2KImportData {
        E2KImportData {
            nodes: vec![
                E2KNode { id: 1, x: 0.0, y: 0.0, z: 0.0, restrained: [true, true, true, true, false, false] },
                E2KNode { id: 2, x: 4.0, y: 0.0, z: 0.0, restrained: [true, true, true, false, false, false] },
                E2KNode { id: 3, x: 8.0, y: 0.0, z: 0.0, restrained: [true, true, true, false, false, false] },
            ],
            elements: vec![
                E2KElement { id: 1, elem_type: "frame".to_string(), node_i: 1, node_j: 2, material_id: 1, section_id: 1 },
                E2KElement { id: 2, elem_type: "frame".to_string(), node_i: 2, node_j: 3, material_id: 1, section_id: 1 },
            ],
            load_cases: vec![E2KLoadCase {
                id: 1,
                name: "CASE1".to_string(),
                pattern: "Live".to_string(),
                nodal_loads: vec![E2KNodalLoad {
                    node_id: 2, fx: 0.0, fy: 0.0, fz: -10_000.0, mx: 0.0, my: 0.0, mz: 0.0,
                }],
                distributed_loads: vec![],
            }],
            materials: vec![E2KMaterial {
                id: 1, mat_type: "steel".to_string(), youngs_modulus: 200e9, poisson_ratio: 0.3,
            }],
            sections: vec![E2KSection {
                id: 1, sec_type: "rectangular".to_string(), width: 0.3, height: 0.5, area: 0.15,
            }],
        }
    }

    #[test]
    fn test_e2k_import_builds_model() {
        let (model, analysis) = import_e2k(&sample_e2k()).expect("e2k import");
        assert_eq!(model.nodes.len(), 3);
        assert_eq!(model.elements.len(), 2);
        assert_eq!(analysis.load_cases.len(), 1);
        assert!(!model.materials.is_empty());
        assert!(!model.sections.is_empty());
    }

    #[test]
    fn test_e2k_import_restraints_and_nodal_loads() {
        let (model, analysis) = import_e2k(&sample_e2k()).expect("e2k import");
        let base = model.nodes.values().find(|n| n.x == 0.0).expect("base node");
        assert!(base.restrained[0] && base.restrained[3]);
        assert!(matches!(analysis.load_cases[0].pattern, LoadType::Live));
        assert_eq!(analysis.load_cases[0].nodal_loads.len(), 1);
        let loaded_id = analysis.load_cases[0].nodal_loads[0].node_id;
        assert!(model.nodes.contains_key(&loaded_id));
    }

    #[test]
    fn test_parse_load_pattern() {
        assert!(matches!(parse_load_pattern("Dead"), LoadType::Dead));
        assert!(matches!(parse_load_pattern("SNOW"), LoadType::Snow));
        assert!(matches!(parse_load_pattern("Temperature"), LoadType::Other(_)));
    }

    #[test]
    fn test_parse_combination_eq() {
        let (terms, ty) = parse_combination_eq("1.4*DEAD + 1.7*LIVE");
        assert_eq!(terms.len(), 2);
        assert!((terms[0].0 - 1.4).abs() < 1e-9);
        assert_eq!(terms[1].1, "LIVE");
        assert!(matches!(ty, ComboType::LinearAdd));
    }

    #[test]
    fn test_project_export_import_roundtrip() {
        let (model, analysis) = import_e2k(&sample_e2k()).expect("e2k import");
        let config = crate::reporting::ReportConfig {
            title: "Roundtrip".into(),
            ..Default::default()
        };
        let json = export_openstruct_json(&model, &analysis, &config).expect("export");
        let (model2, analysis2) = import_openstruct_json_str(&json).expect("import");
        assert_eq!(model2.nodes.len(), model.nodes.len());
        assert_eq!(model2.elements.len(), model.elements.len());
        assert_eq!(analysis2.load_cases.len(), analysis.load_cases.len());
        let loaded = analysis2.load_cases[0].nodal_loads[0].node_id;
        assert!(model2.nodes.contains_key(&loaded));
    }

    #[test]
    fn test_project_struct_roundtrip() {
        let project = OpenStructProject {
            metadata: OpenStructProjectMetadata {
                name: "Test Project".to_string(),
                description: Some("Test description".to_string()),
                created: 1_700_000_000,
            },
            materials: vec![],
            sections: vec![],
            nodes: vec![],
            elements: vec![],
            load_cases: vec![],
        };
        let json = serde_json::to_string(&project).unwrap();
        let back: OpenStructProject = serde_json::from_str(&json).unwrap();
        assert_eq!(back.metadata.name, "Test Project");
    }
}
