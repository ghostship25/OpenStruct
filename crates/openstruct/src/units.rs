//! Display units and unit-aware input parsing.
//!
//! The model always stores SI — metres, newtons, newton-metres and pascals.
//! Everything here converts SI to the user's display unit or parses typed
//! input (for example `5 ft`, `3000 mm`, `12 kN/m`, `2 kPa`) back to SI.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum UnitSystem {
    /// Kilonewtons, metres, millimetres, kilopascals.
    #[default]
    Metric,
    Imperial,
}

impl UnitSystem {
    pub const ALL: [UnitSystem; 2] = [UnitSystem::Metric, UnitSystem::Imperial];

    pub fn label(self) -> &'static str {
        match self {
            UnitSystem::Metric => "Metric (kN, m)",
            UnitSystem::Imperial => "Imperial (kip, ft)",
        }
    }

    pub fn len_label(self) -> &'static str {
        match self {
            UnitSystem::Metric => "m",
            UnitSystem::Imperial => "ft",
        }
    }

    pub fn len_factor(self) -> f64 {
        match self {
            UnitSystem::Metric => 1.0,
            UnitSystem::Imperial => 3.280_839_895_013_123,
        }
    }

    pub fn disp_label(self) -> &'static str {
        match self {
            UnitSystem::Metric => "mm",
            UnitSystem::Imperial => "in",
        }
    }

    pub fn disp_factor(self) -> f64 {
        match self {
            UnitSystem::Metric => 1000.0,
            UnitSystem::Imperial => 39.370_078_740_157_48,
        }
    }

    pub fn force_label(self) -> &'static str {
        match self {
            UnitSystem::Metric => "kN",
            UnitSystem::Imperial => "kip",
        }
    }

    pub fn force_factor(self) -> f64 {
        match self {
            UnitSystem::Metric => 1.0,
            UnitSystem::Imperial => 0.224_808_943_1,
        }
    }

    pub fn moment_label(self) -> &'static str {
        match self {
            UnitSystem::Metric => "kN·m",
            UnitSystem::Imperial => "kip·ft",
        }
    }

    pub fn moment_factor(self) -> f64 {
        match self {
            UnitSystem::Metric => 1.0,
            UnitSystem::Imperial => 0.737_562_149_3,
        }
    }

    pub fn dist_label(self) -> &'static str {
        match self {
            UnitSystem::Metric => "kN/m",
            UnitSystem::Imperial => "kip/ft",
        }
    }

    pub fn dist_factor(self) -> f64 {
        match self {
            UnitSystem::Metric => 1.0,
            UnitSystem::Imperial => 0.068_521_766,
        }
    }

    pub fn pressure_label(self) -> &'static str {
        match self {
            UnitSystem::Metric => "kPa",
            UnitSystem::Imperial => "ksf",
        }
    }

    pub fn pressure_factor(self) -> f64 {
        match self {
            UnitSystem::Metric => 1.0,
            UnitSystem::Imperial => 0.020_885_434,
        }
    }

    pub fn area_label(self) -> &'static str {
        match self {
            UnitSystem::Metric => "m²",
            UnitSystem::Imperial => "ft²",
        }
    }

    pub fn area_factor(self) -> f64 {
        self.len_factor().powi(2)
    }

    pub fn inertia_label(self) -> &'static str {
        match self {
            UnitSystem::Metric => "m⁴",
            UnitSystem::Imperial => "ft⁴",
        }
    }

    pub fn inertia_factor(self) -> f64 {
        self.len_factor().powi(4)
    }

    pub fn section_modulus_label(self) -> &'static str {
        match self {
            UnitSystem::Metric => "m³",
            UnitSystem::Imperial => "ft³",
        }
    }

    pub fn section_modulus_factor(self) -> f64 {
        self.len_factor().powi(3)
    }

    // -- display conversion --------------------------------------------------

    /// Metres to display units.
    pub fn length_to_display(self, metres: f64) -> f64 {
        metres * self.len_factor()
    }

    /// Display units to metres.
    pub fn length_from_display(self, value: f64) -> f64 {
        value / self.len_factor()
    }

    /// Newtons to display force units.
    pub fn force_to_display(self, newtons: f64) -> f64 {
        newtons / 1000.0 * self.force_factor()
    }

    /// Newtons to display moment units.
    pub fn moment_to_display(self, newton_metres: f64) -> f64 {
        newton_metres / 1000.0 * self.moment_factor()
    }

    /// Newtons per metre to display distributed-load units.
    pub fn dist_to_display(self, newtons_per_metre: f64) -> f64 {
        newtons_per_metre / 1000.0 * self.dist_factor()
    }

    /// Pascals to display pressure units.
    pub fn pressure_to_display(self, pascals: f64) -> f64 {
        pascals / 1000.0 * self.pressure_factor()
    }

    // -- unit-aware parsing --------------------------------------------------

    /// Parse a length; returns metres. A bare number means display units.
    pub fn parse_length(self, input: &str) -> Option<f64> {
        let (value, unit) = split_quantity(input)?;
        let metres = match unit.as_str() {
            "" => value / self.len_factor(),
            "m" | "metre" | "meter" | "metres" | "meters" => value,
            "mm" => value / 1000.0,
            "cm" => value / 100.0,
            "ft" | "foot" | "feet" | "'" => value * 0.3048,
            "in" | "inch" | "inches" | "\"" => value * 0.0254,
            _ => return None,
        };
        Some(metres)
    }

    /// Parse a force; returns newtons. A bare number means display units.
    pub fn parse_force(self, input: &str) -> Option<f64> {
        let (value, unit) = split_quantity(input)?;
        let newtons = match unit.as_str() {
            "" => value / self.force_factor() * 1000.0,
            "n" => value,
            "kn" => value * 1000.0,
            "kip" | "kips" => value * 4448.221_615_3,
            _ => return None,
        };
        Some(newtons)
    }

    /// Parse a distributed load; returns newtons per metre.
    pub fn parse_distributed(self, input: &str) -> Option<f64> {
        let (value, unit) = split_quantity(input)?;
        let newtons_per_metre = match unit.as_str() {
            "" => value / self.dist_factor() * 1000.0,
            "n/m" => value,
            "kn/m" => value * 1000.0,
            "kip/ft" => value * 14_593.902_9,
            "kn/m2" | "kn/m²" => return None,
            _ => return None,
        };
        Some(newtons_per_metre)
    }

    /// Parse a pressure; returns pascals.
    pub fn parse_pressure(self, input: &str) -> Option<f64> {
        let (value, unit) = split_quantity(input)?;
        let pascals = match unit.as_str() {
            "" => value / self.pressure_factor() * 1000.0,
            "pa" => value,
            "kpa" | "kn/m2" | "kn/m²" => value * 1000.0,
            "ksf" => value * 47_880.258_8,
            _ => return None,
        };
        Some(pascals)
    }
}

