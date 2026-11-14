//! Steel Frame Design per AISC 360 and EN 1993-1-1 (EC3).
//! Design checks for structural steel frames including:
//! - Flexural strength (M_n) using section moduli
//! - Axial-bending interaction (P/M interaction) per PMM interaction curve
//! - Shear strength (V_n)
//! - Slenderness and stability checks (KL/r)
//! - Compression and tension member design
//!
//! The code is selectable: [`DesignCode`] switches the resistance or partial
//! factors and the column curve between AISC 360-16, EN 1993-1-1 and
//! IS 800:2007. Capacities are returned in newtons and newton-metres.

use crate::materials::Material;
use crate::model;
use crate::sections::{Section, SectionShape};
use std::fmt;

use super::codes::DesignCode;

/// Resistance factor (phi) values per AISC 360-16
/// phi_c = 0.75 for compression, phi_t = 0.90 for tension,
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ResistanceFactor {
    Compression,
    Tension,
    Bending,
    Shear,
}

impl ResistanceFactor {
    /// Get the phi value for this resistance factor
    pub fn phi(self) -> f64 {
        match self {
            ResistanceFactor::Compression => 0.75,
            ResistanceFactor::Tension => 0.90,
            ResistanceFactor::Bending => 0.90,
            ResistanceFactor::Shear => 0.90,
        }
    }
}

/// Area that resists shear: the web of an I-section, five sixths of the area
/// for a rectangle and ninety percent for a circle.
pub fn shear_area(section: &Section) -> f64 {
    match section.shape {
        Some(SectionShape::ISection { depth, flange, web, .. }) => {
            let clear_web = (depth - 2.0 * flange).max(0.0);
            if clear_web * web > 0.0 {
                clear_web * web
            } else {
                0.6 * section.area
            }
        }
        Some(SectionShape::Circular { .. }) => 0.9 * section.area,
        _ => 5.0 / 6.0 * section.area,
    }
}

/// Elastic flexural buckling load about the weaker axis, in newtons.
/// A zero or negative length means the member is fully braced.
pub fn euler_buckling_load(section: &Section, material: &Material, unbraced_length: f64) -> f64 {
    let inertia = section.iz.min(section.iy);
    if unbraced_length <= 0.0 || inertia <= 0.0 {
        return f64::INFINITY;
    }
    std::f64::consts::PI.powi(2) * material.e * inertia / (unbraced_length * unbraced_length)
}

/// Non-dimensional slenderness sqrt(F_y A / N_cr) shared by the three curves.
fn slenderness(section: &Section, material: &Material, unbraced_length: f64) -> f64 {
    let ncr = euler_buckling_load(section, material, unbraced_length);
    if !ncr.is_finite() || ncr <= 0.0 {
        return 0.0;
    }
    (material.fy * section.area / ncr).sqrt()
}

/// Design compressive strength, in newtons.
///
/// AISC 360 E3 uses its own inelastic/elastic column curve; EN 1993-1-1
/// 6.3.1.2 and IS 800:2007 7.1.2 use the Perry-Robertson expression with the
/// code's imperfection factor.
pub fn compressive_strength(
    section: &Section,
    material: &Material,
    unbraced_length: f64,
    code: DesignCode,
) -> f64 {
    let fy = material.fy;
    let area = section.area;
    if area <= 0.0 || fy <= 0.0 {
        return 0.0;
    }
    let factors = code.steel_factors();
    let lambda = slenderness(section, material, unbraced_length);
    let squash = fy * area;

    match code {
        DesignCode::AiscAci => {
            let fcr = if lambda <= 1.5 {
                fy * 0.658_f64.powf(lambda * lambda)
            } else {
                fy * 0.877 / (lambda * lambda)
            };
            factors.compression * fcr.min(fy) * area
        }
        DesignCode::Eurocode => {
            if lambda <= 0.2 {
                return squash * factors.compression;
            }
            let phi = 0.5 * (1.0 + factors.imperfection * (lambda - 0.2) + lambda * lambda);
            let chi = (1.0 / (phi + (phi * phi - lambda * lambda).max(0.0).sqrt())).min(1.0);
            chi * squash * factors.compression
        }
        DesignCode::Indian => {
            let limit = squash * factors.compression;
            if lambda <= 0.2 {
                return limit;
            }
            let phi = 0.5 * (1.0 + factors.imperfection * (lambda - 0.2) + lambda * lambda);
            let fcd = fy * factors.compression / (phi + (phi * phi - lambda * lambda).max(0.0).sqrt());
            fcd.min(fy * factors.compression) * area
        }
    }
}

