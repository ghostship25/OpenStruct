//! Pre-analysis model checks.
//!
//! These are the diagnostics a structural workflow shows before a run — the
//! "Check Model" step: problems that would make the solve fail or the results
//! meaningless, kept separate from advisory notes so a caller can decide
//! whether to block the run.

use crate::model::Model;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Joints closer than this are the same point, in metres.
pub const COINCIDENT_TOLERANCE: f64 = 1.0e-6;
/// Members shorter than this count as zero length, in metres.
pub const MIN_MEMBER_LENGTH: f64 = 1.0e-6;
/// Levels closer than this are the same story, in metres.
pub const STORY_TOLERANCE: f64 = 1.0e-3;
/// Most coincident-joint pairs reported before the list is truncated.
const COINCIDENT_REPORT_LIMIT: usize = 25;

/// How serious a check item is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum CheckSeverity {
    /// The analysis cannot be trusted or cannot run.
    Error,
    /// Suspicious, but the analysis still runs.
    Warning,
    /// Informational note.
    Info,
}

impl CheckSeverity {
    pub fn label(self) -> &'static str {
        match self {
            CheckSeverity::Error => "Error",
            CheckSeverity::Warning => "Warning",
            CheckSeverity::Info => "Info",
        }
    }

    /// True when the item stops a run.
    pub fn is_blocking(self) -> bool {
        matches!(self, CheckSeverity::Error)
    }
}

/// Which part of the model an item belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckCategory {
    Model,
    Geometry,
    Properties,
    Loading,
    Stability,
    Diaphragms,
}

impl CheckCategory {
    pub fn label(self) -> &'static str {
        match self {
            CheckCategory::Model => "Model",
            CheckCategory::Geometry => "Geometry",
            CheckCategory::Properties => "Properties",
            CheckCategory::Loading => "Loading",
            CheckCategory::Stability => "Stability",
            CheckCategory::Diaphragms => "Diaphragms",
        }
    }
}

/// One finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckItem {
    pub severity: CheckSeverity,
    pub category: CheckCategory,
    /// The object the item refers to, e.g. `Joint 7` or `Frame 3`.
    pub object: String,
    pub message: String,
}

impl CheckItem {
    pub fn new(
        severity: CheckSeverity,
        category: CheckCategory,
        object: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            category,
            object: object.into(),
            message: message.into(),
        }
    }

    pub fn error(category: CheckCategory, object: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(CheckSeverity::Error, category, object, message)
    }

    pub fn warning(category: CheckCategory, object: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(CheckSeverity::Warning, category, object, message)
    }

    pub fn info(category: CheckCategory, object: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(CheckSeverity::Info, category, object, message)
    }
}

/// Every finding for one model, in the order the checks ran.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckReport {
    pub items: Vec<CheckItem>,
}

impl CheckReport {
    pub fn errors(&self) -> usize {
        self.items.iter().filter(|item| item.severity == CheckSeverity::Error).count()
    }

    pub fn warnings(&self) -> usize {
        self.items.iter().filter(|item| item.severity == CheckSeverity::Warning).count()
    }

    pub fn infos(&self) -> usize {
        self.items.iter().filter(|item| item.severity == CheckSeverity::Info).count()
    }

    /// True when nothing blocks an analysis run.
    pub fn can_analyse(&self) -> bool {
        self.errors() == 0
    }

    /// Items of one severity, most severe first.
    pub fn by_severity(&self, severity: CheckSeverity) -> Vec<&CheckItem> {
        self.items.iter().filter(|item| item.severity == severity).collect()
    }

    /// One line for the status bar: `2 errors, 5 warnings, 1 note`.
    pub fn summary(&self) -> String {
        format!(
            "{} error(s), {} warning(s), {} note(s)",
            self.errors(),
            self.warnings(),
            self.infos()
        )
    }
}

