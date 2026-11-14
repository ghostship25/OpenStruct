//! E2K text: a plain, line-based exchange format for a whole model.
//!
//! The writer and the reader live together so they cannot drift apart: every
//! record [`export_e2k`] writes is a record [`parse_e2k_text`] reads, and the
//! round trip preserves the geometry, properties, supports, stories, diaphragms
//! and loads.
//!
//! ## Format
//!
//! One record per line, fields separated by `|`, lengths in metres, forces in
//! newtons. Blank lines and lines starting with `!` are ignored, and records the
//! reader does not know are skipped so extra header blocks still import.
//!
//! ```text
//! TITLE|name
//! MATERIAL|id|name|E|nu|rho|fy|fu
//! SECTION|id|name|R or C|width or diameter|depth
//! JOINT|id|x|y|z|ux|uy|uz|rx|ry|rz
//! FRAME|id|node_i|node_j|material_id|section_id
//! STORY|name|elevation
//! DIAPHRAGM|id|name|elevation|out_of_plane
//! CASE|id|name|pattern|self_weight_multiplier
//! NODAL|case_id|node_id|fx|fy|fz|mx|my|mz
//! DIST|case_id|element_id|G or L|sx|sy|sz|ex|ey|ez|projected
//! ```

use crate::analysis::Analysis;
use crate::elements::Element;
use crate::loads::{DistributedLoad, LoadCase, LoadDirection, NodalLoad};
use crate::materials::Material;
use crate::model::{Diaphragm, Model, Story};
use crate::sections::Section;
use std::collections::HashMap;
use std::fmt::Write as _;

/// Stable name for a load pattern in the text file.
fn pattern_name(pattern: &crate::loads::LoadType) -> &'static str {
    match pattern {
        crate::loads::LoadType::Dead => "Dead",
        crate::loads::LoadType::Live => "Live",
        crate::loads::LoadType::Wind => "Wind",
        crate::loads::LoadType::Seismic => "Seismic",
        crate::loads::LoadType::Snow => "Snow",
        crate::loads::LoadType::Other(_) => "Other",
    }
}

/// Write the model as E2K text.
pub fn export_e2k(model: &Model) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "! OpenStruct E2K text export");
    let _ = writeln!(out, "TITLE|{}", model.name);

    let mut ids: Vec<usize> = model.materials.keys().copied().collect();
    ids.sort_unstable();
    for id in ids {
        if let Some(material) = model.materials.get(&id) {
            let _ = writeln!(
                out,
                "MATERIAL|{}|{}|{:.6}|{:.6}|{:.3}|{:.1}|{:.1}",
                id, material.name, material.e, material.nu, material.rho, material.fy, material.fu
            );
        }
    }

    let mut ids: Vec<usize> = model.sections.keys().copied().collect();
    ids.sort_unstable();
    for id in ids {
        if let Some(section) = model.sections.get(&id) {
            let (tag, first, second) = match section.shape {
                Some(crate::sections::SectionShape::Circular { diameter }) => ("C", diameter, 0.0),
                _ => ("R", section.width, section.depth),
            };
            let _ = writeln!(out, "SECTION|{}|{}|{}|{:.6}|{:.6}", id, section.name, tag, first, second);
        }
    }

    let mut ids: Vec<usize> = model.nodes.keys().copied().collect();
    ids.sort_unstable();
    for id in ids {
        if let Some(node) = model.nodes.get(&id) {
            let _ = writeln!(
                out,
                "JOINT|{}|{:.6}|{:.6}|{:.6}|{}",
                id,
                node.x,
                node.y,
                node.z,
                node.restrained.map(|flag| if flag { "1" } else { "0" }).join("|")
            );
        }
    }

    let mut ids: Vec<usize> = model.elements.keys().copied().collect();
    ids.sort_unstable();
    for id in ids {
        if let Some(element) = model.elements.get(&id) {
            let _ = writeln!(
                out,
                "FRAME|{}|{}|{}|{}|{}",
                id, element.node_i, element.node_j, element.material_id, element.section_id
            );
        }
    }

    for story in &model.stories {
        let _ = writeln!(out, "STORY|{}|{:.6}", story.name, story.elevation);
    }

    for diaphragm in &model.diaphragms {
        let _ = writeln!(
            out,
            "DIAPHRAGM|{}|{}|{:.6}|{}",
            diaphragm.id,
            diaphragm.name,
            diaphragm.story,
            if diaphragm.out_of_plane { "1" } else { "0" }
        );
    }

    for case in &model.load_cases {
        let _ = writeln!(
            out,
            "CASE|{}|{}|{}|{:.6}",
            case.id,
            case.name,
            pattern_name(&case.pattern),
            case.self_weight_multiplier
        );
        for load in &case.nodal_loads {
            let _ = writeln!(
                out,
                "NODAL|{}|{}|{:.6}|{:.6}|{:.6}|{:.6}|{:.6}|{:.6}",
                case.id, load.node_id, load.fx, load.fy, load.fz, load.mx, load.my, load.mz
            );
        }
        for load in &case.distributed_loads {
            let _ = writeln!(
                out,
                "DIST|{}|{}|{}|{:.6}|{:.6}|{:.6}|{:.6}|{:.6}|{:.6}|{}",
                case.id,
                load.element_id,
                match load.direction {
                    LoadDirection::Global => "G",
                    LoadDirection::Local => "L",
                },
                load.start[0],
                load.start[1],
                load.start[2],
                load.end[0],
                load.end[1],
                load.end[2],
                if load.projected { "1" } else { "0" }
            );
        }
    }

    out
}