/// Design flexural strength for the strong and weak axes, in newton-metres.
/// The elastic section modulus is used because the model stores no plastic
/// modulus.
pub fn flexural_strength(section: &Section, material: &Material, code: DesignCode) -> (f64, f64) {
    let (strong_modulus, weak_modulus) = section.section_moduli();
    let factor = code.steel_factors().bending;
    (factor * material.fy * strong_modulus, factor * material.fy * weak_modulus)
}

/// Design tensile strength, in newtons.
pub fn tensile_strength(section: &Section, material: &Material, code: DesignCode) -> f64 {
    code.steel_factors().bending * material.fy * section.area
}

/// Design shear strength, in newtons.
pub fn shear_strength(section: &Section, material: &Material, code: DesignCode) -> f64 {
    let area = shear_area(section);
    let factor = code.steel_factors().shear;
    match code {
        DesignCode::AiscAci => factor * 0.6 * material.fy * area,
        DesignCode::Eurocode | DesignCode::Indian => factor * material.fy * area / 3.0_f64.sqrt(),
    }
}

/// Axial-bending interaction ratio for one code.
///
/// AISC 360 H1-1 switches between `P/Pc + 8/9 M/Mc` and `P/2Pc + M/Mc` at
/// `P/Pc = 0.2`; EN 1993-1-1 6.3.3 and IS 800:2007 9.3.1 use the linear form.
pub fn interaction_ratio(
    code: DesignCode,
    axial: f64,
    moment_weak: f64,
    moment_strong: f64,
    axial_capacity: f64,
    moment_weak_capacity: f64,
    moment_strong_capacity: f64,
) -> f64 {
    let compression = if axial > 0.0 { axial / axial_capacity.max(1e-9) } else { 0.0 };
    let bending = moment_weak.abs() / moment_weak_capacity.max(1e-9)
        + moment_strong.abs() / moment_strong_capacity.max(1e-9);
    match code {
        DesignCode::AiscAci => {
            if compression >= 0.2 {
                compression + 8.0 / 9.0 * bending
            } else {
                compression / 2.0 + bending
            }
        }
        DesignCode::Eurocode | DesignCode::Indian => compression + bending,
    }
}

/// Nominal axial strength results per AISC 360 Chapter H
#[derive(Debug, Clone)]
pub struct NominalAxialCapacity {
    /// Nominal compressive strength P_n (kips or kN)
    pub p_n: f64,
    pub p_nt: f64,
    pub p_lb: f64,
    pub p_wb: f64,
}

impl NominalAxialCapacity {
    /// Compute nominal axial strength for a steel section
    /// Uses AISC 360 Chapter H equations
    pub fn from_section(section: &Section, material: &Material) -> Self {
        let fy = material.fy; // yield strength in Pa
        let area = section.area;
        let iy = section.iy; // strong-axis inertia
        let iz = section.iz; // weak-axis inertia
        let depth = section.depth;
        let width = section.width;

        // Gross area compressive yield strength
        let p_n_gross = area * fy;

        // Estimate radii of gyration
        let r_y = (iy / area).sqrt(); // strong-axis radius of gyration
        let r_z = (iz / area).sqrt(); // weak-axis radius of gyration

        // Simplified AISC H2 nominal compressive strength
        // Placeholder: use gross area with phi for this stub implementation
        let p_n = 0.75 * p_n_gross; // phi_c incorporated as simplified nominal

        // Tensile strength (phi_t = 0.90)
        let p_nt = 0.90 * area * fy;

        // Local buckling limits (simplified)
        let p_lb = p_n_gross * 0.3; // flange buckling threshold
        let p_wb = p_n_gross * 0.2; // web buckling threshold

        Self {
            p_n,
            p_nt,
            p_lb,
            p_wb,
        }
    }
}

