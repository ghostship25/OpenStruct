//! Steel Connection Design per AISC 360-16.
//!
//! Implements basic connection checks for:
//! - Bolted connections: shear, bearing, tear-out, tension
//! - Welded connections: fillet weld, CJP/PJP welds
//! - Connection classification: simple, moment-resisting, semi-rigid

use crate::materials::Material;
use crate::sections::Section;
use std::fmt;

/// Resistance factor for bolted connections (AISC 360 Table J3.2)
pub const PHI_BOLT_SHEAR: f64 = 0.75;
pub const PHI_BOLT_TENSION: f64 = 0.75;
pub const PHI_BEARING: f64 = 0.75;
pub const PHI_WELD: f64 = 0.75;

/// Bolt grade properties (AISC 360 Table J3.2)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BoltGrade {
    /// ASTM A325 high-strength bolts
    A325,
    A490,
    A307,
}

impl BoltGrade {
    /// Nominal tensile strength F_nt (ksi or MPa)
    pub fn f_nt(&self) -> f64 {
        match self {
            BoltGrade::A325 => 120.0, // ksi
            BoltGrade::A490 => 150.0, // ksi
            BoltGrade::A307 => 60.0,  // ksi
        }
    }

    /// Nominal shear strength F_nv (ksi or MPa)
    pub fn f_nv(&self) -> f64 {
        match self {
            BoltGrade::A325 => 68.0,  // ksi (threads included)
            BoltGrade::A490 => 84.0,  // ksi
            BoltGrade::A307 => 30.0,  // ksi
        }
    }
}

/// Bolt size and properties
#[derive(Debug, Clone, Copy)]
pub struct Bolt {
    /// Bolt diameter (inches or mm)
    pub diameter: f64,
    pub grade: BoltGrade,
    pub count: usize,
    pub shear_planes: usize,
}

impl Bolt {
    /// Bolt cross-sectional area (in² or mm²)
    pub fn area(&self) -> f64 {
        std::f64::consts::PI * (self.diameter / 2.0).powi(2)
    }

    /// Nominal shear strength per bolt (kips or kN)
    /// R_n = F_nv * A_b * N (AISC J3.6)
    pub fn nominal_shear(&self) -> f64 {
        self.grade.f_nv() * self.area() * self.shear_planes as f64
    }

    /// Nominal tensile strength per bolt (kips or kN)
    /// R_n = F_nt * A_b (AISC J3.6)
    pub fn nominal_tension(&self) -> f64 {
        self.grade.f_nt() * self.area()
    }

    /// Design shear strength per bolt (kips or kN)
    pub fn design_shear(&self) -> f64 {
        PHI_BOLT_SHEAR * self.nominal_shear()
    }

    /// Design tension strength per bolt (kips or kN)
    pub fn design_tension(&self) -> f64 {
        PHI_BOLT_TENSION * self.nominal_tension()
    }
}

/// Bolted connection check results
#[derive(Debug, Clone)]
pub struct BoltedConnectionResult {
    /// Connection identifier
    pub connection_id: String,
    pub bolt_count: usize,
    pub bolt_diameter: f64,
    pub bolt_grade: BoltGrade,
    pub applied_shear: f64,
    pub applied_tension: f64,
    pub design_shear_capacity: f64,
    pub design_tension_capacity: f64,
    pub shear_util: f64,
    pub tension_util: f64,
    pub interaction_ratio: f64,
    pub status: ConnectionStatus,
}

/// Connection status
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionStatus {
    Pass,
    Fail,
    CheckRequired,
}

/// Check a bolted connection per AISC 360
pub fn check_bolted_connection(
    connection_id: &str,
    bolt: &Bolt,
    applied_shear: f64,
    applied_tension: f64,
) -> BoltedConnectionResult {
    let design_shear_cap = bolt.design_shear() * bolt.count as f64;
    let design_tension_cap = bolt.design_tension() * bolt.count as f64;

    let shear_util = if design_shear_cap > 0.0 {
        applied_shear.abs() / design_shear_cap
    } else {
        f64::INFINITY
    };

    let tension_util = if design_tension_cap > 0.0 {
        applied_tension.abs() / design_tension_cap
    } else {
        f64::INFINITY
    };

    // AISC J3.7 interaction: (P/P_n + M/M_n) or simplified shear-tension interaction
    // For combined shear and tension, use: (f_va/F_nv + f_nt/F_nt) ≤ 1.0
    let interaction_ratio = shear_util + tension_util;

    let status = if interaction_ratio <= 1.0 {
        ConnectionStatus::Pass
    } else {
        ConnectionStatus::Fail
    };

    BoltedConnectionResult {
        connection_id: connection_id.to_string(),
        bolt_count: bolt.count,
        bolt_diameter: bolt.diameter,
        bolt_grade: bolt.grade,
        applied_shear,
        applied_tension,
        design_shear_capacity: design_shear_cap,
        design_tension_capacity: design_tension_cap,
        shear_util,
        tension_util,
        interaction_ratio,
        status,
    }
}