/// Read the E2K text [`export_e2k`] writes.
pub fn parse_e2k_text(text: &str) -> anyhow::Result<(Model, Analysis)> {
    let mut model = Model::new("E2K text");
    let mut material_rows: Vec<(usize, String, f64, f64, f64, f64, f64)> = Vec::new();
    let mut section_rows: Vec<(usize, String, bool, f64, f64)> = Vec::new();
    let mut joint_rows: Vec<(usize, f64, f64, f64, [bool; 6])> = Vec::new();
    let mut frame_rows: Vec<(usize, usize, usize, usize, usize)> = Vec::new();
    let mut story_rows: Vec<(String, f64)> = Vec::new();
    let mut diaphragm_rows: Vec<(usize, String, f64, bool)> = Vec::new();
    let mut case_rows: Vec<(usize, String, String, f64)> = Vec::new();
    let mut nodal_rows: Vec<(usize, usize, [f64; 6])> = Vec::new();
    let mut dist_rows: Vec<(usize, usize, LoadDirection, [f64; 3], [f64; 3], bool)> = Vec::new();

    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('!') {
            continue;
        }
        let parts: Vec<&str> = line.split('|').map(str::trim).collect();
        let record = parts.first().copied().unwrap_or_default().to_ascii_uppercase();
        let whole = |at: usize| {
            parts
                .get(at)
                .and_then(|value| value.parse::<usize>().ok())
                .ok_or_else(|| anyhow::anyhow!("line {}: expected an integer at field {}: `{line}`", index + 1, at + 1))
        };
        let number = |at: usize| {
            parts
                .get(at)
                .and_then(|value| value.parse::<f64>().ok())
                .ok_or_else(|| anyhow::anyhow!("line {}: expected a number at field {}: `{line}`", index + 1, at + 1))
        };
        let text_at = |at: usize| parts.get(at).map(|value| (*value).to_string()).unwrap_or_default();
        let flag = |at: usize| {
            parts
                .get(at)
                .map(|value| *value == "1" || value.eq_ignore_ascii_case("true"))
                .unwrap_or(false)
        };

        match record.as_str() {
            "TITLE" => {
                if let Some(title) = parts.get(1) {
                    model.name = (*title).to_string();
                }
            }
            "MATERIAL" => material_rows.push((
                whole(1)?,
                text_at(2),
                number(3)?,
                number(4)?,
                number(5).unwrap_or(0.0),
                number(6).unwrap_or(0.0),
                number(7).unwrap_or(0.0),
            )),
            "SECTION" => section_rows.push((
                whole(1)?,
                text_at(2),
                text_at(3).eq_ignore_ascii_case("C"),
                number(4)?,
                number(5).unwrap_or(0.0),
            )),
            "JOINT" => joint_rows.push((
                whole(1)?,
                number(2)?,
                number(3)?,
                number(4)?,
                [flag(5), flag(6), flag(7), flag(8), flag(9), flag(10)],
            )),
            "FRAME" => frame_rows.push((whole(1)?, whole(2)?, whole(3)?, whole(4)?, whole(5)?)),
            "STORY" => story_rows.push((text_at(1), number(2)?)),
            "DIAPHRAGM" => diaphragm_rows.push((whole(1)?, text_at(2), number(3)?, flag(4))),
            "CASE" => case_rows.push((whole(1)?, text_at(2), text_at(3), number(4).unwrap_or(1.0))),
            "NODAL" => nodal_rows.push((
                whole(1)?,
                whole(2)?,
                [
                    number(3)?,
                    number(4)?,
                    number(5)?,
                    number(6).unwrap_or(0.0),
                    number(7).unwrap_or(0.0),
                    number(8).unwrap_or(0.0),
                ],
            )),
            "DIST" => dist_rows.push((
                whole(1)?,
                whole(2)?,
                if text_at(3).eq_ignore_ascii_case("L") {
                    LoadDirection::Local
                } else {
                    LoadDirection::Global
                },
                [number(4)?, number(5)?, number(6)?],
                [number(7)?, number(8)?, number(9)?],
                flag(10),
            )),
            _ => {}
        }
    }

    let mut material_ids: HashMap<usize, usize> = HashMap::new();
    for (id, name, e, nu, rho, fy, fu) in material_rows {
        let mut material = Material::new(&name, e, nu, rho);
        material.fy = fy;
        material.fu = fu;
        material_ids.insert(id, model.add_material(material));
    }

    let mut section_ids: HashMap<usize, usize> = HashMap::new();
    for (id, name, circular, first, second) in section_rows {
        let section = if circular {
            Section::circular(&name, first)
        } else {
            Section::rectangular(&name, first, second.max(0.001))
        };
        section_ids.insert(id, model.add_section(section));
    }

    if !story_rows.is_empty() {
        model.stories = story_rows
            .into_iter()
            .map(|(name, elevation)| Story::new(&name, elevation))
            .collect();
    }

    let mut node_ids: HashMap<usize, usize> = HashMap::new();
    for (id, x, y, z, restrained) in joint_rows {
        let new_id = model.add_node(x, y, z);
        if let Some(node) = model.nodes.get_mut(&new_id) {
            node.restrained = restrained;
        }
        node_ids.insert(id, new_id);
    }

    let mut element_ids: HashMap<usize, usize> = HashMap::new();
    for (id, node_i, node_j, material_id, section_id) in frame_rows {
        let (Some(&node_i), Some(&node_j)) = (node_ids.get(&node_i), node_ids.get(&node_j)) else {
            continue;
        };
        let material = material_ids
            .get(&material_id)
            .copied()
            .or_else(|| material_ids.values().next().copied())
            .unwrap_or(0);
        let section = section_ids
            .get(&section_id)
            .copied()
            .or_else(|| section_ids.values().next().copied())
            .unwrap_or(0);
        let new_id = model.add_element(Element::frame(1, node_i, node_j, material, section));
        element_ids.insert(id, new_id);
    }

    let mut diaphragms: Vec<Diaphragm> = diaphragm_rows
        .into_iter()
        .map(|(id, name, story, out_of_plane)| Diaphragm {
            id,
            name,
            story,
            out_of_plane,
        })
        .collect();
    diaphragms.sort_by_key(|diaphragm| diaphragm.id);
    model.diaphragms = diaphragms;

    let mut cases: Vec<LoadCase> = case_rows
        .into_iter()
        .map(|(id, name, pattern, self_weight_multiplier)| LoadCase {
            id,
            name,
            pattern: crate::import::parse_load_pattern(&pattern),
            self_weight_multiplier,
            nodal_loads: Vec::new(),
            distributed_loads: Vec::new(),
            p_delta: false,
        })
        .collect();

    for (case_id, node_id, loads) in nodal_rows {
        let Some(&node_id) = node_ids.get(&node_id) else { continue };
        if let Some(case) = cases.iter_mut().find(|case| case.id == case_id) {
            case.nodal_loads.push(NodalLoad {
                node_id,
                fx: loads[0],
                fy: loads[1],
                fz: loads[2],
                mx: loads[3],
                my: loads[4],
                mz: loads[5],
            });
        }
    }

    for (case_id, element_id, direction, start, end, projected) in dist_rows {
        let Some(&element_id) = element_ids.get(&element_id) else { continue };
        if let Some(case) = cases.iter_mut().find(|case| case.id == case_id) {
            case.distributed_loads.push(DistributedLoad {
                element_id,
                direction,
                start,
                end,
                projected,
                case_id: Some(case_id),
            });
        }
    }

    model.load_cases = cases;
    let analysis = Analysis::new(model.clone());
    Ok((model, analysis))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Story;

    fn sample_model() -> Model {
        let mut model = Model::new("Round trip");
        let material = model.add_material(Material::steel());
        let section = model.add_section(Section::rectangular("COL 400x400", 0.4, 0.4));
        model.stories = vec![Story::new("Story1", 0.0), Story::new("Story2", 3.5)];
        let a = model.add_node(0.0, 0.0, 0.0);
        let b = model.add_node(0.0, 0.0, 3.5);
        if let Some(node) = model.nodes.get_mut(&a) {
            node.restrained = [true; 6];
        }
        let element = model.add_element(Element::frame(1, a, b, material, section));
        model.diaphragms.push(Diaphragm::new(1, "D1", 3.5));
        if let Some(node) = model.nodes.get_mut(&b) {
            node.diaphragm = Some(1);
        }
        model.load_cases.push(LoadCase {
            id: 1,
            name: "DEAD".to_string(),
            pattern: crate::loads::LoadType::Dead,
            self_weight_multiplier: 1.0,
            nodal_loads: vec![NodalLoad {
                node_id: b,
                fx: 1000.0,
                fy: 0.0,
                fz: -5000.0,
                mx: 0.0,
                my: 0.0,
                mz: 0.0,
            }],
            distributed_loads: vec![DistributedLoad {
                element_id: element,
                direction: LoadDirection::Global,
                start: [0.0, 0.0, -2000.0],
                end: [0.0, 0.0, -2000.0],
                projected: false,
                case_id: Some(1),
            }],
            p_delta: false,
        });
        model
    }

    #[test]
    fn export_writes_one_record_per_object() {
        let text = export_e2k(&sample_model());
        assert!(text.starts_with("! OpenStruct E2K text export"));
        assert!(text.contains("TITLE|Round trip"));
        assert_eq!(text.lines().filter(|line| line.starts_with("JOINT|")).count(), 2);
        assert_eq!(text.lines().filter(|line| line.starts_with("FRAME|")).count(), 1);
        assert_eq!(text.lines().filter(|line| line.starts_with("NODAL|")).count(), 1);
        assert_eq!(text.lines().filter(|line| line.starts_with("DIST|")).count(), 1);
        assert!(text.contains("DIAPHRAGM|1|D1|3.500000|0"));
    }

    #[test]
    fn a_model_survives_the_round_trip() {
        let original = sample_model();
        let (restored, analysis) = parse_e2k_text(&export_e2k(&original)).expect("valid text");
        assert_eq!(restored.name, original.name);
        assert_eq!(restored.nodes.len(), original.nodes.len());
        assert_eq!(restored.elements.len(), original.elements.len());
        assert_eq!(restored.stories.len(), 2);
        assert_eq!(restored.diaphragms.len(), 1);
        assert_eq!(restored.diaphragms[0].name, "D1");
        assert_eq!(analysis.load_cases.len(), 1);
        assert_eq!(analysis.load_cases[0].nodal_loads.len(), 1);
        assert!((analysis.load_cases[0].nodal_loads[0].fz + 5000.0).abs() < 1e-6);
        assert_eq!(analysis.load_cases[0].distributed_loads.len(), 1);
        assert!(restored.nodes.values().any(|node| node.restrained == [true; 6]));
        let section = restored.sections.values().next().expect("a section");
        assert!((section.width - 0.4).abs() < 1e-6 && (section.depth - 0.4).abs() < 1e-6);
    }

    #[test]
    fn unknown_records_and_comments_are_skipped() {
        let text = "! header\nUNITS|m\nTITLE|Loose file\n\nJOINT|1|0|0|0|1|1|1|1|1|1\n";
        let (model, _) = parse_e2k_text(text).expect("valid text");
        assert_eq!(model.name, "Loose file");
        assert_eq!(model.nodes.len(), 1);
        assert_eq!(model.nodes[&1].restrained, [true; 6]);
    }

    #[test]
    fn a_malformed_record_names_its_line() {
        let error = match parse_e2k_text("JOINT|1|zero|0|0|0|0|0|0|0|0") {
            Ok(_) => panic!("a malformed joint should not parse"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("line 1"), "unexpected error: {error}");
    }
}
