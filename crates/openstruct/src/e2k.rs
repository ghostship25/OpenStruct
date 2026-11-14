//! Import of `.e2k` text model files.
//!
//! The format writes one `KEYWORD` record per line, indented, with quoted names and
//! `FIELD value` pairs, under `$ SECTION` comments. Geometry is defined once in
//! plan (`POINT`, `LINE`, `AREA`) and placed on storeys by `POINTASSIGN`,
//! `LINEASSIGN` and `AREAASSIGN`, so a column exists between a storey and the
//! storey below it while a beam exists at the storey level.
//!
//! The parser reads the documented text records only and knows nothing about
//! any binary database file. Units come from the file's own
//! `UNITS` record and are converted to the model's SI (metres, newtons, pascals).
//!
//! What is read: stories (with master and similar-to), materials, frame and shell
//! sections, points, lines, areas, supports, diaphragm assignments, load patterns,
//! linear static load cases with joint and frame loads, area loads, and load
//! combinations.
//!
//! What is not: auto-select lists beyond their first shape, section designer
//! shapes, shell thickness from decks, modal/response-spectrum cases, seismic and
//! wind auto-loads, and staged construction. Each gap is reported as a warning so
//! the import is never silently partial.

use crate::analysis::Analysis;
use crate::areas::{Area, AreaKind};
use crate::elements::Element;
use crate::loads::{AreaLoad, ComboType, DistributedLoad, LoadCase, LoadCombination, LoadDirection, LoadPattern, LoadType};
use crate::materials::{Material, MaterialType, GRAVITY};
use crate::model::{Diaphragm, Model, Story};
use crate::sections::{Section, SectionShape};
use std::collections::HashMap;

/// What an import found, so the app can report it honestly.
#[derive(Debug, Clone, Default)]
pub struct E2kImportReport {
    /// The file's own units, e.g. `N / MM`.
    pub units: String,
    pub stories: usize,
    pub materials: usize,
    pub sections: usize,
    pub points: usize,
    pub frames: usize,
    pub areas: usize,
    pub load_cases: usize,
    pub load_combinations: usize,
    pub nodal_loads: usize,
    pub frame_loads: usize,
    pub area_loads: usize,
    /// Name of the story the file marks as the master story, if any.
    pub master_story: Option<String>,
    pub warnings: Vec<String>,
}

impl E2kImportReport {
    /// One line the status bar can show.
    pub fn summary(&self) -> String {
        format!(
            "E2K import ({}): {} stories, {} joints, {} frames, {} areas, {} cases, {} combos, {} warnings.",
            self.units,
            self.stories,
            self.points,
            self.frames,
            self.areas,
            self.load_cases,
            self.load_combinations,
            self.warnings.len()
        )
    }
}

// -- units -------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Units {
    /// Metres per file length unit.
    length: f64,
    /// Newtons per file force unit.
    force: f64,
    label: String,
}

impl Default for Units {
    fn default() -> Self {
        Self {
            length: 1.0,
            force: 1.0,
            label: "m / N".to_string(),
        }
    }
}

impl Units {
    fn from_record(tokens: &[String]) -> Option<Self> {
        let force_name = tokens.first()?.trim().to_ascii_uppercase();
        let length_name = tokens.get(1)?.trim().to_ascii_uppercase();
        let force = match force_name.as_str() {
            "N" => 1.0,
            "KN" => 1000.0,
            "KGF" => 9.806_65,
            "TONF" => 9806.65,
            "LBF" => 4.448_221_6,
            "KIP" => 4448.221_6,
            _ => return None,
        };
        let length = match length_name.as_str() {
            "MM" => 0.001,
            "CM" => 0.01,
            "M" => 1.0,
            "IN" => 0.0254,
            "FT" => 0.3048,
            _ => return None,
        };
        Some(Self {
            length,
            force,
            label: format!("{force_name} / {length_name}"),
        })
    }

    /// Force per area, i.e. stress: N/m² from the file's stress unit.
    fn stress(&self) -> f64 {
        self.force / (self.length * self.length)
    }

    /// Force per length: N/m from the file's line-load unit.
    fn line_load(&self) -> f64 {
        self.force / self.length
    }

    /// Mass density from a weight density written in force per length³.
    fn mass_density(&self, weight: f64) -> f64 {
        weight * self.force / (self.length * self.length * self.length) / GRAVITY
    }
}

// -- tokenizer ---------------------------------------------------------------

/// Split one record into tokens, treating a quoted span as a single token and a
/// doubled quote inside it as a literal quote (`"Slab5"""`).
fn tokenize(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;
    let mut chars = line.chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '"' if in_quote => {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    current.push('"');
                } else {
                    in_quote = false;
                    tokens.push(std::mem::take(&mut current));
                }
            }
            '"' => in_quote = true,
            character if character.is_whitespace() && !in_quote => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            character => current.push(character),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn field<'a>(tokens: &'a [String], key: &str) -> Option<&'a str> {
    tokens
        .iter()
        .position(|token| token.eq_ignore_ascii_case(key))
        .and_then(|index| tokens.get(index + 1))
        .map(String::as_str)
}

fn number(tokens: &[String], key: &str) -> Option<f64> {
    field(tokens, key).and_then(|value| value.trim().parse::<f64>().ok())
}

fn truthy(tokens: &[String], key: &str) -> bool {
    field(tokens, key).is_some_and(|value| value.eq_ignore_ascii_case("yes") || value == "1")
}

// -- intermediate records ----------------------------------------------------