/// Run every check against a model.
pub fn check_model(model: &Model) -> CheckReport {
    let mut items = Vec::new();

    if model.nodes.is_empty() {
        items.push(CheckItem::error(CheckCategory::Model, "Model", "The model has no joints."));
    } else if model.elements.is_empty() && model.areas.is_empty() {
        items.push(CheckItem::error(
            CheckCategory::Model,
            "Model",
            "The model has no frame or area objects to analyse.",
        ));
    }

    check_geometry(model, &mut items);
    check_properties(model, &mut items);
    check_loading(model, &mut items);
    check_stability(model, &mut items);
    check_diaphragms(model, &mut items);

    CheckReport { items }
}

/// Coincident joints, zero-length members, disconnected joints and levels.
fn check_geometry(model: &Model, items: &mut Vec<CheckItem>) {
    let mut coincidents = 0usize;
    let mut ids: Vec<usize> = model.nodes.keys().copied().collect();
    ids.sort_unstable();
    for (position, &first) in ids.iter().enumerate() {
        let Some(a) = model.nodes.get(&first) else { continue };
        for &second in ids.iter().skip(position + 1) {
            let Some(b) = model.nodes.get(&second) else { continue };
            let near = (a.x - b.x).abs() <= COINCIDENT_TOLERANCE
                && (a.y - b.y).abs() <= COINCIDENT_TOLERANCE
                && (a.z - b.z).abs() <= COINCIDENT_TOLERANCE;
            if !near {
                continue;
            }
            coincidents += 1;
            if coincidents <= COINCIDENT_REPORT_LIMIT {
                items.push(CheckItem::warning(
                    CheckCategory::Geometry,
                    format!("Joint {first}"),
                    format!("Joint {first} and joint {second} share the same coordinates; merge them or confirm the duplication is intended."),
                ));
            }
        }
    }
    if coincidents > COINCIDENT_REPORT_LIMIT {
        items.push(CheckItem::warning(
            CheckCategory::Geometry,
            "Model",
            format!("{} more coincident joint pairs are not listed.", coincidents - COINCIDENT_REPORT_LIMIT),
        ));
    }

    let mut element_ids: Vec<usize> = model.elements.keys().copied().collect();
    element_ids.sort_unstable();
    for id in element_ids {
        let Some(element) = model.elements.get(&id) else { continue };
        let (Some(a), Some(b)) = (model.nodes.get(&element.node_i), model.nodes.get(&element.node_j)) else {
            continue;
        };
        let length = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2) + (b.z - a.z).powi(2)).sqrt();
        if length <= MIN_MEMBER_LENGTH {
            items.push(CheckItem::error(
                CheckCategory::Geometry,
                format!("Frame {id}"),
                format!("Frame {id} has zero length; its stiffness cannot be formed."),
            ));
        }
    }

    let mut area_ids: Vec<usize> = model.areas.keys().copied().collect();
    area_ids.sort_unstable();
    for id in area_ids {
        let Some(area) = model.areas.get(&id) else { continue };
        if area.node_ids.len() < 3 {
            items.push(CheckItem::error(
                CheckCategory::Geometry,
                format!("Area {id}"),
                format!("Area {id} has {} corner joint(s); at least three are needed.", area.node_ids.len()),
            ));
        }
    }

    // Joints must be attached to something and should sit on a defined level.
    let mut used: BTreeMap<usize, usize> = BTreeMap::new();
    for element in model.elements.values() {
        *used.entry(element.node_i).or_default() += 1;
        *used.entry(element.node_j).or_default() += 1;
    }
    for area in model.areas.values() {
        for node_id in &area.node_ids {
            *used.entry(*node_id).or_default() += 1;
        }
    }
    for &node_id in &ids {
        let Some(node) = model.nodes.get(&node_id) else { continue };
        if !used.contains_key(&node_id) {
            items.push(CheckItem::warning(
                CheckCategory::Geometry,
                format!("Joint {node_id}"),
                format!("Joint {node_id} is not connected to any frame or area; it carries no stiffness."),
            ));
        }
        let on_story = model
            .stories
            .iter()
            .any(|story| (story.elevation - node.z).abs() <= STORY_TOLERANCE);
        if !on_story && !model.stories.is_empty() {
            items.push(CheckItem::info(
                CheckCategory::Geometry,
                format!("Joint {node_id}"),
                format!("Joint {node_id} sits at z = {:.3} m, which is not one of the defined levels.", node.z),
            ));
        }
    }

    for (index, story) in model.stories.iter().enumerate() {
        let duplicates = model
            .stories
            .iter()
            .skip(index + 1)
            .filter(|other| (other.elevation - story.elevation).abs() <= STORY_TOLERANCE)
            .count();
        if duplicates > 0 {
            items.push(CheckItem::warning(
                CheckCategory::Geometry,
                format!("Story {}", story.name),
                format!("Story {} shares its level ({:.3} m) with another story.", story.name, story.elevation),
            ));
        }
        let joints = model
            .nodes
            .values()
            .filter(|node| (node.z - story.elevation).abs() <= STORY_TOLERANCE)
            .count();
        if joints == 0 {
            items.push(CheckItem::info(
                CheckCategory::Geometry,
                format!("Story {}", story.name),
                format!("Story {} (z = {:.3} m) has no joints.", story.name, story.elevation),
            ));
        }
    }
}