/// Bearing check result (AISC J3.10)
#[derive(Debug, Clone)]
pub struct BearingResult {
    /// Design bearing strength (kips or kN)
    pub design_bearing: f64,
    pub applied_force: f64,
    pub util: f64,
    pub status: ConnectionStatus,
}

/// Check bearing capacity per AISC 360 J3.10
/// R_n = 1.2 * L_c * t * F_u ≤ 2.4 * d * t * F_u
pub fn check_bearing(
    bolt_diameter: f64,
    plate_thickness: f64,
    edge_distance: f64,
    fu: f64, // ultimate tensile strength of connected material
    applied_force: f64,
) -> BearingResult {
    let lc = edge_distance - bolt_diameter / 2.0; // clear distance
    let lc = lc.max(0.0);

    // Nominal bearing strength
    let r_n_1 = 1.2 * lc * plate_thickness * fu;
    let r_n_2 = 2.4 * bolt_diameter * plate_thickness * fu;
    let r_n = r_n_1.min(r_n_2);

    let design_bearing = PHI_BEARING * r_n;
    let util = if design_bearing > 0.0 {
        applied_force.abs() / design_bearing
    } else {
        f64::INFINITY
    };

    let status = if util <= 1.0 {
        ConnectionStatus::Pass
    } else {
        ConnectionStatus::Fail
    };

    BearingResult {
        design_bearing,
        applied_force,
        util,
        status,
    }
}

/// Tear-out check (AISC J3.10)
/// R_n = 1.5 * L_c * t * F_u (for tear-out at edge)
#[derive(Debug, Clone)]
pub struct TearOutResult {
    pub design_tearout: f64,
    pub applied_force: f64,
    pub util: f64,
    pub status: ConnectionStatus,
}

pub fn check_tearout(
    clear_distance: f64,
    plate_thickness: f64,
    fu: f64,
    applied_force: f64,
) -> TearOutResult {
    let r_n = 1.5 * clear_distance * plate_thickness * fu;
    let design_tearout = PHI_BEARING * r_n;

    let util = if design_tearout > 0.0 {
        applied_force.abs() / design_tearout
    } else {
        f64::INFINITY
    };

    let status = if util <= 1.0 {
        ConnectionStatus::Pass
    } else {
        ConnectionStatus::Fail
    };

    TearOutResult {
        design_tearout,
        applied_force,
        util,
        status,
    }
}

/// Weld type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WeldType {
    /// Fillet weld
    Fillet,
    CJP,
    PJP,
}

/// Welded connection check results
#[derive(Debug, Clone)]
pub struct WeldedConnectionResult {
    pub connection_id: String,
    pub weld_type: WeldType,
    pub weld_size: f64,
    pub weld_length: f64,
    pub applied_force: f64,
    pub design_strength: f64,
    pub util: f64,
    pub status: ConnectionStatus,
}

/// Check fillet weld strength per AISC 360 J2.4
/// R_n = 0.6 * F_exx * (0.707 * w) * L * (1.0 + 0.5 * sin^1.5(theta))
/// Simplified: R_n = 0.6 * F_exx * (0.707 * w) * L
pub fn check_fillet_weld(
    connection_id: &str,
    weld_size: f64,
    weld_length: f64,
    f_exx: f64, // electrode strength (ksi or MPa)
    applied_force: f64,
) -> WeldedConnectionResult {
    // Effective throat = 0.707 * weld_size for 45° fillet
    let throat = 0.707 * weld_size;
    let r_n = 0.6 * f_exx * throat * weld_length;
    let design_strength = PHI_WELD * r_n;

    let util = if design_strength > 0.0 {
        applied_force.abs() / design_strength
    } else {
        f64::INFINITY
    };

    let status = if util <= 1.0 {
        ConnectionStatus::Pass
    } else {
        ConnectionStatus::Fail
    };

    WeldedConnectionResult {
        connection_id: connection_id.to_string(),
        weld_type: WeldType::Fillet,
        weld_size,
        weld_length,
        applied_force,
        design_strength,
        util,
        status,
    }
}

/// Check CJP groove weld strength per AISC 360 J2.3
/// R_n = F_y * A_g (base metal strength, since CJP weld is as strong as base metal)
pub fn check_cjp_weld(
    connection_id: &str,
    weld_length: f64,
    plate_thickness: f64,
    fy: f64, // yield strength of base metal
    applied_force: f64,
) -> WeldedConnectionResult {
    let area = weld_length * plate_thickness;
    let r_n = fy * area;
    let design_strength = PHI_WELD * r_n;

    let util = if design_strength > 0.0 {
        applied_force.abs() / design_strength
    } else {
        f64::INFINITY
    };

    let status = if util <= 1.0 {
        ConnectionStatus::Pass
    } else {
        ConnectionStatus::Fail
    };

    WeldedConnectionResult {
        connection_id: connection_id.to_string(),
        weld_type: WeldType::CJP,
        weld_size: plate_thickness,
        weld_length,
        applied_force,
        design_strength,
        util,
        status,
    }
}