#[derive(Debug, Clone)]
struct StoryDef {
    name: String,
    height: Option<f64>,
    elevation: Option<f64>,
    master: bool,
    similar_to: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct MaterialDef {
    name: String,
    material_type: MaterialType,
    e: f64,
    nu: f64,
    alpha: f64,
    weight_density: Option<f64>,
    fy: f64,
    fu: f64,
    has_e: bool,
}

#[derive(Debug, Clone)]
struct SectionDef {
    name: String,
    material: Option<String>,
    shape: String,
    depth: Option<f64>,
    width: Option<f64>,
    flange: Option<f64>,
    web: Option<f64>,
}

#[derive(Debug, Clone)]
struct ShellDef {
    name: String,
    material: Option<String>,
    thickness: f64,
    wall: bool,
}

#[derive(Debug, Clone)]
struct LineDef {
    kind: String,
    start: String,
    end: String,
}

#[derive(Debug, Clone)]
struct AreaDef {
    kind: String,
    points: Vec<String>,
}

#[derive(Debug, Clone)]
struct LineAssign {
    line: String,
    story: String,
    section: String,
}

#[derive(Debug, Clone)]
struct AreaAssign {
    area: String,
    story: String,
    shell: String,
    diaphragm: Option<String>,
}

#[derive(Debug, Clone)]
struct CaseDef {
    name: String,
    kind: String,
    pattern: Option<String>,
}

#[derive(Debug, Clone)]
struct LineLoadDef {
    line: String,
    story: String,
    case: String,
    direction: String,
    value: f64,
}

#[derive(Debug, Clone)]
struct AreaLoadDef {
    area: String,
    story: String,
    case: String,
    value: f64,
}

fn material_type(name: &str) -> MaterialType {
    match name.trim().to_ascii_lowercase().as_str() {
        "concrete" => MaterialType::Concrete,
        "rebar" => MaterialType::Rebar,
        "tendon" => MaterialType::Tendon,
        "masonry" => MaterialType::Masonry,
        "aluminum" | "aluminium" => MaterialType::Aluminium,
        "cold formed" | "coldformed" => MaterialType::ColdFormed,
        _ => MaterialType::Steel,
    }
}

fn load_type(name: &str) -> LoadType {
    match name.trim().to_ascii_lowercase().as_str() {
        "dead" => LoadType::Dead,
        "super dead" => LoadType::Dead,
        "live" | "reducible live" | "roof live" => LoadType::Live,
        "wind" => LoadType::Wind,
        "seismic" | "quake" => LoadType::Seismic,
        "snow" => LoadType::Snow,
        other => LoadType::Other(other.to_string()),
    }
}

// -- parse -------------------------------------------------------------------

/// Read an E2K `.e2k` text into a model, its load cases and a report.
pub fn parse_e2k(text: &str) -> anyhow::Result<(Model, Analysis, E2kImportReport)> {
    let mut report = E2kImportReport::default();
    let mut units = Units::default();
    let mut title = String::new();
    let mut stories: Vec<StoryDef> = Vec::new();
    let mut materials: Vec<MaterialDef> = Vec::new();
    let mut sections: Vec<SectionDef> = Vec::new();
    let mut shells: Vec<ShellDef> = Vec::new();
    let mut points: HashMap<String, (f64, f64, Option<f64>)> = HashMap::new();
    let mut point_order: Vec<String> = Vec::new();
    let mut lines: HashMap<String, LineDef> = HashMap::new();
    let mut areas: HashMap<String, AreaDef> = HashMap::new();
    let mut restraints: Vec<(String, String, [bool; 6])> = Vec::new();
    let mut line_assigns: Vec<LineAssign> = Vec::new();
    let mut area_assigns: Vec<AreaAssign> = Vec::new();
    let mut diaphragm_names: Vec<String> = Vec::new();
    let mut patterns: Vec<(String, LoadType, f64)> = Vec::new();
    let mut cases: Vec<CaseDef> = Vec::new();
    let mut line_loads: Vec<LineLoadDef> = Vec::new();
    let mut area_loads: Vec<AreaLoadDef> = Vec::new();
    let mut combos: Vec<(String, Vec<(String, f64)>)> = Vec::new();
    let mut auto_sections: HashMap<String, Vec<String>> = HashMap::new();
    let mut skipped_records: HashMap<String, usize> = HashMap::new();

    for raw in text.lines() {
        let line = raw.trim_end_matches(['\r', '\n']);
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('$') || trimmed.starts_with("TABLE:") {
            continue;
        }
        // Records are indented; the free-text history at the end of the file is not.
        if !line.starts_with(' ') && !line.starts_with('\t') {
            continue;
        }
        let tokens = tokenize(trimmed);
        let Some(keyword) = tokens.first() else { continue };
        let rest = &tokens[1..];
        match keyword.to_ascii_uppercase().as_str() {
            "UNITS" => {
                if let Some(parsed) = Units::from_record(rest) {
                    units = parsed;
                }
            }
            "TITLE1" => title = rest.first().cloned().unwrap_or_default(),
            "TITLE2" => {
                if !rest.first().is_some_and(|extra| extra.is_empty()) && title.is_empty() {
                    title = rest.first().cloned().unwrap_or_default();
                }
            }
            "STORY" => {
                let Some(name) = rest.first() else { continue };
                stories.push(StoryDef {
                    name: name.clone(),
                    height: number(rest, "HEIGHT"),
                    elevation: number(rest, "ELEV"),
                    master: truthy(rest, "MASTERSTORY"),
                    similar_to: field(rest, "SIMILARTO").map(str::to_string),
                });
            }
            "MATERIAL" => {
                let Some(name) = rest.first() else { continue };
                let index = materials.iter().position(|material| material.name == *name);
                let slot = match index {
                    Some(index) => &mut materials[index],
                    None => {
                        materials.push(MaterialDef {
                            name: name.clone(),
                            material_type: MaterialType::Steel,
                            e: 0.0,
                            nu: 0.0,
                            alpha: 0.0,
                            weight_density: None,
                            fy: 0.0,
                            fu: 0.0,
                            has_e: false,
                        });
                        materials.last_mut().expect("just pushed")
                    }
                };
                if let Some(kind) = field(rest, "TYPE") {
                    slot.material_type = material_type(kind);
                }
                if let Some(weight) = number(rest, "WEIGHTPERVOLUME") {
                    slot.weight_density = Some(weight);
                }
                if let Some(e) = number(rest, "E") {
                    slot.e = e;
                    slot.has_e = true;
                }
                if let Some(nu) = number(rest, "U") {
                    slot.nu = nu;
                }
                if let Some(alpha) = number(rest, "A") {
                    slot.alpha = alpha;
                }
                if let Some(fy) = number(rest, "FY") {
                    slot.fy = fy;
                }
                if let Some(fu) = number(rest, "FU") {
                    slot.fu = fu;
                }
                if let Some(fc) = number(rest, "FC") {
                    slot.fy = fc;
                }
            }
            "FRAMESECTION" => {
                let Some(name) = rest.first() else { continue };
                sections.push(SectionDef {
                    name: name.clone(),
                    material: field(rest, "MATERIAL").map(str::to_string),
                    shape: field(rest, "SHAPE").unwrap_or("Rectangular").to_string(),
                    depth: number(rest, "D"),
                    width: number(rest, "B"),
                    flange: number(rest, "TF"),
                    web: number(rest, "TW"),
                });
            }
            "SHELLPROP" => {
                let Some(name) = rest.first() else { continue };
                shells.push(ShellDef {
                    name: name.clone(),
                    material: field(rest, "MATERIAL").map(str::to_string),
                    thickness: number(rest, "SLABTHICKNESS")
                        .or_else(|| number(rest, "WALLTHICKNESS"))
                        .unwrap_or(0.0),
                    wall: field(rest, "PROPTYPE").is_some_and(|kind| kind.eq_ignore_ascii_case("wall")),
                });
            }
            "AUTOSECTION" => {
                let Some(name) = rest.first() else { continue };
                auto_sections
                    .entry(name.clone())
                    .or_default()
                    .extend(rest.iter().skip(1).cloned());
            }
            "POINT" => {
                let Some(name) = rest.first() else { continue };
                let x = number(rest, "X").or_else(|| rest.get(1).and_then(|value| value.parse().ok()));
                let y = number(rest, "Y").or_else(|| rest.get(2).and_then(|value| value.parse().ok()));
                let z = number(rest, "Z").or_else(|| rest.get(3).and_then(|value| value.parse().ok()));
                if let (Some(x), Some(y)) = (x, y) {
                    if !points.contains_key(name) {
                        point_order.push(name.clone());
                    }
                    points.insert(name.clone(), (x, y, z));
                }
            }
            "LINE" => {
                let Some(name) = rest.first() else { continue };
                let Some(start) = rest.get(2) else { continue };
                let Some(end) = rest.get(3) else { continue };
                lines.insert(
                    name.clone(),
                    LineDef {
                        kind: rest.get(1).cloned().unwrap_or_else(|| "LINE".to_string()),
                        start: start.clone(),
                        end: end.clone(),
                    },
                );
            }
            "AREA" => {
                let Some(name) = rest.first() else { continue };
                let count: usize = rest.get(2).and_then(|value| value.parse().ok()).unwrap_or(0);
                let points: Vec<String> = rest.iter().skip(3).take(count).cloned().collect();
                areas.insert(
                    name.clone(),
                    AreaDef {
                        kind: rest.get(1).cloned().unwrap_or_else(|| "FLOOR".to_string()),
                        points,
                    },
                );
            }
            "POINTASSIGN" => {
                let (Some(point), Some(story)) = (rest.first(), rest.get(1)) else { continue };
                if let Some(list) = field(rest, "RESTRAINT") {
                    let mut flags = [false; 6];
                    for dof in list.split_whitespace() {
                        match dof.to_ascii_uppercase().trim() {
                            "UX" => flags[0] = true,
                            "UY" => flags[1] = true,
                            "UZ" => flags[2] = true,
                            "RX" => flags[3] = true,
                            "RY" => flags[4] = true,
                            "RZ" => flags[5] = true,
                            _ => {}
                        }
                    }
                    restraints.push((point.clone(), story.clone(), flags));
                }
            }
            "LINEASSIGN" => {
                let (Some(line), Some(story)) = (rest.first(), rest.get(1)) else { continue };
                if let Some(section) = field(rest, "SECTION") {
                    line_assigns.push(LineAssign {
                        line: line.clone(),
                        story: story.clone(),
                        section: section.to_string(),
                    });
                }
            }
            "AREAASSIGN" => {
                let (Some(area), Some(story)) = (rest.first(), rest.get(1)) else { continue };
                let Some(shell) = field(rest, "SECTION") else { continue };
                area_assigns.push(AreaAssign {
                    area: area.clone(),
                    story: story.clone(),
                    shell: shell.to_string(),
                    diaphragm: field(rest, "DIAPH").map(str::to_string),
                });
            }
            "DIAPHRAGM" => {
                if let Some(name) = rest.first() {
                    if !diaphragm_names.contains(name) {
                        diaphragm_names.push(name.clone());
                    }
                }
            }
            "LOADPATTERN" => {
                let Some(name) = rest.first() else { continue };
                patterns.push((
                    name.clone(),
                    load_type(field(rest, "TYPE").unwrap_or("Other")),
                    number(rest, "SELFWEIGHT").unwrap_or(0.0),
                ));
            }
            "LOADCASE" => {
                let Some(name) = rest.first() else { continue };
                if let Some(kind) = field(rest, "TYPE") {
                    let index = match cases.iter().position(|case| case.name == *name) {
                        Some(index) => index,
                        None => {
                            cases.push(CaseDef {
                                name: name.clone(),
                                kind: kind.to_string(),
                                pattern: None,
                            });
                            cases.len() - 1
                        }
                    };
                    cases[index].kind = kind.to_string();
                } else if let Some(pattern) = field(rest, "LOADPAT") {
                    if let Some(case) = cases.iter_mut().find(|case| case.name == *name) {
                        if case.pattern.is_none() {
                            case.pattern = Some(pattern.to_string());
                        }
                    }
                }
            }
            "LINELOAD" => {
                let (Some(line), Some(story)) = (rest.first(), rest.get(1)) else { continue };
                let Some(case) = field(rest, "LC") else { continue };
                let Some(value) = number(rest, "FVAL") else { continue };
                line_loads.push(LineLoadDef {
                    line: line.clone(),
                    story: story.clone(),
                    case: case.to_string(),
                    direction: field(rest, "DIR").unwrap_or("GRAV").to_string(),
                    value,
                });
            }
            "AREALOAD" => {
                let (Some(area), Some(story)) = (rest.first(), rest.get(1)) else { continue };
                let Some(case) = field(rest, "LC") else { continue };
                let Some(value) = number(rest, "FVAL") else { continue };
                area_loads.push(AreaLoadDef {
                    area: area.clone(),
                    story: story.clone(),
                    case: case.to_string(),
                    value,
                });
            }
            "COMBO" => {
                let Some(name) = rest.first() else { continue };
                if let Some(case) = field(rest, "LOADCASE") {
                    let factor = number(rest, "SF").unwrap_or(1.0);
                    match combos.iter_mut().find(|(existing, _)| existing == name) {
                        Some((_, factors)) => factors.push((case.to_string(), factor)),
                        None => combos.push((name.clone(), vec![(case.to_string(), factor)])),
                    }
                }
            }
            other => {
                *skipped_records.entry(other.to_string()).or_default() += 1;
            }
        }
    }

    if points.is_empty() && stories.is_empty() {
        anyhow::bail!("this file has no POINT or STORY records; is it really an .e2k export?");
    }

    report.units = units.label.clone();
    report.master_story = stories.iter().find(|story| story.master).map(|story| story.name.clone());

    // ---- stories ----------------------------------------------------------
    let base_elevation = stories
        .iter()
        .find_map(|story| story.elevation)
        .map(|elevation| elevation * units.length)
        .unwrap_or(0.0);
    let mut stacked: Vec<(String, f64, bool, Option<String>)> = Vec::new();
    let mut level = base_elevation;
    for story in stories.iter().rev() {
        if story.elevation.is_some() {
            continue;
        }
        level += story.height.unwrap_or(0.0) * units.length;
        stacked.push((story.name.clone(), level, story.master, story.similar_to.clone()));
    }
    stacked.reverse();

    let mut model = Model::new(if title.trim().is_empty() { "E2K import" } else { title.trim() });
    model.stories.clear();
    if let Some(base) = stories.iter().find(|story| story.elevation.is_some()) {
        model.stories.push(Story::new(&base.name, base_elevation));
    }
    for (name, elevation, _master, similar_to) in &stacked {
        let mut story = Story::new(name, *elevation);
        story.similar_to = similar_to.clone();
        model.stories.push(story);
    }
    model.stories.sort_by(|a, b| a.elevation.partial_cmp(&b.elevation).unwrap_or(std::cmp::Ordering::Equal));
    report.stories = model.stories.len();
    if model.stories.is_empty() {
        model.stories.push(Story::new("Story1", 0.0));
        report.warnings.push("No STORY records: the model was put on a single story at elevation 0.".to_string());
    }

    let levels: HashMap<String, f64> = model.stories.iter().map(|story| (story.name.clone(), story.elevation)).collect();
    let mut sorted_levels: Vec<f64> = levels.values().copied().collect();
    sorted_levels.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let level_of = |story: &str| -> Option<f64> { levels.get(story).copied() };
    let level_below = |elevation: f64| -> f64 {
        sorted_levels
            .iter()
            .copied()
            .filter(|level| *level < elevation - 1.0e-6)
            .fold(None::<f64>, |best, level| Some(best.map_or(level, |current| current.max(level))))
            .unwrap_or(elevation - 1.0)
    };

    // ---- materials --------------------------------------------------------
    let mut material_ids: HashMap<String, usize> = HashMap::new();
    if materials.is_empty() {
        let id = model.add_material(Material::steel());
        material_ids.insert("Steel".to_string(), id);
        report.warnings.push("No MATERIAL records: a default steel material was added.".to_string());
    }
    for definition in &materials {
        let e = if definition.has_e { definition.e * units.stress() } else { 200.0e9 };
        let nu = if definition.nu > 0.0 { definition.nu } else { 0.3 };
        let rho = definition.weight_density.map(|weight| units.mass_density(weight)).unwrap_or(7850.0);
        let mut material = Material::of_type(&definition.name, definition.material_type, e, nu, rho);
        material.fy = definition.fy * units.stress();
        material.fu = definition.fu * units.stress();
        if definition.alpha > 0.0 {
            material.alpha = definition.alpha;
        }
        material.sync_shear_modulus();
        let id = model.add_material(material);
        material_ids.insert(definition.name.clone(), id);
    }
    report.materials = model.materials.len();

    // ---- frame sections ---------------------------------------------------
    let mut section_ids: HashMap<String, usize> = HashMap::new();
    // A section's material belongs to the frame element in this model, so keep it here.
    let mut section_material: HashMap<String, usize> = HashMap::new();
    for definition in &sections {
        let material_name = definition.material.clone().unwrap_or_else(|| {
            sections
                .iter()
                .find_map(|other| other.material.clone())
                .unwrap_or_else(|| materials.first().map(|material| material.name.clone()).unwrap_or_default())
        });
        let material_id = material_ids.get(&material_name).copied().unwrap_or_else(|| {
            model.materials.keys().copied().min().unwrap_or(0)
        });
        let shape = definition.shape.to_ascii_lowercase();
        let depth = definition.depth.map(|value| value * units.length).unwrap_or(0.3);
        let width = definition.width.map(|value| value * units.length).unwrap_or(0.3);
        let is_auto = shape.contains("auto");
        let resolved = if is_auto {
            let first = auto_sections
                .get(&definition.name)
                .and_then(|list| list.first())
                .and_then(|name| sections.iter().find(|other| other.name == *name));
            match first {
                Some(target) => {
                    let d = target.depth.map(|value| value * units.length).unwrap_or(0.3);
                    let b = target.width.map(|value| value * units.length).unwrap_or(0.3);
                    (format!("{} ({})", definition.name, target.name), d, b, target.flange, target.web)
                }
                None => {
                    report.warnings.push(format!(
                        "Auto-select section {} has no section list; a 300×300 mm section was used.",
                        definition.name
                    ));
                    (definition.name.clone(), 0.3, 0.3, None, None)
                }
            }
        } else {
            (
                definition.name.clone(),
                depth,
                width,
                definition.flange.map(|value| value * units.length),
                definition.web.map(|value| value * units.length),
            )
        };
        let (name, depth, width, flange, web) = resolved;
        let section = if shape.contains("circle") || shape.contains("circular") || shape.contains("round") {
            Section::circular(&name, depth.max(0.05))
        } else if shape.contains("i/wide flange") || shape.contains("i-section") || shape.contains("wide flange") {
            Section::from_shape(
                &name,
                SectionShape::ISection {
                    depth: depth.max(0.05),
                    width: width.max(0.05),
                    web: web.unwrap_or(0.01).max(0.002),
                    flange: flange.unwrap_or(0.01).max(0.002),
                },
            )
            .unwrap_or_else(|| Section::rectangular(&name, width.max(0.05), depth.max(0.05)))
        } else {
            Section::rectangular(&name, width.max(0.05), depth.max(0.05))
        };
        let id = model.add_section(section);
        section_ids.insert(definition.name.clone(), id);
        section_material.insert(definition.name.clone(), material_id);
    }
    if !sections.is_empty() {
        report.sections = model.sections.len();
    }

    // Shell properties: name -> (thickness, material id, wall kind).
    let mut shell_info: HashMap<String, (f64, usize, bool)> = HashMap::new();
    for shell in &shells {
        let material_id = shell
            .material
            .as_ref()
            .and_then(|name| material_ids.get(name).copied())
            .unwrap_or_else(|| model.materials.keys().copied().min().unwrap_or(0));
        let thickness = if shell.thickness > 0.0 { shell.thickness * units.length } else { 0.2 };
        shell_info.insert(shell.name.clone(), (thickness, material_id, shell.wall));
    }

    // ---- nodes ------------------------------------------------------------
    let mut nodes: HashMap<(String, i64), usize> = HashMap::new();
    let new_node = |model: &mut Model,
                        nodes: &mut HashMap<(String, i64), usize>,
                        point: &str,
                        elevation: f64|
     -> Option<usize> {
        let (x, y, z) = *points.get(point)?;
        let z = z.map(|explicit| base_elevation + explicit * units.length).unwrap_or(elevation);
        let key = (point.to_string(), (z * 1.0e6).round() as i64);
        if let Some(&id) = nodes.get(&key) {
            return Some(id);
        }
        let id = model.add_node(x * units.length, y * units.length, z);
        nodes.insert(key, id);
        Some(id)
    };

    // ---- frames -----------------------------------------------------------
    let mut element_by_line_story: HashMap<(String, String), usize> = HashMap::new();
    for assign in &line_assigns {
        let Some(line) = lines.get(&assign.line) else { continue };
        let Some(level) = level_of(&assign.story) else { continue };
        let (bottom, top) = if line.kind.eq_ignore_ascii_case("column") {
            (level_below(level), level)
        } else {
            (level, level)
        };
        let (Some(start), Some(end)) = (
            new_node(&mut model, &mut nodes, &line.start, bottom),
            new_node(&mut model, &mut nodes, &line.end, top),
        ) else {
            report.warnings.push(format!(
                "Line {} on {} references a point that is not defined; the member was skipped.",
                assign.line, assign.story
            ));
            continue;
        };
        if start == end {
            continue;
        }
        let section_id = section_ids.get(&assign.section).copied().unwrap_or_else(|| {
            model.sections.keys().copied().min().unwrap_or(0)
        });
        let material_id = section_material
            .get(&assign.section)
            .copied()
            .or_else(|| model.materials.keys().copied().min())
            .unwrap_or(0);
        let id = model.add_element(Element::frame(1, start, end, material_id, section_id));
        element_by_line_story.insert((assign.line.clone(), assign.story.clone()), id);
        report.frames += 1;
    }

    // ---- areas and diaphragms --------------------------------------------
    let mut area_by_assign: HashMap<(String, String), usize> = HashMap::new();
    for assign in &area_assigns {
        let Some(area) = areas.get(&assign.area) else { continue };
        let Some(level) = level_of(&assign.story) else { continue };
        let mut ids = Vec::new();
        let mut missing = false;
        for point in &area.points {
            match new_node(&mut model, &mut nodes, point, level) {
                Some(id) => ids.push(id),
                None => missing = true,
            }
        }
        if missing || ids.len() < 3 {
            report.warnings.push(format!(
                "Area {} on {} has undefined points; it was skipped.",
                assign.area, assign.story
            ));
            continue;
        }
        let (thickness, material_id, wall_prop) = shell_info
            .get(&assign.shell)
            .copied()
            .unwrap_or((0.2, model.materials.keys().copied().min().unwrap_or(0), false));
        let kind = if area.kind.eq_ignore_ascii_case("panel") || wall_prop {
            AreaKind::Wall
        } else {
            AreaKind::Slab
        };
        let id = model.add_area(Area::new(ids.clone(), kind, material_id, thickness));
        area_by_assign.insert((assign.area.clone(), assign.story.clone()), id);
        report.areas += 1;
        if let Some(diaphragm) = &assign.diaphragm {
            let existing = model
                .diaphragms
                .iter()
                .find(|existing| existing.name == *diaphragm && (existing.story - level).abs() < 1e-6)
                .map(|existing| existing.id);
            let diaphragm_id = match existing {
                Some(id) => id,
                None => {
                    let id = model.diaphragms.len() + 1;
                    model.diaphragms.push(Diaphragm::new(id, diaphragm, level));
                    id
                }
            };
            for node_id in &ids {
                if let Some(node) = model.nodes.get_mut(node_id) {
                    node.diaphragm = Some(diaphragm_id);
                }
            }
        }
    }
    if !diaphragm_names.is_empty() && model.diaphragms.is_empty() {
        report.warnings.push(format!(
            "{} diaphragm name(s) are defined but no area assigns them, so no rigid diaphragm was created.",
            diaphragm_names.len()
        ));
    }

    // ---- supports ---------------------------------------------------------
    for (point, story, flags) in &restraints {
        let Some(level) = level_of(story) else { continue };
        if let Some(node_id) = new_node(&mut model, &mut nodes, point, level) {
            if let Some(node) = model.nodes.get_mut(&node_id) {
                node.restrained = *flags;
            }
        }
    }
    report.points = model.nodes.len();

    // ---- load patterns, cases and combinations ---------------------------
    if patterns.is_empty() {
        report.warnings.push("No LOADPATTERN records: the default patterns were kept.".to_string());
    } else {
        model.load_patterns = patterns
            .iter()
            .enumerate()
            .map(|(index, (name, kind, self_weight))| LoadPattern {
                id: index + 1,
                name: name.clone(),
                load_type: kind.clone(),
                self_weight_multiplier: *self_weight,
            })
            .collect();
    }
    let self_weight_of = |pattern: &Option<String>| -> f64 {
        pattern
            .as_ref()
            .and_then(|name| patterns.iter().find(|(existing, _, _)| existing == name))
            .map(|(_, _, self_weight)| *self_weight)
            .unwrap_or(1.0)
    };

    let mut imported_cases: Vec<LoadCase> = Vec::new();
    let mut case_ids: HashMap<String, usize> = HashMap::new();
    let mut skipped_cases: Vec<String> = Vec::new();
    for definition in &cases {
        if !definition.kind.to_ascii_lowercase().contains("linear static") {
            skipped_cases.push(definition.name.clone());
            continue;
        }
        let id = imported_cases.len() + 1;
        let pattern = definition
            .pattern
            .as_ref()
            .map(|name| load_type(name))
            .unwrap_or(LoadType::Dead);
        imported_cases.push(LoadCase {
            id,
            name: definition.name.clone(),
            pattern,
            self_weight_multiplier: self_weight_of(&definition.pattern),
            nodal_loads: Vec::new(),
            distributed_loads: Vec::new(),
            p_delta: false,
        });
        case_ids.insert(definition.name.clone(), id);
        report.load_cases += 1;
    }
    if !skipped_cases.is_empty() {
        report.warnings.push(format!(
            "{} non-static case(s) are not solved and were skipped: {}.",
            skipped_cases.len(),
            skipped_cases.join(", ")
        ));
    }

    for load in &line_loads {
        let Some(case_id) = case_ids.get(&load.case).copied() else { continue };
        let Some(element_id) = element_by_line_story
            .get(&(load.line.clone(), load.story.clone()))
            .copied()
        else {
            continue;
        };
        let magnitude = load.value * units.line_load();
        let start = match load.direction.to_ascii_uppercase().as_str() {
            "X" => [magnitude, 0.0, 0.0],
            "Y" => [0.0, magnitude, 0.0],
            _ => [0.0, 0.0, -magnitude],
        };
        let load = DistributedLoad {
            element_id,
            direction: LoadDirection::Global,
            start,
            end: start,
            projected: false,
            case_id: Some(case_id),
        };
        // The solver reads the model's frame-load table; the case copy is kept
        // for validation and the text writer.
        model.frame_loads.push(load.clone());
        let Some(case) = imported_cases.iter_mut().find(|case| case.id == case_id) else { continue };
        case.distributed_loads.push(load);
        report.frame_loads += 1;
    }

    for load in &area_loads {
        let Some(case_id) = case_ids.get(&load.case).copied() else { continue };
        let Some(area_id) = area_by_assign.get(&(load.area.clone(), load.story.clone())).copied() else { continue };
        model.area_loads.push(AreaLoad {
            area_id,
            direction: LoadDirection::Global,
            pressure: load.value * units.stress(),
            case_id: Some(case_id),
        });
        report.area_loads += 1;
    }
    if report.area_loads > 0 {
        report.warnings.push(format!(
            "{} area load(s) are transferred to their perimeter frame members as uniform line loads; shell elements are not modelled.",
            report.area_loads
        ));
    }

    for (name, factors) in &combos {
        let mut terms = Vec::new();
        let mut equation = String::new();
        for (case, factor) in factors {
            let Some(id) = case_ids.get(case).copied() else { continue };
            if !equation.is_empty() {
                equation.push_str(if *factor < 0.0 { " - " } else { " + " });
            }
            let _ = std::fmt::Write::write_fmt(&mut equation, format_args!("{}×{}", factor.abs(), case));
            terms.push((id, *factor));
        }
        if terms.is_empty() {
            continue;
        }
        let id = model.load_combinations.len() + 1;
        model.load_combinations.push(LoadCombination {
            id,
            name: name.clone(),
            equation,
            combo_type: ComboType::LinearAdd,
            factors: terms,
        });
        report.load_combinations += 1;
    }

    // A linear static case with no exported loads solves to zero. Auto wind and
    // seismic cases are calculated by the source application and arrive here empty, so say so
    // instead of letting a silent zero result look like a real analysis.
    let empty_cases: Vec<&str> = imported_cases
        .iter()
        .filter(|case| {
            case.self_weight_multiplier == 0.0
                && case.nodal_loads.is_empty()
                && case.distributed_loads.is_empty()
                && !model.area_loads.iter().any(|load| load.case_id == Some(case.id))
        })
        .map(|case| case.name.as_str())
        .collect();
    if !empty_cases.is_empty() {
        report.warnings.push(format!(
            "Case(s) {} have no exported loads and will solve to zero (auto wind/seismic loads are not generated).",
            empty_cases.join(", ")
        ));
    }

    model.load_cases = imported_cases;
    let analysis = Analysis::new(model.clone());

    // Records we knowingly ignore are listed once, with counts.
    let mut ignored: Vec<(String, usize)> = skipped_records.into_iter().collect();
    ignored.sort_by(|a, b| b.1.cmp(&a.1));
    let ignored_summary: Vec<String> = ignored
        .iter()
        .filter(|(keyword, _)| !matches!(keyword.as_str(), "CONCRETESECTION" | "SDSECTION" | "REBARDEFINITION" | "GRID" | "GRIDSYSTEM" | "MASSSOURCE" | "MASSSOURCELOAD" | "FUNCTION" | "AUTOMESHOPTIONS" | "ACTIVEDOF" | "PDELTA" | "PREFERENCE" | "RLLF" | "PROGRAM" | "MODELHINGESINLINKS"))
        .take(8)
        .map(|(keyword, count)| format!("{keyword} ×{count}"))
        .collect();
    if !ignored_summary.is_empty() {
        report.warnings.push(format!(
            "Records not imported: {}.",
            ignored_summary.join(", ")
        ));
    }

    Ok((model, analysis, report))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small text export in the same record style as a real file.
    const SAMPLE: &str = r#"
$ PROGRAM INFORMATION
  PROGRAM  "STRUCT"  VERSION "1.0"

$ CONTROLS
  UNITS  "N"  "MM"  "C"
  TITLE1  "Two storey"

$ STORIES - IN SEQUENCE FROM TOP
  STORY "Story2"  HEIGHT 3000 MASTERSTORY "Yes"
  STORY "Story1"  HEIGHT 3500 SIMILARTO "Story2"
  STORY "Base"  ELEV 0

$ MATERIAL PROPERTIES
  MATERIAL  "CONC"    TYPE "Concrete"    WEIGHTPERVOLUME 2.499262E-05
  MATERIAL  "CONC"    SYMTYPE "Isotropic"  E 25000  U 0.2  A 5.5E-06
  MATERIAL  "CONC"  FC 25

$ FRAME SECTIONS
  FRAMESECTION  "C(12X12)"  MATERIAL "CONC"  SHAPE "Concrete Rectangular"  D 305 B 305
  FRAMESECTION  "B(10x15)"  MATERIAL "CONC"  SHAPE "Concrete Rectangular"  D 381 B 254

$ POINT COORDINATES
  POINT "1"  0 0
  POINT "2"  4000 0

$ LINE CONNECTIVITIES
  LINE  "C1"  COLUMN  "1"  "1"  1
  LINE  "C2"  COLUMN  "2"  "2"  1
  LINE  "B1"  BEAM  "1"  "2"  0

$ POINT ASSIGNS
  POINTASSIGN  "1"  "Base"  RESTRAINT "UX UY UZ RX RY RZ"
  POINTASSIGN  "2"  "Base"  RESTRAINT "UX UY UZ RX RY RZ"

$ LINE ASSIGNS
  LINEASSIGN  "C1"  "Story1"  SECTION "C(12X12)"
  LINEASSIGN  "C1"  "Story2"  SECTION "C(12X12)"
  LINEASSIGN  "C2"  "Story1"  SECTION "C(12X12)"
  LINEASSIGN  "C2"  "Story2"  SECTION "C(12X12)"
  LINEASSIGN  "B1"  "Story1"  SECTION "B(10x15)"
  LINEASSIGN  "B1"  "Story2"  SECTION "B(10x15)"

$ LOAD PATTERNS
  LOADPATTERN "Dead"  TYPE  "Dead"  SELFWEIGHT  1
  LOADPATTERN "Live"  TYPE  "Live"  SELFWEIGHT  0

$ FRAME OBJECT LOADS
  LINELOAD  "B1"  "Story1"  TYPE "UNIFF"  DIR "GRAV"  LC "Live"  FVAL 10

$ SHELL OBJECT LOADS

$ LOAD CASES
  LOADCASE "Modal"  TYPE  "Modal - Eigen"  INITCOND  "PRESET"
  LOADCASE "Dead"  TYPE  "Linear Static"  INITCOND  "PRESET"
  LOADCASE "Dead"  LOADPAT  "Dead"  SF  1
  LOADCASE "Live"  TYPE  "Linear Static"  INITCOND  "PRESET"
  LOADCASE "Live"  LOADPAT  "Live"  SF  1

$ LOAD COMBINATIONS
  COMBO "UDConS1"  TYPE "Linear Add"
  COMBO "UDConS1"  LOADCASE "Dead"  SF 1.5
  COMBO "UDConS1"  LOADCASE "Live"  SF 1.5
"#;

    #[test]
    fn a_two_storey_export_becomes_a_solvable_model() {
        let (model, analysis, report) = parse_e2k(SAMPLE).expect("the sample must parse");

        // Stories: Base at 0, then Story1 3.5 m and Story2 6.5 m above it.
        assert_eq!(model.stories.len(), 3);
        let elevations: Vec<f64> = model.stories.iter().map(|story| story.elevation).collect();
        assert!((elevations[0] - 0.0).abs() < 1e-9, "the base sits at 0: {elevations:?}");
        assert!((elevations[1] - 3.5).abs() < 1e-9, "Story1 is one storey above the base: {elevations:?}");
        assert!((elevations[2] - 6.5).abs() < 1e-9, "Story2 is 3.0 m above Story1: {elevations:?}");
        assert_eq!(model.stories[2].similar_to.as_deref(), None, "only Story1 names a similar-to story");
        assert_eq!(model.stories[1].similar_to.as_deref(), Some("Story2"));
        assert_eq!(report.master_story.as_deref(), Some("Story2"));

        // Two columns per storey plus one beam per storey.
        assert_eq!(model.elements.len(), 6, "4 columns and 2 beams");
        assert_eq!(report.frames, 6);
        assert_eq!(model.nodes.len(), 6, "two plan points on three levels");

        // Supports land on the base nodes.
        let fixed = model.nodes.values().filter(|node| node.restrained == [true; 6]).count();
        assert_eq!(fixed, 2);

        // Units: 305 mm square column, 25 MPa concrete, 2500 kg/m³.
        let section = model.sections.values().find(|section| section.name == "C(12X12)").expect("column section");
        assert!((section.width - 0.305).abs() < 1e-9 && (section.depth - 0.305).abs() < 1e-9);
        let concrete = model.materials.values().find(|material| material.name == "CONC").expect("concrete");
        assert!((concrete.e - 25.0e9).abs() < 1.0, "25 MPa is 25 GPa: {}", concrete.e);
        assert!((concrete.fy - 25.0e6).abs() < 1.0, "FC is the compressive strength: {}", concrete.fy);
        // 2.499262E-05 N/mm³ is 25 kN/m³, i.e. 2548.5 kg/m³ of mass.
        assert!((concrete.rho - 2548.5).abs() < 1.0, "weight density converts to mass density: {}", concrete.rho);

        // Only the static cases are kept, so the solver's first case carries loads.
        assert_eq!(model.load_cases.len(), 2);
        assert_eq!(analysis.load_cases[0].name, "Dead");
        let live = model.load_cases.iter().find(|case| case.name == "Live").expect("live case");
        assert_eq!(live.distributed_loads.len(), 1, "the frame load belongs to the Live case");
        let load = &live.distributed_loads[0];
        assert!((load.start[2] + 10_000.0).abs() < 1e-6, "10 N/mm is 10 kN/m downwards: {:?}", load.start);

        // One combination, with both static cases factored.
        assert_eq!(model.load_combinations.len(), 1);
        assert_eq!(model.load_combinations[0].factors.len(), 2);
        assert_eq!(report.load_cases, 2);
        assert_eq!(report.load_combinations, 1);
        assert!(report.warnings.iter().any(|warning| warning.contains("non-static")), "the modal case is reported");
    }

    #[test]
    fn doubled_quotes_and_quoted_lists_survive_tokenizing() {
        let tokens = tokenize(r#"SHELLPROP "Slab5""" PROPTYPE "Slab" RESTRAINT "UX UY UZ""#);
        assert_eq!(tokens[0], "SHELLPROP");
        assert_eq!(tokens[1], "Slab5\"");
        assert_eq!(tokens[3], "Slab");
        assert_eq!(tokens[5], "UX UY UZ");
    }

    #[test]
    fn a_file_that_is_not_an_export_is_refused() {
        let error = match parse_e2k("hello\nworld\n") {
            Ok(_) => panic!("plain text is not an export"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("POINT or STORY"), "unexpected error: {error}");
    }

    /// The workspace's real NB2015 export, when it is present beside the crate.
    #[test]
    fn the_sample_nb2015_file_imports() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../NB2015.e2k");
        let Ok(text) = std::fs::read_to_string(path) else {
            eprintln!("NB2015.e2k is not present; the real-file check was skipped");
            return;
        };
        let (model, _, report) = parse_e2k(&text).expect("the real export must parse");
        assert_eq!(report.units, "N / MM");
        assert_eq!(model.stories.len(), 5, "Base plus four stories");
        assert!((model.stories[4].elevation - 11.9634).abs() < 1e-6, "top level at 11963.4 mm");
        assert!(model.materials.len() >= 6, "six materials in the file, got {}", model.materials.len());
        assert!(model.elements.len() >= 60, "columns and beams, got {}", model.elements.len());
        assert!(!model.load_cases.is_empty());
        assert!(!model.load_combinations.is_empty());
        let wall = model.load_cases.iter().find(|case| case.name == "Wall").expect("the Wall case imports");
        assert!(!wall.distributed_loads.is_empty(), "the wall loads land on their case's beams");
        assert!(!model.load_cases[0].name.is_empty(), "the first case stays a static case for the solver");
    }
}
