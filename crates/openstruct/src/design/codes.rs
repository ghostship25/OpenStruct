//! Design-code selection for the steel and concrete checks.
//!
//! Three code families are supported:
//!
//! - **AISC/ACI** — AISC 360-16 for steel and ACI 318-19 for concrete, both
//!   using load-and-resistance-factor (LRFD/φ) design.
//! - **Eurocode** — EN 1993-1-1 for steel and EN 1992-1-1 for concrete, using
//!   the partial factors of those standards.
//! - **Indian** — IS 800:2007 for steel and IS 456:2000 for concrete, using the
//!   Indian partial safety factors for material strength.
//!
//! The factors below are the ones the checks in [`super::steel`] and
//! [`super::concrete`] apply. Every design function takes a [`DesignCode`], so
//! one model can be reviewed the way each market does it without touching the
//! geometry.
//!
//! Unit convention (the same one the model uses): lengths in metres, stresses
//! in pascals, forces in newtons and moments in newton-metres.

use serde::{Deserialize, Serialize};

/// The design code a steel or concrete check is carried out to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DesignCode {
    /// AISC 360-16 steel and ACI 318-19 concrete (LRFD).
    #[default]
    AiscAci,
    /// EN 1993-1-1 steel and EN 1992-1-1 concrete.
    Eurocode,
    /// IS 800:2007 steel and IS 456:2000 concrete.
    Indian,
}

impl DesignCode {
    /// Every code, in menu order.
    pub const ALL: [DesignCode; 3] = [
        DesignCode::AiscAci,
        DesignCode::Eurocode,
        DesignCode::Indian,
    ];

    /// Short label for tables and combo boxes.
    pub fn short_name(self) -> &'static str {
        match self {
            DesignCode::AiscAci => "AISC/ACI",
            DesignCode::Eurocode => "Eurocode",
            DesignCode::Indian => "IS 800/456",
        }
    }

    /// The standards this code applies.
    pub fn label(self) -> &'static str {
        match self {
            DesignCode::AiscAci => "AISC 360-16 / ACI 318-19",
            DesignCode::Eurocode => "EN 1993-1-1 / EN 1992-1-1",
            DesignCode::Indian => "IS 800:2007 / IS 456:2000",
        }
    }

    /// Which factor system each code uses, for the dialogs.
    pub fn factor_name(self) -> &'static str {
        match self {
            DesignCode::AiscAci => "resistance factors (phi)",
            DesignCode::Eurocode => "partial factors (gamma M)",
            DesignCode::Indian => "partial safety factors (gamma m)",
        }
    }

    /// Steel factors: compression, bending and shear reduction, plus the
    /// imperfection factor the buckling curve uses.
    pub fn steel_factors(self) -> SteelCodeFactors {
        match self {
            // AISC 360-16: phi_c = 0.90 (E1), phi_b = 0.90 (F1),
            // phi_v = 1.00 for shear yielding (G2.1). The column curve is E3,
            // which does not use an imperfection factor, so it stays zero.
            DesignCode::AiscAci => SteelCodeFactors {
                compression: 0.90,
                bending: 0.90,
                shear: 1.00,
                imperfection: 0.0,
            },
            // EN 1993-1-1: gamma_M0 = gamma_M1 = 1.00 at the recommended
            // values. Curve b (alpha = 0.34) for h/b > 1.2, otherwise curve c.
            DesignCode::Eurocode => SteelCodeFactors {
                compression: 1.00,
                bending: 1.00,
                shear: 1.00,
                imperfection: 0.34,
            },
            // IS 800:2007 Table 5: gamma_m0 = 1.10, gamma_m1 = 1.25. The
            // compressive strength uses gamma_m0 (7.1.2) with buckling class c
            // (alpha = 0.49) for hot-rolled sections.
            DesignCode::Indian => SteelCodeFactors {
                compression: 1.0 / 1.10,
                bending: 1.0 / 1.10,
                shear: 1.0 / 1.10,
                imperfection: 0.49,
            },
        }
    }

    /// Concrete factors: the φ values ACI applies to the nominal strength, or
    /// the partial factors EC2 and IS 456 apply to the material strengths.
    pub fn concrete_factors(self) -> ConcreteCodeFactors {
        match self {
            // ACI 318-19: phi_c = 0.65 (tied), phi_b = 0.90, phi_v = 0.75.
            DesignCode::AiscAci => ConcreteCodeFactors {
                axial: 0.65,
                bending: 0.90,
                shear: 0.75,
                gamma_c: 1.0,
                gamma_s: 1.0,
                steel_fy: 420.0e6,
            },
            // EN 1992-1-1: gamma_c = 1.50, gamma_s = 1.15, alpha_cc = 0.85,
            // with f_yk = 500 MPa for the assumed reinforcement.
            DesignCode::Eurocode => ConcreteCodeFactors {
                axial: 0.85 / 1.50,
                bending: 1.0 / 1.15,
                shear: 1.0,
                gamma_c: 1.50,
                gamma_s: 1.15,
                steel_fy: 500.0e6,
            },
            // IS 456:2000: partial safety factors of 1.50 on concrete and 1.15
            // on steel, already inside the 0.4/0.67 design strengths.
            DesignCode::Indian => ConcreteCodeFactors {
                axial: 1.0,
                bending: 1.0,
                shear: 1.0,
                gamma_c: 1.50,
                gamma_s: 1.15,
                steel_fy: 415.0e6,
            },
        }
    }
}

