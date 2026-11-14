use crate::areas::Area;
use crate::elements::Element;
use crate::loads::{default_patterns, AreaLoad, DistributedLoad, LoadCase, LoadCombination, LoadPattern, TemperatureLoad};
use crate::materials::Material;
use crate::sections::Section;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: usize,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub restrained: [bool; 6], // UX, UY, UZ, RX, RY, RZ
    /// Support spring stiffness per global dof: N/m for translations and
    /// N·m/rad for rotations. Zero is no spring. A spring on a restrained dof
    /// has no effect (the support is already rigid).
    #[serde(default)]
    pub springs: [f64; 6],
    #[serde(default)]
    pub mass: [f64; 3],
    /// Rigid-diaphragm id this joint is tied to, if any.
    #[serde(default)]
    pub diaphragm: Option<usize>,
}

impl Node {
    pub fn new(id: usize, x: f64, y: f64, z: f64) -> Self {
        Self {
            id,
            x,
            y,
            z,
            restrained: [false; 6],
            springs: [0.0; 6],
            mass: [0.0; 3],
            diaphragm: None,
        }
    }

    pub fn fix_all(&mut self) {
        self.restrained = [true; 6];
    }

    /// True when any dof carries a support spring.
    pub fn has_springs(&self) -> bool {
        self.springs.iter().any(|&stiffness| stiffness > 0.0)
    }

    /// True when the joint has a rigid restraint or a support spring.
    pub fn has_support(&self) -> bool {
        self.restrained.iter().any(|&flag| flag) || self.has_springs()
    }
}

/// A building level. Objects are associated with a story by their Z coordinate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Story {
    pub name: String,
    pub elevation: f64,
    /// Story this level is typical of. `None` is the "None" entry.
    #[serde(default)]
    pub similar_to: Option<String>,
    /// True when column splices are taken at this level for steel design.
    #[serde(default)]
    pub splice_story: bool,
    /// Splice height above this story's elevation, in metres.
    #[serde(default)]
    pub splice_height: f64,
    /// Display colour of the level, RGB.
    #[serde(default = "default_story_color")]
    pub color: [u8; 3],
}

/// Neutral level colour, used for stories written before colours were stored.
pub fn default_story_color() -> [u8; 3] {
    [158, 158, 158]
}

impl Story {
    pub fn new(name: &str, elevation: f64) -> Self {
        Self {
            name: name.to_string(),
            elevation,
            similar_to: None,
            splice_story: false,
            splice_height: 0.0,
            color: default_story_color(),
        }
    }

    /// Name of the similar-to story as the Story Data table shows it.
    pub fn similar_to_label(&self) -> &str {
        self.similar_to
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .unwrap_or("None")
    }
}

/// A rigid floor diaphragm. Joints assigned to one are expected to lie on the
/// same level and to move together in plan.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Diaphragm {
    pub id: usize,
    pub name: String,
    /// Elevation of the floor the diaphragm ties, in metres.
    pub story: f64,
    /// True when the plane is also rigid out of plane (UZ, RX and RY tied).
    #[serde(default)]
    pub out_of_plane: bool,
}

impl Diaphragm {
    pub fn new(id: usize, name: &str, story: f64) -> Self {
        Self {
            id,
            name: name.to_string(),
            story,
            out_of_plane: false,
        }
    }

    /// True when the joint's level matches the diaphragm level.
    pub fn contains_level(&self, z: f64, tolerance: f64) -> bool {
        (self.story - z).abs() <= tolerance
    }
}