/// Split `"5 ft"` into `(5.0, "ft")`, tolerating spaces and mixed case.
fn split_quantity(input: &str) -> Option<(f64, String)> {
    let text = input.trim();
    if text.is_empty() {
        return None;
    }
    let split = text
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+' || c == 'e' || c == 'E'))
        .unwrap_or(text.len());
    let (number, unit) = text.split_at(split);
    let value: f64 = number.trim().parse().ok()?;
    Some((value, unit.trim().replace(' ', "").to_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metric_parsing_accepts_explicit_units() {
        let metric = UnitSystem::Metric;
        assert!((metric.parse_length("3000 mm").unwrap() - 3.0).abs() < 1e-12);
        assert!((metric.parse_length("2.5 m").unwrap() - 2.5).abs() < 1e-12);
        assert!((metric.parse_length("5").unwrap() - 5.0).abs() < 1e-12);
        assert!(metric.parse_length("nonsense").is_none());
    }

    #[test]
    fn imperial_parsing_uses_display_units_for_bare_numbers() {
        let imperial = UnitSystem::Imperial;
        assert!((imperial.parse_length("10").unwrap() - 3.048).abs() < 1e-9);
        assert!((imperial.parse_length("5 ft").unwrap() - 1.524).abs() < 1e-9);
        assert!((imperial.parse_length("12 in").unwrap() - 0.3048).abs() < 1e-9);
    }

    #[test]
    fn load_parsing_returns_si() {
        let metric = UnitSystem::Metric;
        assert!((metric.parse_distributed("12 kN/m").unwrap() - 12_000.0).abs() < 1e-9);
        assert!((metric.parse_distributed("12").unwrap() - 12_000.0).abs() < 1e-9);
        assert!((metric.parse_pressure("2 kPa").unwrap() - 2000.0).abs() < 1e-9);
        assert!((metric.parse_pressure("1.5 kN/m²").unwrap() - 1500.0).abs() < 1e-9);
        assert!((metric.parse_force("3 kN").unwrap() - 3000.0).abs() < 1e-9);
    }

    #[test]
    fn display_conversion_round_trips() {
        for units in UnitSystem::ALL {
            let metres = 4.572;
            let shown = units.length_to_display(metres);
            assert!((units.length_from_display(shown) - metres).abs() < 1e-12);
        }
    }
}