impl std::fmt::Display for DesignCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.short_name())
    }
}

/// Steel design factors for one code.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SteelCodeFactors {
    /// Multiplier applied to the compressive strength (φ_c, or 1/γ_M).
    pub compression: f64,
    /// Multiplier applied to the flexural strength (φ_b, or 1/γ_M).
    pub bending: f64,
    /// Multiplier applied to the shear strength (φ_v, or 1/γ_M0).
    pub shear: f64,
    /// Buckling-curve imperfection factor (zero where the code uses its own
    /// column curve, as AISC 360 E3 does).
    pub imperfection: f64,
}

/// Concrete design factors for one code.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConcreteCodeFactors {
    /// Axial-strength multiplier (ACI φ, or α_cc/γ_c for EC2; IS 456 keeps its
    /// factors inside the design strengths, so this is 1.0).
    pub axial: f64,
    /// Flexural multiplier (ACI φ_b, or 1/γ_s for EC2).
    pub bending: f64,
    /// Shear multiplier (ACI φ_v; EC2 and IS 456 use design strengths).
    pub shear: f64,
    /// Partial factor on concrete.
    pub gamma_c: f64,
    /// Partial factor on reinforcement.
    pub gamma_s: f64,
    /// Reinforcement yield strength assumed by the check, in pascals.
    pub steel_fy: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_aisc_aci() {
        assert_eq!(DesignCode::default(), DesignCode::AiscAci);
    }

    #[test]
    fn every_code_has_a_distinct_label() {
        let labels: Vec<&str> = DesignCode::ALL.iter().map(|code| code.label()).collect();
        assert_eq!(labels.len(), 3);
        assert!(labels[0] != labels[1] && labels[1] != labels[2] && labels[0] != labels[2]);
    }

    #[test]
    fn indian_factors_are_stricter_than_eurocode_for_steel() {
        let euro = DesignCode::Eurocode.steel_factors();
        let indian = DesignCode::Indian.steel_factors();
        assert!(indian.compression < euro.compression);
        assert!(indian.bending < euro.bending);
        assert!(indian.shear < euro.shear);
    }

    #[test]
    fn codes_carry_their_own_reinforcement_grade() {
        assert!((DesignCode::AiscAci.concrete_factors().steel_fy - 420.0e6).abs() < 1.0);
        assert!((DesignCode::Eurocode.concrete_factors().steel_fy - 500.0e6).abs() < 1.0);
        assert!((DesignCode::Indian.concrete_factors().steel_fy - 415.0e6).abs() < 1.0);
    }
}