pub(crate) fn default_stories() -> Vec<Story> {
    vec![Story::new("Story1", 0.0)]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Model {
    pub name: String,
    #[serde(default = "default_stories")]
    pub stories: Vec<Story>,
    pub nodes: HashMap<usize, Node>,
    pub elements: HashMap<usize, Element>,
    #[serde(default)]
    pub areas: HashMap<usize, Area>,
    pub materials: HashMap<usize, Material>,
    pub sections: HashMap<usize, Section>,
    #[serde(default)]
    pub load_patterns: Vec<LoadPattern>,
    #[serde(default)]
    pub frame_loads: Vec<DistributedLoad>,
    #[serde(default)]
    pub area_loads: Vec<AreaLoad>,
    #[serde(default)]
    pub temperature_loads: Vec<TemperatureLoad>,
    #[serde(default)]
    pub load_cases: Vec<LoadCase>,
    #[serde(default)]
    pub load_combinations: Vec<LoadCombination>,
    /// Rigid floor diaphragms, with joints tied to each other by `Node::diaphragm`.
    #[serde(default)]
    pub diaphragms: Vec<Diaphragm>,
    pub node_counter: usize,
    pub element_counter: usize,
    #[serde(default)]
    pub area_counter: usize,
    pub material_counter: usize,
    pub section_counter: usize,
}

impl Model {
    /// A ready-made 3-storey, 3×3-bay steel building: columns on a 5 m grid,
    /// It is the Phase 2 "model a small building" acceptance case.
    pub fn three_story_three_bay_building() -> Self {
        const BAYS: usize = 3;
        const LEVELS: usize = 4;
        const BAY: f64 = 5.0;
        const HEIGHT: f64 = 3.5;
        const SIDE: usize = BAYS + 1;

        let mut model = Model::new("3-storey 3×3-bay building");
        let material_id = model.add_material(Material::steel());
        let section_id = model.add_section(Section::rectangular("COL 400x400", 0.4, 0.4));
        model.stories = (0..LEVELS - 1).map(|level| Story::new(&format!("Story{}", level + 1), level as f64 * HEIGHT)).collect();

        let mut joints = vec![vec![vec![0usize; SIDE]; SIDE]; LEVELS];
        for level in 0..LEVELS {
            for i in 0..SIDE {
                for j in 0..SIDE {
                    joints[level][i][j] = model.add_node(i as f64 * BAY, j as f64 * BAY, level as f64 * HEIGHT);
                }
            }
        }

        for level in 0..LEVELS - 1 {
            for i in 0..SIDE {
                for j in 0..SIDE {
                    model.add_element(Element::frame(1, joints[level][i][j], joints[level + 1][i][j], material_id, section_id));
                }
            }
        }
        for level in 1..LEVELS {
            for i in 0..SIDE {
                for j in 0..SIDE - 1 {
                    model.add_element(Element::frame(1, joints[level][i][j], joints[level][i][j + 1], material_id, section_id));
                }
            }
            for i in 0..SIDE - 1 {
                for j in 0..SIDE {
                    model.add_element(Element::frame(1, joints[level][i][j], joints[level][i + 1][j], material_id, section_id));
                }
            }
        }
        for level in 0..LEVELS - 1 {
            for i in 0..SIDE - 1 {
                for j in 0..SIDE - 1 {
                    let ids = vec![
                        joints[level][i][j],
                        joints[level][i + 1][j],
                        joints[level][i + 1][j + 1],
                        joints[level][i][j + 1],
                    ];
                    let mut area = Area::new(ids, crate::areas::AreaKind::Slab, material_id, 0.2);
                    area.curve = None;
                    model.add_area(area);
                }
            }
        }
        for i in 0..SIDE {
            for j in 0..SIDE {
                if let Some(node) = model.nodes.get_mut(&joints[0][i][j]) {
                    node.fix_all();
                }
            }
        }
        model
    }

    /// Deterministic 5,000-joint benchmark building: 9×9 bays, 50 levels.
    /// It exists for viewport and picking performance work, not design.
    pub fn benchmark_building() -> Self {
        const PER_SIDE: usize = 10;
        const STORIES: usize = 50;
        const BAY: f64 = 5.0;
        const HEIGHT: f64 = 3.5;

        let mut model = Model::new("OpenStruct 5,000-joint benchmark");
        let material_id = model.add_material(Material::steel());
        let section_id = model.add_section(Section::rectangular("BENCH 300x500", 0.3, 0.5));
        model.stories = (0..STORIES)
            .map(|level| Story::new(&format!("Story{}", level + 1), level as f64 * HEIGHT))
            .collect();

        let mut joints = vec![vec![vec![0usize; PER_SIDE]; PER_SIDE]; STORIES];
        for level in 0..STORIES {
            for i in 0..PER_SIDE {
                for j in 0..PER_SIDE {
                    joints[level][i][j] = model.add_node(i as f64 * BAY, j as f64 * BAY, level as f64 * HEIGHT);
                }
            }
        }

        for level in 0..STORIES - 1 {
            for i in 0..PER_SIDE {
                for j in 0..PER_SIDE {
                    model.add_element(Element::frame(1, joints[level][i][j], joints[level + 1][i][j], material_id, section_id));
                }
            }
        }
        for level in 0..STORIES {
            for i in 0..PER_SIDE {
                for j in 0..PER_SIDE - 1 {
                    model.add_element(Element::frame(1, joints[level][i][j], joints[level][i][j + 1], material_id, section_id));
                }
            }
            for i in 0..PER_SIDE - 1 {
                for j in 0..PER_SIDE {
                    model.add_element(Element::frame(1, joints[level][i][j], joints[level][i + 1][j], material_id, section_id));
                }
            }
        }
        for i in 0..PER_SIDE {
            for j in 0..PER_SIDE {
                model.nodes.get_mut(&joints[0][i][j]).unwrap().fix_all();
            }
        }
        model
    }

    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            stories: default_stories(),
            nodes: HashMap::new(),
            elements: HashMap::new(),
            areas: HashMap::new(),
            materials: HashMap::new(),
            sections: HashMap::new(),
            load_patterns: default_patterns(),
            frame_loads: Vec::new(),
            area_loads: Vec::new(),
            temperature_loads: Vec::new(),
            load_cases: Vec::new(),
            load_combinations: Vec::new(),
            diaphragms: Vec::new(),
            node_counter: 0,
            element_counter: 0,
            area_counter: 0,
            material_counter: 0,
            section_counter: 0,
        }
    }

    pub fn add_node(&mut self, x: f64, y: f64, z: f64) -> usize {
        self.node_counter += 1;
        let node = Node::new(self.node_counter, x, y, z);
        self.nodes.insert(self.node_counter, node);
        self.node_counter
    }

    pub fn add_material(&mut self, material: Material) -> usize {
        self.material_counter += 1;
        self.materials.insert(self.material_counter, material);
        self.material_counter
    }

    pub fn add_section(&mut self, section: Section) -> usize {
        self.section_counter += 1;
        self.sections.insert(self.section_counter, section);
        self.section_counter
    }

    pub fn add_element(&mut self, mut element: Element) -> usize {
        self.element_counter += 1;
        element.id = self.element_counter;
        self.elements.insert(self.element_counter, element);
        self.element_counter
    }

    /// Insert an area, assigning it the next area id.
    pub fn add_area(&mut self, mut area: Area) -> usize {
        self.area_counter += 1;
        area.id = self.area_counter;
        self.areas.insert(self.area_counter, area);
        self.area_counter
    }

    /// Elevation of the named story, if it exists.
    pub fn story_elevation(&self, name: &str) -> Option<f64> {
        self.stories
            .iter()
            .find(|story| story.name == name)
            .map(|story| story.elevation)
    }

    /// Index of the story at (or within `tolerance` of) an elevation.
    pub fn story_index_at(&self, z: f64, tolerance: f64) -> Option<usize> {
        self.stories
            .iter()
            .position(|story| (story.elevation - z).abs() <= tolerance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joint_springs_round_trip_and_default_to_zero() {
        // Models written before springs existed must still load.
        let legacy: Node = serde_json::from_str(
            r#"{"id":1,"x":0.0,"y":0.0,"z":0.0,"restrained":[false,false,false,false,false,false]}"#,
        )
        .expect("legacy node parses");
        assert_eq!(legacy.springs, [0.0; 6]);
        assert!(!legacy.has_support());

        let node = Node { springs: [1.0e6, 0.0, 5.0e5, 0.0, 0.0, 2.0e4], ..legacy };
        assert!(node.has_springs());
        assert!(node.has_support());
        let text = serde_json::to_string(&node).unwrap();
        let back: Node = serde_json::from_str(&text).unwrap();
        assert_eq!(back.springs, node.springs);
    }

    #[test]
    fn legacy_story_json_defaults_the_story_data_columns() {
        // Stories written before similar-to, splice and colour existed still load.
        let story: Story = serde_json::from_str(r#"{"name":"Story1","elevation":3.5}"#).expect("legacy story parses");
        assert_eq!(story.similar_to, None);
        assert!(!story.splice_story);
        assert_eq!(story.splice_height, 0.0);
        assert_eq!(story.color, default_story_color());
        assert_eq!(story.similar_to_label(), "None");

        let mut other = Story::new("Story2", 7.0);
        other.similar_to = Some("Story1".to_string());
        other.splice_story = true;
        other.splice_height = 0.6;
        other.color = [0, 170, 0];
        let text = serde_json::to_string(&other).unwrap();
        let back: Story = serde_json::from_str(&text).unwrap();
        assert_eq!(back, other);
        assert_eq!(back.similar_to_label(), "Story1");
    }

    #[test]
    fn benchmark_building_has_five_thousand_joints() {
        let model = Model::benchmark_building();
        assert_eq!(model.nodes.len(), 5_000);
        assert_eq!(model.elements.len(), 13_900);
        assert_eq!(model.stories.len(), 50);
        assert_eq!(model.materials.len(), 1);
        assert_eq!(model.sections.len(), 1);
        assert!(model.areas.is_empty());
    }

    #[test]
    fn three_story_building_has_complete_levels_and_supports() {
        let model = Model::three_story_three_bay_building();
        assert_eq!(model.stories.len(), 3);
        assert_eq!(model.nodes.len(), 64);
        assert_eq!(model.areas.len(), 27);
        assert_eq!(model.elements.len(), 48 + 72);
        assert!(model.nodes.values().all(|node| node.z >= 0.0));
        assert!(model.nodes.values().filter(|node| node.z == 0.0).all(|node| node.restrained == [true; 6]));
        assert_eq!(model.nodes.values().filter(|node| node.z == 0.0).count(), 16);
    }
}