/// Missing references and unusable properties.
fn check_properties(model: &Model, items: &mut Vec<CheckItem>) {
    let mut element_ids: Vec<usize> = model.elements.keys().copied().collect();
    element_ids.sort_unstable();
    for id in element_ids {
        let Some(element) = model.elements.get(&id) else { continue };
        for (label, node_id) in [("start", element.node_i), ("end", element.node_j)] {
            if !model.nodes.contains_key(&node_id) {
                items.push(CheckItem::error(
                    CheckCategory::Properties,
                    format!("Frame {id}"),
                    format!("Frame {id} refers to missing joint {node_id} at its {label}."),
                ));
            }
        }
        if !model.materials.contains_key(&element.material_id) {
            items.push(CheckItem::error(
                CheckCategory::Properties,
                format!("Frame {id}"),
                format!("Frame {id} uses material {}, which is not defined.", element.material_id),
            ));
        }
        match model.sections.get(&element.section_id) {
            None => items.push(CheckItem::error(
                CheckCategory::Properties,
                format!("Frame {id}"),
                format!("Frame {id} uses section {}, which is not defined.", element.section_id),
            )),
            Some(section) if section.area <= 0.0 || section.iy <= 0.0 || section.iz <= 0.0 => {
                items.push(CheckItem::error(
                    CheckCategory::Properties,
                    format!("Frame {id}"),
                    format!("Section '{}' has non-positive area or inertia.", section.name),
                ));
            }
            Some(_) => {}
        }
    }

    let mut area_ids: Vec<usize> = model.areas.keys().copied().collect();
    area_ids.sort_unstable();
    for id in area_ids {
        let Some(area) = model.areas.get(&id) else { continue };
        for node_id in &area.node_ids {
            if !model.nodes.contains_key(node_id) {
                items.push(CheckItem::error(
                    CheckCategory::Properties,
                    format!("Area {id}"),
                    format!("Area {id} refers to missing joint {node_id}."),
                ));
            }
        }
        if !model.materials.contains_key(&area.material_id) {
            items.push(CheckItem::error(
                CheckCategory::Properties,
                format!("Area {id}"),
                format!("Area {id} uses material {}, which is not defined.", area.material_id),
            ));
        }
        if area.thickness <= 0.0 {
            items.push(CheckItem::error(
                CheckCategory::Properties,
                format!("Area {id}"),
                format!("Area {id} has a thickness of {:.4} m.", area.thickness),
            ));
        }
    }

    for (id, material) in &model.materials {
        if material.e <= 0.0 {
            items.push(CheckItem::error(
                CheckCategory::Properties,
                format!("Material {id}"),
                format!("Material '{}' has a non-positive modulus of elasticity.", material.name),
            ));
        }
    }
}

