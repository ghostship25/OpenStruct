//! Load case editor helpers and validation for Phase 3.

use super::DistributedLoad;
use super::LoadCase;
use super::LoadPattern;
use crate::loads::LoadType;

/// Attributes that Phase 3 exposes when creating or editing a load case.
pub struct LoadCaseBuilder {
    pub id: usize,
    pub name: String,
    pub pattern: LoadPattern,
    pub self_weight_multiplier: f64,
    pub p_delta: bool,
    pub nodal_loads: Vec<crate::loads::NodalLoad>,
    pub distributed_loads: Vec<DistributedLoad>,
}

impl LoadCaseBuilder {
    pub fn new(id: usize, name: String, pattern: LoadPattern) -> Self {
        Self {
            id,
            name,
            pattern,
            self_weight_multiplier: 1.0,
            p_delta: false,
            nodal_loads: Vec::new(),
            distributed_loads: Vec::new(),
        }
    }

    pub fn self_weight(mut self, multiplier: f64) -> Self {
        self.self_weight_multiplier = multiplier;
        self
    }

    pub fn p_delta(mut self, on: bool) -> Self {
        self.p_delta = on;
        self
    }

    pub fn build(self) -> LoadCase {
        LoadCase {
            id: self.id,
            name: self.name,
            pattern: self.pattern.load_type,
            self_weight_multiplier: self.self_weight_multiplier,
            nodal_loads: self.nodal_loads,
            distributed_loads: self.distributed_loads,
            p_delta: self.p_delta,
        }
    }
}

/// Validation rules the Phase 3 load case editor presents before saving.
pub fn validate_case(case: &LoadCase, used_names: &std::collections::HashSet<String>) -> Vec<&'static str> {
    let mut errors = Vec::new();
    if case.name.trim().is_empty() {
        errors.push("Case name cannot be blank.");
    } else if used_names.contains(&case.name) {
        errors.push("Another case already uses this name.");
    }
    if case.self_weight_multiplier < 0.0 {
        errors.push("Self weight multiplier must be non-negative.");
    }
    match case.pattern {
        LoadType::Dead | LoadType::Live | LoadType::Wind | LoadType::Seismic => {}
        _ => {
            if case.nodal_loads.is_empty() && case.distributed_loads.is_empty() {
                if case.self_weight_multiplier <= 0.0 {
                    errors.push("A custom load case without a load type must carry loads or a self-weight multiplier.");
                }
            }
        }
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_name_is_rejected() {
        let case = LoadCaseBuilder::new(1, "".into(), LoadPattern::default()).build();
        assert!(validate_case(&case, &std::collections::HashSet::new()).contains(&"Case name cannot be blank."));
    }
}