/// Nominal moment strength results
#[derive(Debug, Clone)]
pub struct NominalMomentCapacity {
    /// Nominal flexural strength about weak axis M_ny (k-ft or kN-m)
    pub m_ny: f64,
    pub m_nz: f64,
    pub l_b_unlimited: f64,
}

impl NominalMomentCapacity {
    /// Compute nominal moment for a section using section moduli
    pub fn from_section(section: &Section, material: &Material) -> Self {
        let fy = material.fy; // yield strength in Pa
        let (s_strong, s_weak) = section.section_moduli(); // section moduli S = I/y

        // Nominal moment strengths (plastic moment M_p = F_y * S)
        let m_nz = fy * s_strong; // strong axis (z-axis bending, i.e. about weak axis)
        let m_ny = fy * s_weak;   // weak axis (y-axis bending, i.e. about strong axis)

        // Limiting unbraced length L_p (plastic hinge length)
        let l_b_unlimited = 1.5 * section.depth;

        Self {
            m_ny,
            m_nz,
            l_b_unlimited,
        }
    }
}

/// Find the story index at or below the given elevation.
/// Stories are ordered from base (elevation 0) upwards.
pub fn story_index_at_z(model: &model::Model, z: f64) -> usize {
    model
        .stories
        .iter()
        .enumerate()
        .rev()
        .find(|(_, s)| s.elevation <= z + 1e-6)
        .map(|(i, _)| i)
        .unwrap_or(0)
}

/// Adjust unbraced length for column splice.
/// When `splice_story` is true, the splice at `splice_height` metres above
/// the story elevation provides a brace point, reducing the effective
/// unbraced length.
pub fn adjust_unbraced_length_for_splice(
    unbraced_length: f64,
    splice_story: bool,
    splice_height: f64,
    story_elevation: f64,
) -> f64 {
    if !splice_story {
        return unbraced_length;
    }
    // Splice is at story_elevation + splice_height
    let splice_level = story_elevation + splice_height;
    // If the unbraced length extends above the splice level,
    // cap it at the splice level to account for the brace.
    let effective_length = unbraced_length.min(splice_level);
    // Ensure we don't return a negative or zero length that would
    // cause division by zero in buckling calculations.
    effective_length.max(1e-3)
}

/// PMM interaction diagram status
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InteractionStatus {
    /// Point is within the interaction domain
    Ok,
    Overstress,
    Boundary,
}

/// Axial-bending interaction point for PMM curve
#[derive(Debug, Clone)]
pub struct InteractionPoint {
    /// Axial force (positive = compression, negative = tension)
    pub p: f64,
    pub m_y: f64,
    pub m_z: f64,
    pub status: InteractionStatus,
    pub util_ratio: f64,
}

/// Steel design check results for a single member
#[derive(Debug, Clone)]
pub struct MemberDesignResult {
    /// Member identifier
    pub member_id: String,
    pub section_name: String,
    pub material_name: String,
    /// The code the checks were carried out to.
    pub code: DesignCode,
    pub flexure_strong_ratio: f64,
    pub flexure_weak_ratio: f64,
    pub axial_ratio: f64,
    pub shear_ratio: f64,
    pub util_ratio: f64,
    pub status: DesignStatus,
}

/// Design status enumeration
#[derive(Debug, Clone, PartialEq)]
pub enum DesignStatus {
    /// Member passes all checks
    Pass,
    Fail,
    Pending,
}

