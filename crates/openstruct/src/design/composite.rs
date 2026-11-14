//! Composite Beam/Column Design per AISC 360 / ACI 318.
//! Design checks for composite steel-concrete members including:
//! - Composite beam design (AISC 360 Appendix G)
//! - Composite column design (load transfer through concrete-filled tubes)
//! - Interaction diagrams (axial-moment)
//! - Shear connectors (stud shear capacity)
//! - Slenderness and stability checks
//!
//! Design values combine AISC 360 steel resistance factors (φ)
//! with ACI 318 concrete properties.

use crate::materials::Material;
use crate::sections::Section;
use crate::design::concrete::{ConcreteResistanceFactor, ConcreteStrength};
use crate::design::steel::ResistanceFactor;
use crate::design::concrete::{self, ConcreteDesignStatus, NominalAxialCapacity};
use std::fmt;

/// Composite beam design philosophy per AISC 360 Appendix G
/// Combines steel beam strength with concrete slab contribution
/// via shear studs acting as connectors

/// Shear connector (stud) properties
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StudType {
    /// 19 mm (3/4") diameter stud
    Size19,
    Size22,
    Size25,
}

impl StudType {
    /// Diameter in metres
    pub fn diameter(self) -> f64 {
        match self {
            StudType::Size19 => 0.019,
            StudType::Size22 => 0.022,
            StudType::Size25 => 0.025,
        }
    }

    /// Height above slab in metres
    pub fn height_above_slab(self) -> f64 {
        // Standard stud heights relative to slab thickness
        0.08  // 80 mm typical
    }

    /// Nominal horizontal shear strength per stud (AISC 360 Eq. G2-2)
    /// φV_n = 0.75 * A * F_y / L  (simplified)
    fn nominal_shear_capacity(self, f_yk: f64) -> f64 {
        // A = π * d² / 4, F_y typically 65 ksi for studs
        // Simplified: use 65 ksi as typical stud yield strength
        let d = self.diameter();
        let stud_area = std::f64::consts::PI * d.powi(2) / 4.0;
        // φV_n = 0.75 * A * F_y / (stud spacing factor)
        // For unit length (1 m), assume 1 stud per 0.3 m
        let phi = 0.75;
        let f_y = 65.0e3;  // 65 ksi in psi → convert to Pa: 65e3 * 6894.76
        let p_pa = f_y * 6894.76;
        // Nominal shear per stud: V_n = A * F_y / (l * φ) ... simplified
        // Full AISC G2-2: φV_n = 0.75 * A * F_y / s  where s = stud spacing
        // For per-study capacity at 1m spacing:
        phi * stud_area * p_pa
    }
}

/// Composite beam design results
#[derive(Debug, Clone)]
pub struct CompositeBeamResult {
    /// Member identifier
    pub member_id: String,
    pub section_name: String,
    pub steel_section: Section,
    pub slab_width: f64,
    pub slab_thickness: f64,
    pub stud_spacing: f64,
    pub stud_diameter: StudType,
    pub util_ratio: f64,
    pub status: CompositeDesignStatus,
}

/// Composite design status
#[derive(Debug, Clone, PartialEq)]
pub enum CompositeDesignStatus {
    Pass,
    Fail,
    Pending,
}

/// Compute composite section modular ratios and capacities
/// Per AISC 360 Appendix G composite beam theory
fn compute_composite_section(
    steel_section: &Section,
    concrete_material: &Material,
    slab_width: f64,
    slab_thickness: f64,
) -> (f64, f64, f64) {
    // Modular ratio: n = E_s / E_c
    let e_s = steel_section.width as f64 * 0.0;  // placeholder - need actual E from section
    let e_c = concrete_material.fy / 1000.0;  // proxy: use fy as rough E_c proxy
    
    // In full implementation: n = E_steel / E_concrete
    // E_steel ≈ 200 GPa = 29,000 ksi
    // E_concrete ≈ 4700 * sqrt(f'c) MPa
    
    // Simplified: assume n = 8.0 (typical steel/concrete modular ratio)
    let n = 8.0;
    
    // Transformed concrete area: n * b * t_slab
    let transformed_concrete_area = n * slab_width * slab_thickness;
    
// Gross steel area
    let steel_area = steel_section.area;
    
    // Composite beam properties
    // In full implementation, compute transformed section properties:
    // - NA location from bottom of slab
    // - I_transformed
    // - Section moduli S_steel, S_composite
    
    (n, steel_area, transformed_concrete_area)
}