/// Empty cases, dangling load targets and broken combinations.
fn check_loading(model: &Model, items: &mut Vec<CheckItem>) {
    if model.load_cases.is_empty() {
        items.push(CheckItem::info(
            CheckCategory::Loading,
            "Load cases",
            "No load cases are defined; the analysis solves the default self-weight case.",
        ));
        return;
    }

    let case_ids: Vec<usize> = model.load_cases.iter().map(|case| case.id).collect();
    for case in &model.load_cases {
        let load_count = case.nodal_loads.len() + case.distributed_loads.len();
        if load_count == 0 && case.self_weight_multiplier == 0.0 {
            items.push(CheckItem::warning(
                CheckCategory::Loading,
                format!("Case {}", case.name),
                format!("Case '{}' has no loads and no self weight, so the run returns zeros.", case.name),
            ));
        }
        for load in &case.nodal_loads {
            if !model.nodes.contains_key(&load.node_id) {
                items.push(CheckItem::error(
                    CheckCategory::Loading,
                    format!("Case {}", case.name),
                    format!("Case '{}' loads missing joint {}.", case.name, load.node_id),
                ));
            }
        }
        for load in &case.distributed_loads {
            if !model.elements.contains_key(&load.element_id) {
                items.push(CheckItem::error(
                    CheckCategory::Loading,
                    format!("Case {}", case.name),
                    format!("Case '{}' loads missing frame {}.", case.name, load.element_id),
                ));
            }
        }
    }

    let check_case_id = |case_id: Option<usize>, object: &str, items: &mut Vec<CheckItem>| {
        if let Some(case_id) = case_id {
            if !case_ids.contains(&case_id) {
                items.push(CheckItem::warning(
                    CheckCategory::Loading,
                    object.to_string(),
                    format!("{object} refers to case {case_id}, which is not defined."),
                ));
            }
        }
    };

    for load in &model.frame_loads {
        if !model.elements.contains_key(&load.element_id) {
            items.push(CheckItem::error(
                CheckCategory::Loading,
                "Frame loads",
                format!("A frame load targets missing frame {}.", load.element_id),
            ));
        }
        check_case_id(load.case_id, "Frame loads", items);
    }
    for load in &model.area_loads {
        if !model.areas.contains_key(&load.area_id) {
            items.push(CheckItem::error(
                CheckCategory::Loading,
                "Area loads",
                format!("An area load targets missing area {}.", load.area_id),
            ));
        }
        check_case_id(load.case_id, "Area loads", items);
    }
    for load in &model.temperature_loads {
        if !model.elements.contains_key(&load.element_id) {
            items.push(CheckItem::error(
                CheckCategory::Loading,
                "Temperature loads",
                format!("A temperature load targets missing frame {}.", load.element_id),
            ));
        }
        check_case_id(load.case_id, "Temperature loads", items);
    }

    for combination in &model.load_combinations {
        if combination.factors.is_empty() {
            items.push(CheckItem::warning(
                CheckCategory::Loading,
                format!("Combo {}", combination.name),
                format!("Combination '{}' has no component cases.", combination.name),
            ));
        }
        for (case_id, _) in &combination.factors {
            if !case_ids.contains(case_id) {
                items.push(CheckItem::error(
                    CheckCategory::Loading,
                    format!("Combo {}", combination.name),
                    format!("Combination '{}' refers to case {case_id}, which is not defined.", combination.name),
                ));
            }
        }
    }
}