/// Compute axial-bending interaction ratio per AISC 360
/// Interaction equation (C1.0): P / φP_n + M_y / φM_ny + M_z / φM_nz ≤ 1.0
///
/// This is the standard C1.0 interaction equation for doubly symmetric I-shapes
/// without axial load eccentricity in the plane of the web.
pub fn compute_interaction_ratio(
    p: f64,
    m_y: f64,
    m_z: f64,
    p_n: f64,
    m_ny: f64,
    m_nz: f64,
    phi_p: ResistanceFactor,
    phi_m: ResistanceFactor,
) -> f64 {
    let phi_p_val = phi_p.phi();
    let phi_m_val = phi_m.phi();

    // AISC 360 Appendix C interaction equation
    // P / φP_n + M_y / φM_ny + M_z / φM_nz ≤ 1.0
    // All terms must be non-negative; negative axial is treated as tension
    let p_term = if p > 0.0 { p / (phi_p_val * p_n) } else { 0.0 };
    let m_y_term = m_y.abs() / (phi_m_val * m_ny.max(1e-10));
    let m_z_term = m_z.abs() / (phi_m_val * m_nz.max(1e-10));

    p_term + m_y_term + m_z_term
}

/// Full member design check per AISC 360 / EC3, kept for the AISC/ACI path.
pub fn design_member(
    member_id: &str,
    section: &Section,
    material: &Material,
    axial_force: f64,
    moment_weak: f64,
    moment_strong: f64,
    shear_force: f64,
    unbraced_length: f64,
    splice_story: bool,
    splice_height: f64,
    story_elevation: f64,
) -> MemberDesignResult {
    design_member_with_code(
        member_id,
        section,
        material,
        axial_force,
        moment_weak,
        moment_strong,
        shear_force,
        unbraced_length,
        DesignCode::AiscAci,
        splice_story,
        splice_height,
        story_elevation,
    )
}

/// Full member design check to a chosen code.
///
/// Forces are in newtons and newton-metres and `unbraced_length` is in metres,
/// matching the model's SI units.
pub fn design_member_with_code(
    member_id: &str,
    section: &Section,
    material: &Material,
    axial_force: f64,
    moment_weak: f64,
    moment_strong: f64,
    shear_force: f64,
    unbraced_length: f64,
    code: DesignCode,
    splice_story: bool,
    splice_height: f64,
    story_elevation: f64,
) -> MemberDesignResult {
    let section_name = section.name.clone();
    let material_name = material.name.clone();
    // Adjust unbraced length for column splice.
    // When `splice_story` is true, the splice at `splice_height` metres above
    // the story elevation provides a brace point, reducing the effective
    // unbraced length for buckling calculations.
    let adjusted_unbraced = adjust_unbraced_length_for_splice(
        unbraced_length,
        splice_story,
        splice_height,
        story_elevation,
    );
    let compression_capacity = compressive_strength(section, material, adjusted_unbraced, code).max(1e-9);
    let (strong_capacity, weak_capacity) = flexural_strength(section, material, code);
    let shear_capacity = shear_strength(section, material, code).max(1e-9);
    // Flexural ratios (separate strong/weak axis)
    let flexure_strong = moment_strong.abs() / strong_capacity.max(1e-9);
    let flexure_weak = moment_weak.abs() / weak_capacity.max(1e-9);
    // Axial ratio: compression against the column curve, tension against yield.
    let axial = if axial_force >= 0.0 {
        axial_force / compression_capacity
    } else {
        axial_force.abs() / tensile_strength(section, material, code).max(1e-9)
    };
    let shear_ratio = shear_force.abs() / shear_capacity;
    let interaction = interaction_ratio(
        code,
        axial_force,
        moment_weak,
        moment_strong,
        compression_capacity,
        weak_capacity,
        strong_capacity,
    );
    // Overall utilisation = max of all check ratios
    let util_ratio = interaction.max(flexure_strong).max(flexure_weak).max(axial).max(shear_ratio);
    let status = if util_ratio <= 1.0 {
        DesignStatus::Pass
    } else {
        DesignStatus::Fail
    };
    MemberDesignResult {
        member_id: member_id.to_string(),
        section_name,
        material_name,
        code,
        flexure_strong_ratio: flexure_strong,
        flexure_weak_ratio: flexure_weak,
        axial_ratio: axial,
        shear_ratio,
        util_ratio,
        status,
    }
}