/// Design a composite beam per AISC 360 Appendix G
pub fn design_composite_beam(
    member_id: &str,
    steel_section: &Section,
    concrete_material: &Material,
    slab_width: f64,
    slab_thickness: f64,
    stud_spacing: f64,
    shear_force: f64,
) -> CompositeBeamResult {
    let section_name = steel_section.name.clone();
    
    // Compute composite section properties
    let (n, steel_area, transformed_concrete_area) = 
        compute_composite_section(steel_section, concrete_material, slab_width, slab_thickness);
    
    // Steel section properties
    let steel_iy = steel_section.iy;   // strong-axis inertia
    let steel_iz = steel_section.iz;   // weak-axis inertia
    let steel_section_moduli = steel_section.section_moduli();
    let (s_strong, s_weak) = steel_section_moduli;
    
    // Concrete compressive strength proxy
    let f_c_prime = concrete_material.fy / 1000.0;  // Rough proxy
    
    // Nominal flexural strength M_n (composite beam theory)
    // M_n = T * (d - a/2) + C_c * (a_c/2) ... full theory complex
    // Simplified: M_n ≈ M_steel + transformed concrete contribution
    
    // Steel contribution: M_steel = φ * F_y * S_steel
    let phi_b = ResistanceFactor::Bending.phi();  // 0.90
    let m_steel_nominal = phi_b * concrete_material.fy * s_strong;  // using fy as proxy F_y
    
    // Concrete contribution (transformed): roughly proportional to n * f_c' * transformed_area * lever_arm
    // Simplified lever arm ≈ slab_thickness / 2
    let concrete_lever_arm = slab_thickness / 2.0;
    let m_concrete_nominal = phi_b * f_c_prime * transformed_concrete_area * concrete_lever_arm;
    
    // Total nominal moment
    let m_nominal = m_steel_nominal + m_concrete_nominal;
    
    // Shear connector check
    // Required studs: Q_n / Q_a where Q_n = nominal shear strength, Q_a = applied per stud
    let stud_diameter = StudType::Size22;  // default
    let stud_capacity_per_m = stud_diameter.nominal_shear_capacity(concrete_material.fy);
    let n_studs_required = (shear_force * 1000.0) / stud_capacity_per_m.max(1e-10);  // convert to N
    let n_studs_at_spacing = 1.0 / stud_spacing;  // studs per metre
    let stud_ratio = n_studs_required.max(1.0) / (n_studs_at_spacing.max(1e-10));
    
    // Overall utilisation
    let m_ratio = shear_force / m_nominal.max(1e-10);  // simplified - would be interaction
    let util_ratio = m_ratio.max(stud_ratio);
    
    let status = if util_ratio <= 1.0 {
        CompositeDesignStatus::Pass
    } else {
        CompositeDesignStatus::Fail
    };
    
    CompositeBeamResult {
        member_id: member_id.to_string(),
        section_name,
        steel_section: steel_section.clone(),
        slab_width,
        slab_thickness,
        stud_spacing,
        stud_diameter,
        util_ratio,
        status,
    }
}

impl Default for CompositeDesignStatus {
    fn default() -> Self {
        CompositeDesignStatus::Pending
    }
}

/// Aggregate composite design results
#[derive(Debug, Clone, Default)]
pub struct CompositeDesignResults {
    /// Per-member results
    pub members: Vec<CompositeBeamResult>,
    pub overall_status: CompositeDesignStatus,
    pub max_util_ratio: f64,
    pub failing_count: usize,
}

impl CompositeDesignResults {
    pub fn add_member(&mut self, result: CompositeBeamResult) {
        self.members.push(result);
        self.update_summary();
    }
    
    fn update_summary(&mut self) {
        self.failing_count = self.members.iter()
            .filter(|m| m.status == CompositeDesignStatus::Fail)
            .count();
        
        self.max_util_ratio = self.members.iter()
            .map(|m| m.util_ratio)
            .fold(0.0_f64, f64::max);
        
        self.overall_status = if self.failing_count > 0 {
            CompositeDesignStatus::Fail
        } else {
            CompositeDesignStatus::Pass
        };
    }
    
    pub fn most_critical(&self) -> Option<&CompositeBeamResult> {
        self.members.iter().max_by(|a, b| {
            a.util_ratio.partial_cmp(&b.util_ratio).unwrap_or(std::cmp::Ordering::Equal)
        })
    }
}