/// Whether the model can move at all.
fn check_stability(model: &Model, items: &mut Vec<CheckItem>) {
    if model.nodes.is_empty() {
        return;
    }
    let supported = model.nodes.values().filter(|node| node.has_support()).count();
    if supported == 0 {
        items.push(CheckItem::error(
            CheckCategory::Stability,
            "Model",
            "No joint is restrained or sprung, so the model is free to move and the stiffness matrix is singular.",
        ));
    } else {
        let free = model.nodes.len() - supported;
        if free > 0 {
            items.push(CheckItem::info(
                CheckCategory::Stability,
                "Model",
                format!("{supported} of {} joints have a support; the remaining {free} are carried by the frames.", model.nodes.len()),
            ));
        }
    }

    // A frame with both ends free in torsion and no joints on the same level is
    // a classic singular mode; report it before the solver does.
    let mut torsional: Vec<usize> = Vec::new();
    for (id, element) in &model.elements {
        let (Some(a), Some(b)) = (model.nodes.get(&element.node_i), model.nodes.get(&element.node_j)) else {
            continue;
        };
        let both_free = !a.restrained[3] && !b.restrained[3] && a.springs[3] == 0.0 && b.springs[3] == 0.0;
        if both_free && a.y == b.y && a.z == b.z {
            torsional.push(*id);
        }
    }
    torsional.sort_unstable();
    for id in torsional.iter().take(COINCIDENT_REPORT_LIMIT) {
        items.push(CheckItem::warning(
            CheckCategory::Stability,
            format!("Frame {id}"),
            format!("Frame {id} has no torsional restraint at either end; add one to avoid a singular RX mode."),
        ));
    }
}