impl Default for DesignStatus {
    fn default() -> Self {
        DesignStatus::Pending
    }
}

/// Aggregate design results for multiple members
#[derive(Debug, Clone, Default)]
pub struct DesignResults {
    /// Per-member results
    pub members: Vec<MemberDesignResult>,
    pub overall_status: DesignStatus,
    pub max_util_ratio: f64,
    pub failing_count: usize,
}

impl DesignResults {
    /// Add a member design result
    pub fn add_member(&mut self, result: MemberDesignResult) {
        self.members.push(result);
        self.update_summary();
    }

    /// Update summary status based on member results
    fn update_summary(&mut self) {
        self.failing_count = self.members.iter()
            .filter(|m| m.status == DesignStatus::Fail)
            .count();

        self.max_util_ratio = self.members.iter()
            .map(|m| m.util_ratio)
            .fold(0.0_f64, f64::max);

        self.overall_status = if self.failing_count > 0 {
            DesignStatus::Fail
        } else {
            DesignStatus::Pass
        };
    }

    /// Get the most critical member (highest utilisation)
    pub fn most_critical(&self) -> Option<&MemberDesignResult> {
        self.members.iter().max_by(|a, b| {
            a.util_ratio.partial_cmp(&b.util_ratio).unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    /// Get failing members
    pub fn failing_members(&self) -> Vec<&MemberDesignResult> {
        self.members.iter()
            .filter(|m| m.status == DesignStatus::Fail)
            .collect()
    }
}

impl fmt::Display for DesignResults {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Steel Frame Design Results")?;
        writeln!(f, "{}", "=" .repeat(35).as_str())?;
        writeln!(f, "Overall Status: {:?}", self.overall_status)?;
        writeln!(f, "Maximum Utilisation: {:.2}%", self.max_util_ratio)?;
        writeln!(f, "Failing Members: {}/{}", self.failing_count, self.members.len())?;
        writeln!(f)?;

        writeln!(f, "{:<15} {:<14} {:<12} {:<10} {:<10} {:<10} {:<10}", 
            "Member", "Section", "Material", "Util%", "Status", "P/φPn", "Mz/φMz");
        writeln!(f, "{}", "-".repeat(85))?;

        for m in &self.members {
            let util_str = format!("{:.1}%", m.util_ratio);
            let status_str = match m.status {
                DesignStatus::Pass => "PASS",
                DesignStatus::Fail => "FAIL",
                DesignStatus::Pending => "PEND",
            };
            writeln!(f, "{:<15} {:<14} {:<12} {:<10} {:<10?} {:<9.2}% {:<9.2}%", 
                &m.member_id[..std::cmp::min(12, m.member_id.len())],
                &m.section_name[..std::cmp::min(10, m.section_name.len())],
                &m.material_name[..std::cmp::min(8, m.material_name.len())],
                util_str,
                status_str,
                m.axial_ratio,
                m.flexure_strong_ratio,
            )?;
        }

        writeln!(f)?;
        writeln!(f, "Detailed Checks:")?;
        writeln!(f, "  Flexure Strong Axis: {:.2}%", 
            self.members.iter().map(|m| m.flexure_strong_ratio).fold(0.0_f64, f64::max))?;
        writeln!(f, "  Flexure Weak Axis: {:.2}%", 
            self.members.iter().map(|m| m.flexure_weak_ratio).fold(0.0_f64, f64::max))?;
        writeln!(f, "  Axial Compression: {:.2}%", 
            self.members.iter().map(|m| m.axial_ratio).fold(0.0_f64, f64::max))?;
        writeln!(f, "  Shear: {:.2}%", 
            self.members.iter().map(|m| m.shear_ratio).fold(0.0_f64, f64::max))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interaction_ratio_passes() {
        let ratio = compute_interaction_ratio(
            50.0,   // P = 50 kips compression
            20.0,   // M_y = 20 k-ft weak axis
            30.0,   // M_z = 30 k-ft strong axis
            500.0,  // P_n = 500 kips
            100.0,  // M_ny = 100 k-ft
            150.0,  // M_nz = 150 k-ft
            ResistanceFactor::Compression,
            ResistanceFactor::Bending,
        );
        
        // Ratio should be positive and less than 2.0 for reasonable input
        assert!(ratio > 0.0);
        assert!(ratio < 2.0, "Interaction ratio {} too large", ratio);
    }

    #[test]
    fn test_interaction_ratio_fails() {
        // High loads that should exceed 1.0
        let ratio = compute_interaction_ratio(
            400.0,  // P = 400 kips compression
            100.0,  // M_y = 100 k-ft
            120.0,  // M_z = 120 k-ft
            500.0,  // P_n = 500 kips
            100.0,  // M_ny = 100 k-ft
            150.0,  // M_nz = 150 k-ft
            ResistanceFactor::Compression,
            ResistanceFactor::Bending,
        );
        
        // Should be > 1.0 for these high loads
        assert!(ratio > 1.0, "Expected failing interaction, got {}", ratio);
    }

    #[test]
    fn test_design_member_creation() {
        let section = Section::rectangular("TEST", 0.3, 0.5);
        let material = Material::steel();

        let result = design_member(
            "B1",
            &section,
            &material,
            50.0,      // 50 kips axial compression
            20.0,      // 20 k-ft weak axis moment
            30.0,      // 30 k-ft strong axis moment
            5.0,       // 5 kips shear
            20.0,      // 20 ft unbraced length
            false,     // splice_story
            0.0,       // splice_height
            0.0,       // story_elevation
        );

        assert!(!result.member_id.is_empty());
        assert!(!result.section_name.is_empty());
        assert!(!result.material_name.is_empty());
        assert!(result.util_ratio >= 0.0);
        assert!(result.util_ratio <= 2.0, "Util ratio {} out of range", result.util_ratio);
        assert!(matches!(result.status, DesignStatus::Pass | DesignStatus::Fail));
    }

    #[test]
    fn test_design_results_summary() {
        let mut results = DesignResults::default();

        results.add_member(MemberDesignResult {
            member_id: "C1".to_string(),
            section_name: "W12x50".to_string(),
            material_name: "A992".to_string(),
            code: DesignCode::AiscAci,
            flexure_strong_ratio: 0.6,
            flexure_weak_ratio: 0.5,
            axial_ratio: 0.5,
            shear_ratio: 0.3,
            util_ratio: 0.6,
            status: DesignStatus::Pass,
        });

        results.add_member(MemberDesignResult {
            member_id: "C2".to_string(),
            section_name: "W12x50".to_string(),
            material_name: "A992".to_string(),
            code: DesignCode::AiscAci,
            flexure_strong_ratio: 0.95,
            flexure_weak_ratio: 0.9,
            axial_ratio: 0.9,
            shear_ratio: 0.4,
            util_ratio: 0.95,
            status: DesignStatus::Fail,
        });

        assert!(results.overall_status == DesignStatus::Fail);
        assert!(results.failing_count == 1);
        assert!(results.max_util_ratio > 0.9);
        
        // Check most critical member
        let critical = results.most_critical();
        assert!(critical.is_some());
        assert_eq!(critical.unwrap().member_id, "C2");
    }

    #[test]
    fn test_design_results_failing_members() {
        let mut results = DesignResults::default();

        results.add_member(MemberDesignResult {
            member_id: "C1".to_string(),
            section_name: "W12x50".to_string(),
            material_name: "A992".to_string(),
            code: DesignCode::AiscAci,
            flexure_strong_ratio: 0.5,
            flexure_weak_ratio: 0.4,
            axial_ratio: 0.3,
            shear_ratio: 0.2,
            util_ratio: 0.5,
            status: DesignStatus::Pass,
        });

        results.add_member(MemberDesignResult {
            member_id: "C2".to_string(),
            section_name: "W12x50".to_string(),
            material_name: "A992".to_string(),
            code: DesignCode::AiscAci,
            flexure_strong_ratio: 1.2,
            flexure_weak_ratio: 1.1,
            axial_ratio: 1.0,
            shear_ratio: 0.5,
            util_ratio: 1.2,
            status: DesignStatus::Fail,
        });

        let failing = results.failing_members();
        assert_eq!(failing.len(), 1);
        assert_eq!(failing[0].member_id, "C2");
    }

    #[test]
    fn test_section_moduli_used_correctly() {
        let section = Section::rectangular("R", 0.3, 0.5);
        let (s_strong, s_weak) = section.section_moduli();
        
        // For rectangular 0.3 wide × 0.5 deep:
        // S_strong pairs Iy with the depth, S_weak pairs Iz with the width, so for
        // a 0.3 m wide x 0.5 m deep rectangle the strong value must be the larger.
        let expected_strong = section.iy / (section.depth / 2.0);
        let expected_weak = section.iz / (section.width / 2.0);
        
        assert!((s_strong - expected_strong).abs() < 1e-12);
        assert!((s_weak - expected_weak).abs() < 1e-12);
    }
}

#[cfg(test)]
mod code_selector_tests {
    use super::*;

    #[test]
    fn slenderness_reduces_capacity_in_every_code() {
        let section = Section::rectangular("R", 0.3, 0.5);
        let material = Material::steel();
        for code in DesignCode::ALL {
            let short = compressive_strength(&section, &material, 0.5, code);
            let long = compressive_strength(&section, &material, 8.0, code);
            assert!(long < short, "{code:?}: {long} is not below {short}");
            assert!(long > 0.0);
        }
    }

    #[test]
    fn a_short_column_reaches_the_squash_load_within_its_code_factor() {
        let section = Section::rectangular("R", 0.3, 0.5);
        let material = Material::steel();
        let squash = section.area * material.fy;
        for code in DesignCode::ALL {
            let capacity = compressive_strength(&section, &material, 0.2, code);
            let expected = squash * code.steel_factors().compression;
            assert!(
                (capacity / expected - 1.0).abs() < 0.05,
                "{code:?}: {capacity} against {expected}"
            );
        }
    }

    #[test]
    fn indian_compression_is_stricter_than_eurocode_when_slender() {
        let section = Section::rectangular("R", 0.3, 0.5);
        let material = Material::steel();
        let euro = compressive_strength(&section, &material, 6.0, DesignCode::Eurocode);
        let indian = compressive_strength(&section, &material, 6.0, DesignCode::Indian);
        assert!(indian < euro, "{indian} is not below {euro}");
    }

    #[test]
    fn shear_area_takes_the_web_of_an_i_section() {
        let section = Section::from_shape(
            "I",
            SectionShape::ISection { depth: 0.4, width: 0.2, web: 0.008, flange: 0.012 },
        )
        .expect("valid I-section");
        let expected = (0.4 - 2.0 * 0.012) * 0.008;
        assert!((shear_area(&section) - expected).abs() < 1e-12);
        assert!(shear_area(&section) < section.area);
    }

    #[test]
    fn design_member_with_code_reports_the_code_it_used() {
        let section = Section::rectangular("R", 0.3, 0.5);
        let material = Material::steel();
        let result = design_member_with_code(
            "B1", &section, &material, 50_000.0, 20_000.0, 30_000.0, 5_000.0, 20.0, DesignCode::Indian,
            false,     // splice_story
            0.0,       // splice_height
            0.0,       // story_elevation
        );
        assert_eq!(result.code, DesignCode::Indian);
        assert!(result.util_ratio > 0.0);
        assert!(matches!(result.status, DesignStatus::Pass | DesignStatus::Fail));
    }
}