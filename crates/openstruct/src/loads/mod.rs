//! Load patterns, load cases and combinations.
//!
//! Core primitives live here; the Phase 3 editor helpers live in the
//! `patterns`, `cases` and `combos` sub-modules.

pub mod patterns;
pub mod cases;
pub mod combos;

use serde::{Deserialize, Serialize};

/// Load type of a load pattern: Dead / Live / Wind / Quake / … set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoadType {
    Dead,
    Live,
    Wind,
    Seismic,
    Snow,
    Other(String),
}

impl LoadType {
    /// Display name of the load type.
    pub fn label(&self) -> &str {
        match self {
            LoadType::Dead => "Dead",
            LoadType::Live => "Live",
            LoadType::Wind => "Wind",
            LoadType::Seismic => "Quake",
            LoadType::Snow => "Snow",
            LoadType::Other(name) => name.as_str(),
        }
    }
}

/// Direction a load is applied in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoadDirection {
    Global,
    Local,
}

/// A concentrated load at a joint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodalLoad {
    pub node_id: usize,
    pub fx: f64,
    pub fy: f64,
    pub fz: f64,
    pub mx: f64,
    pub my: f64,
    pub mz: f64,
}

/// A trapezoidal distributed load over a frame element.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedLoad {
    pub element_id: usize,
    pub direction: LoadDirection,
    pub start: [f64; 3],
    pub end: [f64; 3],
    pub projected: bool,
    /// is how loads saved before cases existed keep behaving.
    #[serde(default)]
    pub case_id: Option<usize>,
}

/// A uniform pressure over an area object, in pascals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AreaLoad {
    pub area_id: usize,
    pub direction: LoadDirection,
    pub pressure: f64,
    #[serde(default)]
    pub case_id: Option<usize>,
}

/// A temperature or initial-strain load on a frame element.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemperatureLoad {
    pub element_id: usize,
    pub delta_t: f64,
    pub strain: f64,
    #[serde(default)]
    pub case_id: Option<usize>,
}

impl TemperatureLoad {
    /// Equivalent axial strain: thermal expansion plus the prescribed strain.
    pub fn axial_strain(&self, alpha: f64) -> f64 {
        alpha * self.delta_t + self.strain
    }
}

/// A named load pattern (DEAD, LIVE, WIND, …).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoadPattern {
    pub id: usize,
    pub name: String,
    pub load_type: LoadType,
    pub self_weight_multiplier: f64,
}

impl Default for LoadPattern {
    fn default() -> Self {
        Self {
            id: 0,
            name: String::new(),
            load_type: LoadType::Dead,
            self_weight_multiplier: 1.0,
        }
    }
}

fn unit() -> f64 {
    1.0
}

/// A load case: one pattern applied with a self-weight multiplier and P-Delta option.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadCase {
    pub id: usize,
    pub name: String,
    pub pattern: LoadType,
    #[serde(default = "unit")]
    pub self_weight_multiplier: f64,
    pub nodal_loads: Vec<NodalLoad>,
    pub distributed_loads: Vec<DistributedLoad>,
    #[serde(default)]
    pub p_delta: bool,
}

impl Default for LoadCase {
    fn default() -> Self {
        Self {
            id: 0,
            name: String::new(),
            pattern: LoadType::Dead,
            self_weight_multiplier: 1.0,
            nodal_loads: Vec::new(),
            distributed_loads: Vec::new(),
            p_delta: false,
        }
    }
}

/// How the component cases of a combination are combined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComboType {
    LinearAdd,
    Envelope,
    Ssrs,
    Abs,
    Cqc,
}

impl ComboType {
    /// Display name of the combination type.
    pub fn label(&self) -> &'static str {
        match self {
            ComboType::LinearAdd => "Linear Add",
            ComboType::Envelope => "Envelope",
            ComboType::Ssrs => "SRSS",
            ComboType::Abs => "ABS",
            ComboType::Cqc => "CQC",
        }
    }
}

/// A load combination: pattern factors combined by [`ComboType`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoadCombination {
    pub id: usize,
    pub name: String,
    pub equation: String,
    pub combo_type: ComboType,
    pub factors: Vec<(usize, f64)>,
}

pub use cases::{validate_case, LoadCaseBuilder};
pub use combos::{code_default_hints, ComboBuilder};
pub use patterns::{default_patterns, standard_catalogue};