/// Diaphragm assignments.
fn check_diaphragms(model: &Model, items: &mut Vec<CheckItem>) {
    if model.diaphragms.is_empty() && model.nodes.values().all(|node| node.diaphragm.is_none()) {
        return;
    }

    for diaphragm in &model.diaphragms {
        let joints: Vec<&crate::model::Node> = model
            .nodes
            .values()
            .filter(|node| node.diaphragm == Some(diaphragm.id))
            .collect();
        if joints.is_empty() {
            items.push(CheckItem::info(
                CheckCategory::Diaphragms,
                format!("Diaphragm {}", diaphragm.name),
                format!("Diaphragm '{}' has no joints assigned.", diaphragm.name),
            ));
            continue;
        }
        if joints.len() == 1 {
            items.push(CheckItem::info(
                CheckCategory::Diaphragms,
                format!("Diaphragm {}", diaphragm.name),
                format!("Diaphragm '{}' ties a single joint, so it changes nothing.", diaphragm.name),
            ));
        }
        for node in joints {
            if !diaphragm.contains_level(node.z, STORY_TOLERANCE) {
                items.push(CheckItem::warning(
                    CheckCategory::Diaphragms,
                    format!("Joint {}", node.id),
                    format!(
                        "Joint {} is at z = {:.3} m but diaphragm '{}' is at {:.3} m.",
                        node.id, node.z, diaphragm.name, diaphragm.story
                    ),
                ));
            }
        }
    }

    let known: Vec<usize> = model.diaphragms.iter().map(|diaphragm| diaphragm.id).collect();
    let mut ids: Vec<usize> = model.nodes.keys().copied().collect();
    ids.sort_unstable();
    for id in ids {
        let Some(node) = model.nodes.get(&id) else { continue };
        if let Some(diaphragm_id) = node.diaphragm {
            if !known.contains(&diaphragm_id) {
                items.push(CheckItem::error(
                    CheckCategory::Diaphragms,
                    format!("Joint {id}"),
                    format!("Joint {id} is assigned to diaphragm {diaphragm_id}, which is not defined."),
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elements::Element;
    use crate::materials::Material;
    use crate::model::{Diaphragm, Story};
    use crate::sections::Section;

    fn beam_model() -> Model {
        let mut model = Model::new("check test");
        let material = model.add_material(Material::steel());
        let section = model.add_section(Section::rectangular("R", 0.3, 0.5));
        let a = model.add_node(0.0, 0.0, 0.0);
        let b = model.add_node(0.0, 0.0, 3.0);
        model.add_element(Element::frame(1, a, b, material, section));
        model.nodes.get_mut(&a).map(|node| node.restrained = [true; 6]);
        model.nodes.get_mut(&b).map(|node| node.restrained[3] = true);
        model
    }

    #[test]
    fn an_empty_model_reports_a_blocking_error() {
        let report = check_model(&Model::new("empty"));
        assert!(!report.can_analyse());
        assert!(report.errors() >= 1);
        assert!(report.items.iter().any(|item| item.category == CheckCategory::Model));
    }

    #[test]
    fn a_sound_cantilever_has_no_errors() {
        let report = check_model(&beam_model());
        assert!(report.can_analyse(), "unexpected errors: {:?}", report.items);
    }

    #[test]
    fn coincident_joints_are_reported_once_per_pair() {
        let mut model = beam_model();
        let extra = model.add_node(0.0, 0.0, 0.0);
        let report = check_model(&model);
        let hits: Vec<&CheckItem> = report
            .items
            .iter()
            .filter(|item| item.severity == CheckSeverity::Warning && item.message.contains("share the same coordinates"))
            .collect();
        assert_eq!(hits.len(), 1);
        assert!(hits[0].message.contains(&format!("joint {extra}")) || hits[0].message.contains("Joint"));
    }

    #[test]
    fn missing_section_and_zero_length_members_are_errors() {
        let mut model = Model::new("broken");
        let material = model.add_material(Material::steel());
        let a = model.add_node(0.0, 0.0, 0.0);
        let b = model.add_node(0.0, 0.0, 0.0);
        model.add_element(Element::frame(1, a, b, material, 99));
        model.nodes.get_mut(&a).map(|node| node.restrained = [true; 6]);
        let report = check_model(&model);
        assert!(report.items.iter().any(|item| item.message.contains("zero length")));
        assert!(report.items.iter().any(|item| item.message.contains("section 99")));
        assert!(!report.can_analyse());
    }

    #[test]
    fn an_unrestrained_model_is_a_stability_error() {
        let mut model = beam_model();
        for node in model.nodes.values_mut() {
            node.restrained = [false; 6];
        }
        let report = check_model(&model);
        assert!(report
            .items
            .iter()
            .any(|item| item.category == CheckCategory::Stability && item.severity == CheckSeverity::Error));
    }

    #[test]
    fn diaphragms_flag_joints_on_the_wrong_level() {
        let mut model = beam_model();
        model.stories = vec![Story::new("Story1", 0.0), Story::new("Story2", 3.0)];
        model.diaphragms.push(Diaphragm::new(1, "D1", 3.0));
        if let Some(node) = model.nodes.get_mut(&1) {
            node.diaphragm = Some(1);
        }
        let report = check_model(&model);
        assert!(report
            .items
            .iter()
            .any(|item| item.category == CheckCategory::Diaphragms && item.message.contains("but diaphragm")));
    }

    #[test]
    fn unknown_diaphragm_references_are_errors() {
        let mut model = beam_model();
        if let Some(node) = model.nodes.get_mut(&2) {
            node.diaphragm = Some(7);
        }
        let report = check_model(&model);
        assert!(report
            .items
            .iter()
            .any(|item| item.severity == CheckSeverity::Error && item.message.contains("diaphragm 7")));
    }

    #[test]
    fn combinations_must_reference_defined_cases() {
        let mut model = beam_model();
        model.load_cases.push(crate::loads::LoadCase {
            id: 1,
            name: "DEAD".to_string(),
            ..Default::default()
        });
        model.load_combinations.push(crate::loads::LoadCombination {
            id: 1,
            name: "COMB1".to_string(),
            equation: "1.2DEAD".to_string(),
            combo_type: crate::loads::ComboType::LinearAdd,
            factors: vec![(5, 1.2)],
        });
        let report = check_model(&model);
        assert!(report
            .items
            .iter()
            .any(|item| item.severity == CheckSeverity::Error && item.message.contains("case 5")));
    }
}
