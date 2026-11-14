//! Default load-pattern catalogue and helpers.

use super::LoadPattern;
use crate::loads::LoadType;

/// Standard standard reference patterns presented as one starting point in
/// `Define → Load Patterns…` until a user-edited library replaces them.
pub fn default_patterns() -> Vec<LoadPattern> {
    standard_catalogue()
        .into_iter()
        .enumerate()
        .map(|(index, (load_type, name))| LoadPattern {
            id: index + 1,
            name: name.to_string(),
            load_type,
            self_weight_multiplier: 1.0,
        })
        .collect()
}

/// Built-in reference load types from Dead / Live / Wind / Quake / Snow / Rain /
/// Temperature / Other model, used as the starting catalogue for the Define → Load Patterns
/// editor in Phase 3.
pub fn standard_catalogue() -> Vec<(LoadType, &'static str)> {
    vec![
        (LoadType::Dead, "DEAD"),
        (LoadType::Live, "LIVE"),
        (LoadType::Other("RDLL".into()), "RDLL"),
        (LoadType::Wind, "WIND"),
        (LoadType::Seismic, "QUAKE"),
        (LoadType::Other("Snow".into()), "SNOW"),
        (LoadType::Other("Rain".into()), "RAIN"),
        (LoadType::Other("Temp".into()), "TEMP"),
        (LoadType::Other("Other".into()), "OTHER"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_patterns_have_expected_names() {
        let patterns = default_patterns();
        let names: Vec<&str> = patterns.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"DEAD"));
        assert!(names.contains(&"LIVE"));
        assert!(names.contains(&"RDLL"));
        assert!(names.contains(&"WIND"));
        assert!(names.contains(&"QUAKE"));
        assert!(names.contains(&"SNOW"));
        assert!(names.contains(&"RAIN"));
        assert!(names.contains(&"TEMP"));
    }
}