/// Connection classification per AISC 360
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConnectionClassification {
    /// Simple connection (transfers shear only, no moment)
    Simple,
    MomentResisting,
    SemiRigid,
}

/// Classify a connection based on its moment capacity and rotational stiffness
pub fn classify_connection(
    moment_capacity: f64,
    rotational_stiffness: f64,
    threshold_stiffness: f64,
) -> ConnectionClassification {
    if moment_capacity.abs() < 1e-6 {
        ConnectionClassification::Simple
    } else if rotational_stiffness >= threshold_stiffness {
        ConnectionClassification::MomentResisting
    } else {
        ConnectionClassification::SemiRigid
    }
}

impl fmt::Display for BoltedConnectionResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Bolted Connection: {}", self.connection_id)?;
        writeln!(f, "  Bolts: {} x {} dia. Grade {:?}", self.bolt_count, self.bolt_diameter, self.bolt_grade)?;
        writeln!(f, "  Applied Shear: {:.2}, Capacity: {:.2} (util: {:.2}%)", self.applied_shear, self.design_shear_capacity, self.shear_util)?;
        writeln!(f, "  Applied Tension: {:.2}, Capacity: {:.2} (util: {:.2}%)", self.applied_tension, self.design_tension_capacity, self.tension_util)?;
        writeln!(f, "  Interaction Ratio: {:.2}%", self.interaction_ratio)?;
        writeln!(f, "  Status: {:?}", self.status)?;
        Ok(())
    }
}

impl fmt::Display for WeldedConnectionResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Welded Connection: {}", self.connection_id)?;
        writeln!(f, "  Type: {:?}, Size: {:.3}, Length: {:.2}", self.weld_type, self.weld_size, self.weld_length)?;
        writeln!(f, "  Applied Force: {:.2}, Capacity: {:.2} (util: {:.2}%)", self.applied_force, self.design_strength, self.util)?;
        writeln!(f, "  Status: {:?}", self.status)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bolt_properties() {
        let bolt = Bolt {
            diameter: 0.75,
            grade: BoltGrade::A325,
            count: 4,
            shear_planes: 1,
        };
        assert!(bolt.area() > 0.0);
        assert!(bolt.nominal_shear() > 0.0);
        assert!(bolt.nominal_tension() > 0.0);
        assert!(bolt.design_shear() > 0.0);
        assert!(bolt.design_tension() > 0.0);
    }

    #[test]
    fn test_bolted_connection_pass() {
        let bolt = Bolt {
            diameter: 0.75,
            grade: BoltGrade::A325,
            count: 4,
            shear_planes: 1,
        };
        let result = check_bolted_connection("C1", &bolt, 10.0, 5.0);
        assert!(result.design_shear_capacity > 0.0);
        assert!(result.design_tension_capacity > 0.0);
        assert!(result.shear_util > 0.0);
        assert!(result.tension_util > 0.0);
        assert!(result.interaction_ratio > 0.0);
    }

    #[test]
    fn test_bolted_connection_fail() {
        let bolt = Bolt {
            diameter: 0.5,
            grade: BoltGrade::A307,
            count: 2,
            shear_planes: 1,
        };
        let result = check_bolted_connection("C2", &bolt, 1000.0, 500.0);
        assert_eq!(result.status, ConnectionStatus::Fail);
    }

    #[test]
    fn test_bearing_check() {
        let result = check_bearing(0.75, 0.5, 1.5, 65.0, 20.0);
        assert!(result.design_bearing > 0.0);
        assert!(result.util > 0.0);
    }

    #[test]
    fn test_tearout_check() {
        let result = check_tearout(1.0, 0.5, 65.0, 15.0);
        assert!(result.design_tearout > 0.0);
        assert!(result.util > 0.0);
    }

    #[test]
    fn test_fillet_weld() {
        let result = check_fillet_weld("W1", 0.25, 10.0, 70.0, 50.0);
        assert!(result.design_strength > 0.0);
        assert!(result.util > 0.0);
    }

    #[test]
    fn test_cjp_weld() {
        let result = check_cjp_weld("W2", 10.0, 0.5, 50.0, 100.0);
        assert!(result.design_strength > 0.0);
        assert!(result.util > 0.0);
    }

    #[test]
    fn test_connection_classification() {
        assert_eq!(
            classify_connection(0.0, 1000.0, 500.0),
            ConnectionClassification::Simple
        );
        assert_eq!(
            classify_connection(50.0, 1000.0, 500.0),
            ConnectionClassification::MomentResisting
        );
        assert_eq!(
            classify_connection(50.0, 100.0, 500.0),
            ConnectionClassification::SemiRigid
        );
    }

    #[test]
    fn test_bolt_grades() {
        assert!(BoltGrade::A490.f_nt() > BoltGrade::A325.f_nt());
        assert!(BoltGrade::A490.f_nv() > BoltGrade::A325.f_nv());
        assert!(BoltGrade::A325.f_nt() > BoltGrade::A307.f_nt());
    }
}