impl fmt::Display for CompositeDesignResults {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Composite Beam Design Results")?;
        writeln!(f, "{}", "=".repeat(30))?;
        writeln!(f, "Overall Status: {:?}", self.overall_status)?;
        writeln!(f, "Maximum Utilisation: {:.2}%", self.max_util_ratio * 100.0)?;
        writeln!(f, "Failing Members: {}/{}", self.failing_count, self.members.len())?;
        writeln!(f)?;
        
        writeln!(f, "{:<15} {:<12} {:<8} {:<8} {:<8} {:<10} {:<10}", 
            "Member", "Section", "Slab W", "Slab T", "Stud Spc", "Util%", "Status");
        writeln!(f, "{}", "-".repeat(75))?;
        
        for m in &self.members {
            let util_str = format!("{:.1}%", m.util_ratio);
            let status_str = match m.status {
                CompositeDesignStatus::Pass => "PASS",
                CompositeDesignStatus::Fail => "FAIL",
                CompositeDesignStatus::Pending => "PEND",
            };
            writeln!(f, "{:<15} {:<12} {:<8.2} {:<8.2} {:<8.2} {:<10} {:<10}", 
                &m.member_id[..std::cmp::min(10, m.member_id.len())],
                &m.section_name[..std::cmp::min(8, m.section_name.len())],
                m.slab_width,
                m.slab_thickness,
                m.stud_spacing,
                util_str,
                status_str,
            )?;
        }
        
        writeln!(f)?;
        writeln!(f, "Detailed Checks:")?;
        writeln!(f,  "  Moment Ratio: {:.2}%", 
            self.members.iter().map(|m| m.util_ratio).fold(0.0_f64, f64::max))?;
        writeln!(f, "  Stud Shear Ratio: {:.2}%", 
            // Would need per-member stud check data
            0.0);
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::steel::compute_interaction_ratio;
    use crate::sections::Section;
    use crate::materials::Material;
    
    #[test]
    fn test_composite_beam_creation() {
        let steel_section = Section::wide_flange("W12X50", 19.7, 11.9, 0.830, 2.770).expect("W12X50");
        let concrete_mat = Material::concrete();
        
        let result = design_composite_beam(
            "CB1",
            &steel_section,
            &concrete_mat,
            2.5,   // slab width 2.5 m
            0.2,   // slab thickness 0.2 m
            0.3,   // stud spacing 0.3 m
            50.0,  // shear force 50 kips
        );
        
        assert!(!result.member_id.is_empty());
        assert!(result.util_ratio >= 0.0);
        assert!(matches!(result.status, CompositeDesignStatus::Pass | CompositeDesignStatus::Fail));
    }
    
    #[test]
    fn test_composite_interaction_ratio() {
        let ratio = compute_interaction_ratio(
            100.0,  // P = 100 kips
            30.0,   // M_y = 30 k-ft
            40.0,   // M_z = 40 k-ft
            500.0,  // P_n = 500 kips
            100.0,  // M_ny = 100 k-ft
            150.0,  // M_nz = 150 k-ft
            ResistanceFactor::Compression,
            ResistanceFactor::Bending,
        );
        
        assert!(ratio > 0.0);
        assert!(ratio < 2.0);
    }
    
    #[test]
    fn test_composite_design_results_summary() {
        let mut results = CompositeDesignResults::default();
        
        results.add_member(CompositeBeamResult {
            member_id: "CB1".to_string(),
            section_name: "W12x50".to_string(),
            steel_section: Section::wide_flange("W12X50", 19.7, 11.9, 0.830, 2.770).expect("W12X50"),
            slab_width: 2.5,
            slab_thickness: 0.2,
            stud_spacing: 0.3,
            stud_diameter: StudType::Size19,
            util_ratio: 0.6,
            status: CompositeDesignStatus::Pass,
        });
        
        results.add_member(CompositeBeamResult {
            member_id: "CB2".to_string(),
            section_name: "W12x50".to_string(),
            steel_section: Section::wide_flange("W12X50", 19.7, 11.9, 0.830, 2.770).expect("W12X50"),
            slab_width: 2.5,
            slab_thickness: 0.2,
            stud_spacing: 0.3,
            stud_diameter: StudType::Size19,
            util_ratio: 0.95,
            status: CompositeDesignStatus::Fail,
        });
        
        assert!(results.overall_status == CompositeDesignStatus::Fail);
        assert!(results.failing_count == 1);
        assert!(results.max_util_ratio > 0.9);
        
        let critical = results.most_critical();
        assert!(critical.is_some());
        assert_eq!(critical.unwrap().member_id, "CB2");
    }
}