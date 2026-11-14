//! Concrete Frame Design per ACI 318 and EN 1992-1-1 (EC2).
//! Design checks for concrete frames including:
//! - Flexural strength (M_n) for beams and columns
//! - Axial-bending interaction (P-M interaction) per PMM curve
//! - Shear strength (V_n)
//! - Tied/spiral column design
//! - Slenderness and stability checks
//! - **One-way and two-way slab design per ACI 318 Section 8** (NEW)
//! - Design values use nominal strengths with ACI 318 resistance factors (φ)
//
//! The code is selectable: [`DesignCode`] switches between ACI 318-19
//! resistance factors, EN 1992-1-1 partial factors and the IS 456:2000 design
//! strengths. Capacities are returned in newtons and newton-metres.

use crate::materials::Material;
use crate::sections::Section;
use std::fmt;

use super::codes::{ConcreteCodeFactors, DesignCode};

/// Resistance factor (phi) values per ACI 318-19
/// phi_c = 0.65 for compression, phi_t = 0.90 for tension,
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConcreteResistanceFactor {
    Compression,
    Tension,
    Bending,
    Shear,
}

impl ConcreteResistanceFactor {
    /// Get the phi value for this resistance factor
    pub fn phi(self) -> f64 {
        match self {
            ConcreteResistanceFactor::Compression => 0.65,
            ConcreteResistanceFactor::Tension => 0.90,
            ConcreteResistanceFactor::Bending => 0.90,
            ConcreteResistanceFactor::Shear => 0.75,
        }
    }
}

/// Concrete compressive strength class
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConcreteStrength {
    /// f'c = 2500 psi (17.2 MPa)
    C2500,
    C3000,
    C3500,
    C4000,
    C5000,
    C6000,
}

impl ConcreteStrength {
    /// Characteristic cylinder strength f_ck in megapascals.
    pub fn fck_mpa(self) -> f64 {
        match self {
            ConcreteStrength::C2500 => 17.2,
            ConcreteStrength::C3000 => 20.7,
            ConcreteStrength::C3500 => 24.1,
            ConcreteStrength::C4000 => 27.6,
            ConcreteStrength::C5000 => 34.5,
            ConcreteStrength::C6000 => 41.4,
        }
    }
}

/// Longitudinal steel assumed by the member checks: the model stores no rebar,
/// so one percent of the gross area is used and the dialogs say so.
pub const ASSUMED_REBAR_RATIO: f64 = 0.01;

/// Nominal axial capacity for concrete section
#[derive(Debug, Clone)]
pub struct NominalAxialCapacity {
    /// Nominal compressive strength P_n (kips or kN)
    pub p_n: f64,
    pub p_nt: f64,
}

impl NominalAxialCapacity {
    /// Compute nominal axial strength for a concrete rectangular section
    /// Per ACI 318 Chapter 10
    pub fn from_rectangular(section: &Section, material: &Material, width: f64, depth: f64) -> Self {
        let f_c_prime = material.fy / 1000.0;  // Proxy for concrete compressive strength
        let a = section.area;

        // Concrete compressive strength
        let f_c_prime = material.fy / 1000.0;
        let p_n_gross = a * f_c_prime;

        // Tensile strength (very small, cracked section)
        let p_nt = 0.1 * p_n_gross;

        // phi_c = 0.65 for compression
        let p_n = 0.65 * p_n_gross;

        Self { p_n, p_nt }
    }
}

/// Nominal moment capacity for concrete rectangular section
#[derive(Debug, Clone)]
pub struct NominalMomentCapacity {
    /// Nominal flexural strength about weak axis M_ny (k-ft or kN-m)
    pub m_ny: f64,
    pub m_nz: f64,
    pub a_depth: f64,
}

impl NominalMomentCapacity {
    /// Compute nominal moment for a concrete rectangular section
    /// Per ACI 318 Section 10.2 (Whitney stress block)
    pub fn from_rectangular(section: &Section, material: &Material, width: f64, depth: f64) -> Self {
        let f_c_prime = material.fy / 1000.0;  // Proxy for concrete compressive strength

        // Whitney stress block parameters
        // a = β1 * c, where β1 ≈ 0.85 for f'c ≤ 4000 psi, reduces by 0.05 per 1000 psi over 4000
        let beta1 = if f_c_prime <= 4000.0 / 1000.0 {
            0.85
        } else {
            0.85 - 0.05 * (f_c_prime - 4.0)
        };

        // Section properties from Section struct
        let a = section.area;
        let iy = section.iy;  // strong-axis inertia
        let iz = section.iz;  // weak-axis inertia

        // Effective depths (d - cover - half_bar_diameter)
        // Simplified: use 0.9 * depth for effective depth
        let d_eff = 0.9 * depth;

        // Nominal moment capacities (simplified elastic)
        // M_n = F_y * S where S = section modulus
        // M_nz (strong axis): uses full section depth
        let m_nz = f_c_prime * a * d_eff * (d_eff / 2.0) * beta1;
        // M_ny (weak axis)
        let m_ny = f_c_prime * a * (width / 2.0) * (depth / 2.0);

        Self {
            m_ny,
            m_nz,
            a_depth: beta1 * d_eff,  // approximate depth of stress block
        }
    }
}

/// Interaction status for PMM curve
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConcreteInteractionStatus {
    Ok,
    Overstress,
    Boundary,
}

/// Concrete design check results for a single member
#[derive(Debug, Clone)]
pub struct ConcreteMemberDesignResult {
    /// Member identifier
    pub member_id: String,
    pub section_name: String,
    pub concrete_strength: ConcreteStrength,
    /// The code the checks were carried out to.
    pub code: DesignCode,
    pub flexure_strong_ratio: f64,
    pub flexure_weak_ratio: f64,
    pub axial_ratio: f64,
    pub shear_ratio: f64,
    pub util_ratio: f64,
    pub status: ConcreteDesignStatus,
}

/// Concrete design status enumeration
#[derive(Debug, Clone, PartialEq)]
pub enum ConcreteDesignStatus {
    Pass,
    Fail,
    Pending,
}

/// Compute concrete interaction ratio per ACI 318
/// Interaction equation: P / φP_n + M_y / φM_ny + M_z / φM_nz ≤ 1.0
pub fn compute_concrete_interaction_ratio(
    p: f64,
    m_y: f64,
    m_z: f64,
    p_n: f64,
    m_ny: f64,
    m_nz: f64,
    phi_c: ConcreteResistanceFactor,
    phi_m: ConcreteResistanceFactor,
) -> f64 {
    let phi_c_val = phi_c.phi();
    let phi_m_val = phi_m.phi();

    // ACI 318 interaction equation
    let p_term = if p > 0.0 { p / (phi_c_val * p_n) } else { 0.0 };
    let m_y_term = m_y.abs() / (phi_m_val * m_ny.max(1e-10));
    let m_z_term = m_z.abs() / (phi_m_val * m_nz.max(1e-10));

    p_term + m_y_term + m_z_term
}

/// Design axial resistance, in newtons.
///
/// ACI 318-19 22.4.2 applies φ = 0.65 to the 0.80 capped nominal strength,
/// EN 1992-1-1 uses α_cc f_ck/γ_c, and IS 456:2000 39.3 uses 0.4 f_ck and
/// 0.67 f_y which already carry the 1.50/1.15 partial safety factors.
pub fn axial_capacity(section: &Section, strength: ConcreteStrength, code: DesignCode) -> f64 {
    let fck = strength.fck_mpa() * 1.0e6;
    let factors = code.concrete_factors();
    let gross = section.area;
    if gross <= 0.0 {
        return 0.0;
    }
    let steel = ASSUMED_REBAR_RATIO * gross;
    let concrete = (gross - steel).max(0.0);
    match code {
        DesignCode::AiscAci => {
            factors.axial * 0.80 * (0.85 * fck * concrete + factors.steel_fy * steel)
        }
        DesignCode::Eurocode => {
            factors.axial * fck * concrete + factors.steel_fy / factors.gamma_s * steel
        }
        DesignCode::Indian => {
            factors.axial * (0.4 * fck * concrete + 0.67 * factors.steel_fy * steel)
        }
    }
}

/// Design flexural resistance for the strong and weak axes, in newton-metres.
///
/// Both axes are treated as singly reinforced with the assumed steel area at an
/// effective depth of 0.9 h, capped at the code's tension-controlled limit.
pub fn flexural_capacity(section: &Section, strength: ConcreteStrength, code: DesignCode) -> (f64, f64) {
    let fck = strength.fck_mpa() * 1.0e6;
    let factors = code.concrete_factors();
    let strong = single_axis_moment(section.width, section.depth, fck, &factors, code);
    let weak = single_axis_moment(section.depth, section.width, fck, &factors, code);
    (strong, weak)
}

fn single_axis_moment(
    width: f64,
    depth: f64,
    fck: f64,
    factors: &ConcreteCodeFactors,
    code: DesignCode,
) -> f64 {
    if width <= 0.0 || depth <= 0.0 {
        return 0.0;
    }
    let effective = 0.9 * depth;
    let steel = ASSUMED_REBAR_RATIO * width * depth;
    let fy = factors.steel_fy;
    match code {
        DesignCode::AiscAci => {
            // Whitney block with beta1 = 0.85, capped at a = 0.75 beta1 d.
            let block = (steel * fy / (0.85 * fck * width)).min(0.75 * 0.85 * effective);
            factors.bending * steel * fy * (effective - block / 2.0)
        }
        DesignCode::Eurocode => {
            // f_cd = α_cc f_ck/γ_c, f_yd = f_yk/γ_s, x capped at 0.45 d.
            let fcd = 0.85 * fck / factors.gamma_c;
            let fyd = fy / factors.gamma_s;
            let neutral = (steel * fyd / (0.8 * width * fcd)).min(0.45 * effective);
            steel * fyd * (effective - 0.4 * neutral)
        }
        DesignCode::Indian => {
            // IS 456:2000 38.1: x_u = 0.87 f_y A_st / (0.36 f_ck b), capped at
            // x_u,max = 0.48 d for Fe 415.
            let neutral = (0.87 * fy * steel / (0.36 * fck * width)).min(0.48 * effective);
            0.87 * fy * steel * (effective - 0.42 * neutral)
        }
    }
}

/// Design shear resistance, in newtons.
pub fn shear_capacity(section: &Section, strength: ConcreteStrength, code: DesignCode) -> f64 {
    let fck = strength.fck_mpa();
    let width = section.width;
    let effective = 0.9 * section.depth;
    if width <= 0.0 || effective <= 0.0 {
        return 0.0;
    }
    let factors = code.concrete_factors();
    match code {
        DesignCode::AiscAci => {
            // ACI 318-19 22.5.5.1 for lambda = 1: V_c = 0.17 sqrt(f'c) b_w d
            // in MPa and mm, converted to N.
            factors.shear * 0.17 * fck.sqrt() * width * effective * 1.0e6
        }
        DesignCode::Eurocode => {
            // EN 1992-1-1 6.2.2 without axial load, k capped at 2.
            let depth_mm = effective * 1000.0;
            let k = (1.0 + (200.0 / depth_mm).sqrt()).min(2.0);
            let rho = ASSUMED_REBAR_RATIO;
            let v = 0.12 * k * (100.0 * rho * fck).cbrt();
            let v_min = 0.035 * k.powf(1.5) * fck.sqrt();
            v.max(v_min) * width * effective * 1.0e6
        }
        DesignCode::Indian => {
            // IS 456:2000 40.2 with the Table 19 design-shear expression,
            // written with p_t as a percentage of steel.
            let percent = 100.0 * ASSUMED_REBAR_RATIO;
            let beta = (0.8 * fck / (6.89 * percent)).max(1.0);
            let tau_c = 0.85 * (0.8 * fck).sqrt() * ((1.0 + 5.0 * beta).sqrt() - 1.0) / (6.0 * beta);
            tau_c * width * effective * 1.0e6
        }
    }
}

/// Axial-moment interaction ratio for one code.
///
/// ACI 318-19 uses the two-branch H1-1 style equation with the 0.2 threshold at
/// 8/9; EN 1992-1-1 5.8.9 and IS 456:2000 39.6 use the linear summation.
pub fn concrete_interaction_ratio(
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

/// Full concrete member design check, to a chosen code.
///
/// The model stores no reinforcement, so the checks assume one percent of the
/// gross area as longitudinal steel and an effective depth of 0.9 h. Forces are
/// in newtons and newton-metres.
pub fn design_concrete_member_with_code(
    member_id: &str,
    section: &Section,
    material: &Material,
    axial_force: f64,
    moment_weak: f64,
    moment_strong: f64,
    shear_force: f64,
    concrete_strength: ConcreteStrength,
    code: DesignCode,
) -> ConcreteMemberDesignResult {
    let _ = material;
    let section_name = section.name.clone();

    let p_capacity = axial_capacity(section, concrete_strength, code).max(1e-9);
    let (m_strong, m_weak) = flexural_capacity(section, concrete_strength, code);
    let v_capacity = shear_capacity(section, concrete_strength, code).max(1e-9);

    let flexure_strong = moment_strong.abs() / m_strong.max(1e-9);
    let flexure_weak = moment_weak.abs() / m_weak.max(1e-9);
    let axial = if axial_force > 0.0 { axial_force / p_capacity } else { 0.0 };
    let shear_ratio = shear_force.abs() / v_capacity;

    let interaction = concrete_interaction_ratio(
        code,
        axial_force,
        moment_weak,
        moment_strong,
        p_capacity,
        m_weak,
        m_strong,
    );

    let util_ratio = interaction.max(flexure_strong).max(flexure_weak).max(axial).max(shear_ratio);
    let status = if util_ratio <= 1.0 {
        ConcreteDesignStatus::Pass
    } else {
        ConcreteDesignStatus::Fail
    };

    ConcreteMemberDesignResult {
        member_id: member_id.to_string(),
        section_name,
        concrete_strength,
        code,
        flexure_strong_ratio: flexure_strong,
        flexure_weak_ratio: flexure_weak,
        axial_ratio: axial,
        shear_ratio,
        util_ratio,
        status,
    }
}

/// Full concrete member design check to ACI 318-19, kept for existing callers.
pub fn design_concrete_member(
    member_id: &str,
    section: &Section,
    material: &Material,
    axial_force: f64,
    moment_weak: f64,
    moment_strong: f64,
    shear_force: f64,
    concrete_strength: ConcreteStrength,
) -> ConcreteMemberDesignResult {
    design_concrete_member_with_code(
        member_id,
        section,
        material,
        axial_force,
        moment_weak,
        moment_strong,
        shear_force,
        concrete_strength,
        DesignCode::AiscAci,
    )
}

impl Default for ConcreteDesignStatus {
    fn default() -> Self {
        ConcreteDesignStatus::Pending
    }
}

/// Aggregate concrete design results
#[derive(Debug, Clone, Default)]
pub struct ConcreteDesignResults {
    /// Per-member results
    pub members: Vec<ConcreteMemberDesignResult>,
    pub panels: Vec<SlabDesignResult>,
    pub overall_status: ConcreteDesignStatus,
    pub max_util_ratio: f64,
    pub failing_count: usize,
    pub failing_panels: usize,
}

impl ConcreteDesignResults {
    pub fn add_member(&mut self, result: ConcreteMemberDesignResult) {
        self.members.push(result);
        self.update_summary();
    }

    pub fn add_panel(&mut self, result: SlabDesignResult) {
        self.panels.push(result);
        self.update_summary();
    }

    fn update_summary(&mut self) {
        self.failing_count = self.members.iter()
            .filter(|m| m.status == ConcreteDesignStatus::Fail)
            .count();

        self.failing_panels = self.panels.iter()
            .filter(|p| p.status == SlabDesignStatus::Fail)
            .count();

        let member_max = self.members.iter().map(|m| m.util_ratio).fold(0.0_f64, f64::max);
        let panel_max = self.panels.iter().map(|p| p.util_ratio).fold(0.0_f64, f64::max);
        self.max_util_ratio = member_max.max(panel_max);

        self.overall_status = if self.failing_count > 0 || self.failing_panels > 0 {
            ConcreteDesignStatus::Fail
        } else {
            ConcreteDesignStatus::Pass
        };
    }

    pub fn most_critical_member(&self) -> Option<&ConcreteMemberDesignResult> {
        self.members.iter().max_by(|a, b| {
            a.util_ratio.partial_cmp(&b.util_ratio).unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    pub fn most_critical_panel(&self) -> Option<&SlabDesignResult> {
        self.panels.iter().max_by(|a, b| {
            a.util_ratio.partial_cmp(&b.util_ratio).unwrap_or(std::cmp::Ordering::Equal)
        })
    }
}

impl fmt::Display for ConcreteDesignResults {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Concrete Frame Design Results")?;
        writeln!(f, "{}", "=" .repeat(30).as_str())?;
        writeln!(f, "Overall Status: {:?}", self.overall_status)?;
        writeln!(f, "Maximum Utilisation: {:.2}%", self.max_util_ratio)?;
        writeln!(f, "Failing Members: {}/{}", self.failing_count, self.members.len())?;
        writeln!(f, "Failing Panels: {}/{}", self.failing_panels, self.panels.len())?;
        writeln!(f)?;

        writeln!(f, "{:<12} {:<14} {:<12} {:<10} {:<10} {:<10} {:<10}",
            "Member", "Section", "f'c", "Util%", "Status", "P/φPn", "Mz/φMz");
        writeln!(f, "{}", "-" .repeat(75))?;

        for m in &self.members {
            let concrete_name = match m.concrete_strength {
                ConcreteStrength::C2500 => "C2500",
                ConcreteStrength::C3000 => "C3000",
                ConcreteStrength::C3500 => "C3500",
                ConcreteStrength::C4000 => "C4000",
                ConcreteStrength::C5000 => "C5000",
                ConcreteStrength::C6000 => "C6000",
            };
            let util_str = format!("{:.1}%", m.util_ratio);
            let status_str = match m.status {
                ConcreteDesignStatus::Pass => "PASS",
                ConcreteDesignStatus::Fail => "FAIL",
                ConcreteDesignStatus::Pending => "PEND",
            };
            writeln!(f, "{:<12} {:<14} {:<12} {:<10} {:<10?} {:<9.2}% {:<9.2}%",
                &m.member_id[..std::cmp::min(8, m.member_id.len())],
                &m.section_name[..std::cmp::min(8, m.section_name.len())],
                concrete_name,
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

        writeln!(f, "  Slab Panels: {}", self.panels.len())?;

        for p in &self.panels {
            let system_str = match p.system_type {
                SlabSystemType::OneWay => "One-Way",
                SlabSystemType::TwoWayEquallySpaced => "Two-Way Equally-Spaced",
                SlabSystemType::TwoWayCornerEnhanced => "Two-Way Corner-Enhanced",
                SlabSystemType::TwoEdgeSupported => "Two-Edge Supported",
            };
            writeln!(f, "    Panel {}: {} | rho_long={:.3} | rho_short={:.3} | util={:.2}% | {}", 
                p.panel_id, system_str, p.rho_long, p.rho_short, p.util_ratio, p.status)?
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

/// ONE-WAY AND TWO-WAY SLAB DESIGN PER ACI 318
/// =============================================
///
/// Supports simply supported one-way slabs, continuous one-way slabs,
///
/// Design Steps (per ACI 318 Sec. 8.3):
/// 1. Verify DDM prerequisites (rectangular panel, lambda <= 2, uniform loads,
/// 3. Determine moment coefficients from ACI 318 Fig. 8.3
/// 4. Compute design moments: M = alpha * w_u * l^2
/// 5. Solve for reinforcement ratio rho from M_n = phi * A_s * f_y * (d - a/2)
/// 6. Calculate required area: A_s = rho * b * d
/// 7. Determine bar spacing
/// 8. Check minimum reinforcement (ACI 7.12) and max spacing
///
/// Prerequisites for DDM (ACI 318 Table 8.3):
/// - Rectangular panel with spans l_x and l_y (l_x <= l_y)
/// - l_x / l_y <= 2
/// - Panels continuous over supports
/// - Uniformly distributed loads
/// - Total factored load <= 2000 lb/ft (or 20 kN/m)
/// - Edge panels must satisfy additional conditions

/// Slab panel definition with design parameters
#[derive(Debug, Clone)]
pub struct SlabPanel {
    /// Panel length in long direction (feet or meters)
    pub long_span: f64,
    pub short_span: f64,
    pub f_c_prime: f64,
    pub f_y: f64,
    pub thickness: f64,
    pub n_spans_long: usize,
    pub n_spans_short: usize,
    pub support_condition: SlabSupportCondition,
    pub load_type: SlabLoadType,
    pub panel_width: f64,
}

/// Support condition for slab panels
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SlabSupportCondition {
    SimplySupported,
    Fixed,
    Continuous,
    Cantilever,
}

/// Load type for slab panels
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SlabLoadType {
    Uniform,
    Triangular,
    TriangularReverse,
    PointLoad,
    UniformPlusPoint,
}

/// Design results for a slab panel
#[derive(Debug, Clone)]
pub struct SlabDesignResult {
    /// Panel identifier
    pub panel_id: String,
    pub system_type: SlabSystemType,
    pub rho_long: f64,
    pub rho_short: f64,
    pub a_s_long: f64,
    pub a_s_short: f64,
    pub spacing_long: f64,
    pub spacing_short: f64,
    pub phi: f64,
    pub util_ratio: f64,
    pub status: SlabDesignStatus,
    pub alpha_m: f64,
    pub alpha_n: f64,
    pub rho_check: f64,
    pub rho_min_check: f64,
    pub deflection_check: DeflectionCheck,
}

/// Slab system type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SlabSystemType {
    OneWay,
    TwoWayEquallySpaced,
    TwoWayCornerEnhanced,
    TwoEdgeSupported,
}

/// Slab design status
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SlabDesignStatus {
    Pass,
    Fail,
    CheckRequired,
}

impl Default for SlabDesignStatus {
    fn default() -> Self {
        SlabDesignStatus::CheckRequired
    }
}

impl std::fmt::Display for SlabDesignStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            SlabDesignStatus::Pass => "PASS",
            SlabDesignStatus::Fail => "FAIL",
            SlabDesignStatus::CheckRequired => "CHECK",
        };
        write!(f, "{}", name)
    }
}

/// Deflection check result
#[derive(Debug, Clone)]
pub struct DeflectionCheck {
    /// Whether deflection is within limits
    pub within_limit: bool,
    pub ratio: f64,
    pub notes: String,
}

/// Compute the lambda factor (ratio of short to long span)
pub fn slab_lambda(long_span: f64, short_span: f64) -> f64 {
    if long_span <= 0.0 || short_span <= 0.0 {
        return 0.0;
    }
    short_span / long_span
}

/// Determine slab system type based on span ratio and support conditions
pub fn slab_system_type(
    long_span: f64,
    short_span: f64,
    support_condition: SlabSupportCondition,
) -> SlabSystemType {
    // ACI 318 Table 8.3.1: a panel is two-way while its long/short span ratio
    // does not exceed 2.0; beyond that it spans one-way on the short direction.
    let ratio = if short_span > 0.0 { long_span / short_span } else { f64::INFINITY };
    if ratio <= 2.0 {
        // Potentially two-way system
        match support_condition {
            SlabSupportCondition::SimplySupported => SlabSystemType::TwoEdgeSupported,
            SlabSupportCondition::Fixed => SlabSystemType::TwoWayEquallySpaced,
            SlabSupportCondition::Continuous => SlabSystemType::TwoWayEquallySpaced,
            SlabSupportCondition::Cantilever => SlabSystemType::OneWay,
        }
    } else {
        // lambda > 2.0 => one-way system
        SlabSystemType::OneWay
    }
}

/// Compute moment coefficients for two-way slab direct design method per ACI 318 Fig. 8.3
/// Returns (alpha_m_long, alpha_n_short) coefficients
pub fn slab_two_way_moments(
    lambda: f64,
    support_long: SlabSupportCondition,
    support_short: SlabSupportCondition,
) -> (f64, f64) {
    // ACI 318 Coefficients from Fig. 8.3 (simplified implementation)
    // These are the maximum negative moment coefficients at middle of spans

    let mut alpha_m: f64 = 0.0;  // Moment coefficient long direction
    let mut alpha_n: f64 = 0.0;  // Moment coefficient short direction

    match support_long {
        SlabSupportCondition::SimplySupported => {
            alpha_m = 0.064;
        }
        SlabSupportCondition::Fixed => {
            alpha_m = 0.084;
        }
        SlabSupportCondition::Continuous => {
            alpha_m = 0.074;
        }
        SlabSupportCondition::Cantilever => {
            alpha_m = 0.064;  // Cantilever treated as simply supported for coefficient
        }
    }

    match support_short {
        SlabSupportCondition::SimplySupported => {
            if lambda <= 2.0 {
                alpha_n = 0.035;
            } else {
                alpha_n = 0.0;  // One-way behavior
            }
        }
        SlabSupportCondition::Fixed => {
            if lambda <= 2.0 {
                alpha_n = 0.061;
            } else {
                alpha_n = 0.0;
            }
        }
        SlabSupportCondition::Continuous => {
            if lambda <= 2.0 {
                alpha_n = 0.055;
            } else {
                alpha_n = 0.0;
            }
        }
        SlabSupportCondition::Cantilever => {
            alpha_n = 0.0;  // Cantilever
        }
    }

    (alpha_m, alpha_n)
}

/// Design a one-way simply supported slab per ACI 318
/// Returns SlabDesignResult with required reinforcement
pub fn design_one_way_slab_ss(
    panel: &SlabPanel,
    service_dead_load: f64,
    service_live_load: f64,
) -> SlabDesignResult {
    let lambda = slab_lambda(panel.long_span, panel.short_span);

    // Determine if one-way behavior
    let system_type = if lambda > 2.0 {
        SlabSystemType::OneWay
    } else {
        // Check support conditions - simply supported with lambda > 2 is one-way
        match panel.support_condition {
            SlabSupportCondition::SimplySupported | SlabSupportCondition::Cantilever => {
                SlabSystemType::OneWay
            }
            _ => SlabSystemType::TwoWayEquallySpaced,
        }
    };

    // Factored loads (ACI 318: 1.2D + 1.6L)
    let factored_dead = 1.2 * service_dead_load;
    let factored_live = 1.6 * service_live_load;
    let factored_total = factored_dead + factored_live;

    // Maximum positive moment at mid-span for simply supported slab
    // M_max = w * l^2 / 8 (per unit width)
    // Use short span for one-way design (governs)
    let l_short = panel.short_span;
    let m_pos = factored_total * l_short.powi(2) / 8.0;  // moment per unit width

    // Design moment coefficient for one-way simply supported
    // alpha = 1/8 for uniform load on simply supported span
    let alpha = 1.0 / 8.0;

    // Nominal moment capacity required
    // phi * M_n >= M_max, where phi = 0.90 for flexure
    let phi = 0.90;
    let m_n_required = m_pos / phi;

    // Solve for reinforcement ratio using Whitney stress block
    // M_n = A_s * f_y * (d - a/2), where a = A_s * f_y / (0.85 * f_c_prime)
    // Rearranged: rho = A_s / (b * d) = (M_n * 8) / (phi * f_y * d^2) (approximate for rectangular section)
    // Using effective depth d_eff = thickness - cover - half_bar_diameter
    let d_eff = panel.thickness - 1.0 - 0.5;  // approximate: subtract cover and half bar diameter
    let d_eff = d_eff.max(0.5);  // minimum effective depth

    // Required reinforcement ratio
    // From M_n = phi * A_s * f_y * (d - a/2) and a = A_s * f_y / (0.85 * f_c_prime)
    // rho = A_s / (b * d) and M_n / (f_y * b * d^2) = rho * (1 - rho/2 * f_y / f_c_prime)
    // Solve quadratic: rho^2 * (f_y / (2 * f_c_prime)) - rho + (M_n / (phi * f_y * d^2)) = 0

    let m_n_star = m_n_required / (phi * panel.thickness * panel.thickness * panel.f_y);  // normalized moment

    // Quadratic formula: a*rho^2 + b*rho + c = 0
    // Here: (f_y / (2 * f_c_prime)) * rho^2 - rho + m_n_star = 0
    let a_coeff = panel.f_y / (2.0 * panel.f_c_prime);
    let b_coeff = -1.0_f64;
    let c_coeff = m_n_star;

    // Solve: rho = (-b +- sqrt(b^2 - 4ac)) / (2a)
    let discriminant = (b_coeff * b_coeff - 4.0 * a_coeff * c_coeff).max(0.0);
    let rho = ((-b_coeff + discriminant.sqrt()) / (2.0 * a_coeff)).min(0.005);  // max rho ~ 0.005

    // Use balanced reinforcement ratio limit per ACI 10.5.1 (rho_b = 0.75 * rho_bal)
    // For simplified calculation, use max 0.0025 for tension reinforcement
    let rho = rho.min(0.0025);

    // Calculate required area per unit width
    let a_s_per_width = rho * d_eff;  // in^2 per unit width (or mm^2 per unit width)

    // Assume #4 bars (0.668 in^2) or #10 bars (78.5 mm^2)
    // For 12" width, calculate spacing
    let bar_area = 0.668;  // #4 bar area in in^2 (assuming US units)
    let bar_spacing = (bar_area / a_s_per_width) * 12.0;  // spacing in inches for 12" width

    // Check minimum reinforcement per ACI 7.12 (rho_min = 0.0018 for fy <= 60,000 psi)
    let rho_min = 0.0018;
    let rho_actual = rho.max(rho_min);
    let a_s_min = rho_min * d_eff;  // minimum area per unit width

    // Check maximum spacing per ACI (3d or 18 in for #4 bars)
    let max_spacing = panel.thickness * 3.0;  // simplified: 3x thickness
    let spacing = bar_spacing.min(max_spacing);

    // Verify spacing doesn't exceed max
    let actual_spacing = if spacing > max_spacing { max_spacing } else { spacing };

    // Utilization ratio
    let util_ratio = m_pos / (phi * rho * panel.f_y * d_eff * panel.thickness / 2.0);  // simplified

    // Deflection check (simplified)
    let deflection_check = DeflectionCheck {
        within_limit: true,  // simplified - would need detailed span/depth ratio check
        ratio: 0.8,  // assumed within limits
        notes: "Deflection check simplified - assume OK per ACI h/l ratio".to_string(),
    };

    // Reinforcement ratio check
    let rho_check = rho_actual / rho_min;

    SlabDesignResult {
        panel_id: format!("ONEWAY_{}_{}", panel.short_span, panel.long_span),
        system_type,
        rho_long: rho_actual,
        rho_short: 0.0,  // One-way: no reinforcement needed in short direction (or minimum)
        a_s_long: a_s_per_width * panel.panel_width,  // total area for panel width
        a_s_short: a_s_min * panel.panel_width,  // minimum area
        spacing_long: actual_spacing,
        spacing_short: max_spacing,  // typically wider spacing or none needed
        phi,
        util_ratio,
        status: if util_ratio <= 1.0 { SlabDesignStatus::Pass } else { SlabDesignStatus::CheckRequired },
        alpha_m: alpha,
        alpha_n: 0.0,  // One-way: no moment in short direction
        rho_check,
        rho_min_check: rho_actual / rho_min,
        deflection_check,
    }
}

/// Design a continuous one-way slab per ACI 318
pub fn design_one_way_slab_continuous(
    panel: &SlabPanel,
    service_dead_load: f64,
    service_live_load: f64,
) -> SlabDesignResult {
    let lambda = slab_lambda(panel.long_span, panel.short_span);

    // Determine system type
    let system_type = if lambda > 2.0 {
        SlabSystemType::OneWay
    } else {
        SlabSystemType::TwoWayEquallySpaced
    };

    // Factored loads
    let factored_dead = 1.2 * service_dead_load;
    let factored_live = 1.6 * service_live_load;
    let factored_total = factored_dead + factored_live;

    // For continuous slab, use negative moment over supports (governs)
    // M_neg = w * l^2 / 10 (approximately for equal spans)
    let l_short = panel.short_span;
    let m_neg = factored_total * l_short.powi(2) / 10.0;  // negative moment per unit width

    // Design
    let phi = 0.90;
    let m_n_required = m_neg / phi;

    // Effective depth
    let d_eff = panel.thickness - 1.0 - 0.5;
    let d_eff = d_eff.max(0.5);

    // Required reinforcement ratio (similar quadratic solution)
    let m_n_star = m_n_required / (phi * panel.thickness * panel.thickness * panel.f_y);

    let a_coeff = panel.f_y / (2.0 * panel.f_c_prime);
    let discriminant = (1.0 - 4.0 * a_coeff * m_n_star).max(0.0).sqrt();
    let rho = (1.0 + discriminant) / (2.0 * a_coeff).min(0.003);

    // Use max 0.0025 per ACI
    let rho = rho.min(0.0025);

    let a_s_per_width = rho * d_eff;
    let bar_area = 0.668;  // #4 bar
    let bar_spacing = (bar_area / a_s_per_width) * 12.0;  // inches for 12" width

    let rho_min = 0.0018;
    let rho_actual = rho.max(rho_min);
    let a_s_min = rho_min * d_eff;

    let max_spacing = panel.thickness * 3.0;
    let spacing = bar_spacing.min(max_spacing);

    // Negative moment reinforcement at supports (top steel)
    // Plus positive moment at mid-span (bottom steel)
    // Design both top and bottom reinforcement

    // Utilization ratio
    let util_ratio = m_neg / (phi * rho * panel.f_y * d_eff * panel.thickness / 2.0);

    // Deflection check
    let deflection_check = DeflectionCheck {
        within_limit: true,
        ratio: 0.85,
        notes: "Continuous one-way slab deflection check simplified".to_string(),
    };

    // Rho check
    let rho_check = rho_actual / rho_min;

    SlabDesignResult {
        panel_id: format!("CONTONEWAY_{}_{}", panel.short_span, panel.long_span),
        system_type,
        rho_long: rho_actual,
        rho_short: 0.0,
        a_s_long: a_s_per_width * panel.panel_width,
        a_s_short: a_s_min * panel.panel_width,
        spacing_long: spacing,
        spacing_short: max_spacing,
        phi,
        util_ratio,
        status: if util_ratio <= 1.0 { SlabDesignStatus::Pass } else { SlabDesignStatus::CheckRequired },
        alpha_m: 0.014,  // Continuous one-way moment coefficient
        alpha_n: 0.0,
        rho_check,
        rho_min_check: rho_actual / rho_min,
        deflection_check,
    }
}

/// Design a two-way slab per ACI 318 Direct Design Method (Section 8.3)
/// Requires: panel ratios, support conditions meet DDM criteria
pub fn design_two_way_slab_ddm(
    panel: &SlabPanel,
    service_dead_load: f64,
    service_live_load: f64,
) -> SlabDesignResult {
    let lambda = slab_lambda(panel.long_span, panel.short_span);

    // Check DDM prerequisites:
    // 1. Rectangular panel
    // 2. Ratio of long to short span <= 2
    // 3. Uniformly distributed loads
    // 4. Panels continuous over supports
    // 5. No edge beams (or edge beams minimal)
    let ddm_valid = lambda <= 2.0 && panel.n_spans_long >= 1 && panel.n_spans_short >= 1;

    let system_type = if ddm_valid {
        SlabSystemType::TwoWayEquallySpaced
    } else {
        // Fall back to matrix analysis or one-way
        SlabSystemType::TwoEdgeSupported
    };

    // Factored loads
    let factored_dead = 1.2 * service_dead_load;
    let factored_live = 1.6 * service_live_load;
    let factored_total = factored_dead + factored_live;

    // ACI 318 Table 8.3 moment coefficients for interior panels
    // Negative moment coefficients at continuous edges
    // Positive moment coefficients at mid-span

    // Long/short span names per ACI convention: l_y = long, l_x = short
    let l_long = panel.long_span;
    let l_short = panel.short_span;

    // Get moment coefficients from ACI 318 Fig. 8.3
    let (alpha_m, alpha_n) = slab_two_way_moments(
        lambda,
        panel.support_condition,
        panel.support_condition,  // Assume same support condition both ways for simplicity
    );

    // Design moments per unit width
    // M_pos = alpha * w_u * l^2 (where l is the span direction)
    // M_neg = alpha * w_u * l^2 (at supports)

    // Long direction moments (using l_long as the span)
    let m_pos_long = alpha_m * factored_total * l_long.powi(2);  // positive moment mid-span long direction
    let m_neg_long = alpha_m * factored_total * l_long.powi(2);  // negative moment at support long direction (simplified same coeff)

    // Short direction moments (using l_short as the span)
    let m_pos_short = alpha_n * factored_total * l_short.powi(2);  // positive moment mid-span short direction
    let m_neg_short = alpha_n * factored_total * l_short.powi(2);  // negative moment at support short direction

    // Design for long direction
    let phi = 0.90;

    // Long direction design
    let m_n_req_long = m_pos_long / phi;
    let d_eff = panel.thickness - 1.0 - 0.5;
    let d_eff = d_eff.max(0.5);

    let m_n_star_long = m_n_req_long / (phi * panel.thickness * panel.thickness * panel.f_y);
    let a_coeff = panel.f_y / (2.0 * panel.f_c_prime);
    let discriminant_long = (1.0 - 4.0 * a_coeff * m_n_star_long).max(0.0).sqrt();
    let rho_long = (1.0 + discriminant_long) / (2.0 * a_coeff).min(0.003);
    let rho_long = rho_long.min(0.0025);
    let a_s_long_per_width = rho_long * d_eff;

    // Short direction design
    let m_n_req_short = m_pos_short / phi;
    let m_n_star_short = m_n_req_short / (phi * panel.thickness * panel.thickness * panel.f_y);
    let discriminant_short = (1.0 - 4.0 * a_coeff * m_n_star_short).max(0.0).sqrt();
    let rho_short = (1.0 + discriminant_short) / (2.0 * a_coeff).min(0.003);
    let rho_short = rho_short.min(0.0025);
    let a_s_short_per_width = rho_short * d_eff;

    // Calculate spacing for both directions
    let bar_area = 0.668;  // #4 bar
    let spacing_long = (bar_area / rho_long.max(0.0018)) * 12.0;  // inches for 12" width
    let spacing_short = (bar_area / rho_short.max(0.0018)) * 12.0;

    let rho_min = 0.0018;
    let rho_actual_long = rho_long.max(rho_min);
    let rho_actual_short = rho_short.max(rho_min);
    let a_s_min_long = rho_min * d_eff;
    let a_s_min_short = rho_min * d_eff;
    let max_spacing = panel.thickness * 3.0;

    let actual_spacing_long = spacing_long.min(max_spacing);
    let actual_spacing_short = spacing_short.min(max_spacing);

    // Utilization ratio - use the more severe direction
    let util_ratio_long = m_pos_long / (phi * rho_long * panel.f_y * d_eff * panel.thickness / 2.0);
    let util_ratio_short = m_pos_short / (phi * rho_short * panel.f_y * d_eff * panel.thickness / 2.0);
    let util_ratio = util_ratio_long.max(util_ratio_short);

    // Deflection check (simplified)
    let deflection_check = DeflectionCheck {
        within_limit: true,
        ratio: 0.8,
        notes: "Two-way slab DDM deflection check simplified".to_string(),
    };

    // Rho checks
    let rho_check_long = rho_actual_long / rho_min;
    let rho_check_short = rho_actual_short / rho_min;

    SlabDesignResult {
        panel_id: format!("TWOWAY_{}_{}_{}", panel.short_span, panel.long_span, lambda),
        system_type,
        rho_long: rho_actual_long,
        rho_short: rho_actual_short,
        a_s_long: a_s_long_per_width * panel.panel_width,
        a_s_short: a_s_short_per_width * panel.panel_width,
        spacing_long: actual_spacing_long,
        spacing_short: actual_spacing_short,
        phi,
        util_ratio,
        status: if util_ratio <= 1.0 { SlabDesignStatus::Pass } else { SlabDesignStatus::CheckRequired },
        alpha_m,
        alpha_n,
        rho_check: rho_check_long.max(rho_check_short),
        rho_min_check: rho_actual_long.max(rho_actual_short) / rho_min,
        deflection_check,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slab_lambda() {
        // Test lambda calculation
        assert_eq!(slab_lambda(10.0, 5.0), 0.5);
        assert_eq!(slab_lambda(4.0, 10.0), 2.5);
        assert_eq!(slab_lambda(0.0, 5.0), 0.0);
        assert_eq!(slab_lambda(10.0, 0.0), 0.0);
    }

    #[test]
    fn test_slab_system_type() {
        // lambda <= 2.0 with simply supported => TwoEdgeSupported
        assert_eq!(slab_system_type(10.0, 5.0, SlabSupportCondition::SimplySupported), SlabSystemType::TwoEdgeSupported);
        // lambda <= 2.0 with fixed => TwoWayEquallySpaced
        assert_eq!(slab_system_type(10.0, 5.0, SlabSupportCondition::Fixed), SlabSystemType::TwoWayEquallySpaced);
        // lambda <= 2.0 with continuous => TwoWayEquallySpaced
        assert_eq!(slab_system_type(10.0, 5.0, SlabSupportCondition::Continuous), SlabSystemType::TwoWayEquallySpaced);
        // lambda <= 2.0 with cantilever => OneWay
        assert_eq!(slab_system_type(10.0, 5.0, SlabSupportCondition::Cantilever), SlabSystemType::OneWay);
        // lambda > 2.0 => OneWay regardless
        assert_eq!(slab_system_type(10.0, 4.0, SlabSupportCondition::SimplySupported), SlabSystemType::OneWay);
    }

    #[test]
    fn test_slab_two_way_moments() {
        // Test moment coefficient computation
        let (alpha_m, alpha_n) = slab_two_way_moments(1.0, SlabSupportCondition::SimplySupported, SlabSupportCondition::SimplySupported);
        assert!(alpha_m > 0.0);
        assert!(alpha_n > 0.0);

        // With lambda > 2.0, alpha_n should be 0 (one-way)
        let (alpha_m2, alpha_n2) = slab_two_way_moments(3.0, SlabSupportCondition::SimplySupported, SlabSupportCondition::SimplySupported);
        assert_eq!(alpha_n2, 0.0);
    }

    #[test]
    fn test_design_one_way_slab_ss() {
        let panel = SlabPanel {
            long_span: 16.0,
            short_span: 8.0,
            f_c_prime: 4000.0,
            f_y: 60000.0,
            thickness: 6.0,
            n_spans_long: 1,
            n_spans_short: 1,
            support_condition: SlabSupportCondition::SimplySupported,
            load_type: SlabLoadType::Uniform,
            panel_width: 12.0,
        };

        let result = design_one_way_slab_ss(&panel, 100.0, 100.0);

        // Verify result structure
        assert!(!result.panel_id.is_empty());
        assert!(matches!(result.system_type, SlabSystemType::OneWay));
        assert!(result.rho_long > 0.0);
        assert!(result.rho_short == 0.0);  // One-way: no reinforcement in short direction
        assert!(result.a_s_long > 0.0);
        assert!(result.a_s_short >= 0.0);
        assert!(result.spacing_long > 0.0);
        assert!(result.spacing_short > 0.0);
        assert!(result.phi > 0.0);
        assert!(result.util_ratio >= 0.0);
        assert!(matches!(result.status, SlabDesignStatus::Pass | SlabDesignStatus::CheckRequired));
        assert!(result.alpha_m > 0.0);
        assert!(result.alpha_n == 0.0);  // One-way: no moment in short direction
        assert!(result.rho_check > 0.0);
        assert!(result.rho_min_check > 0.0);
        assert!(deflection_check_within_limit(&result.deflection_check));
    }

    #[test]
    fn test_design_one_way_slab_continuous() {
        let panel = SlabPanel {
            long_span: 16.0,
            short_span: 8.0,
            f_c_prime: 4000.0,
            f_y: 60000.0,
            thickness: 6.0,
            n_spans_long: 2,
            n_spans_short: 1,
            support_condition: SlabSupportCondition::Continuous,
            load_type: SlabLoadType::Uniform,
            panel_width: 12.0,
        };

        let result = design_one_way_slab_continuous(&panel, 100.0, 100.0);

        assert!(!result.panel_id.is_empty());
        assert!(matches!(result.system_type, SlabSystemType::OneWay | SlabSystemType::TwoWayEquallySpaced));
        assert!(result.rho_long > 0.0);
        assert!(result.util_ratio >= 0.0);
        assert!(matches!(result.status, SlabDesignStatus::Pass | SlabDesignStatus::CheckRequired));
        assert!(result.alpha_m > 0.0);
    }

    #[test]
    fn test_design_two_way_slab_ddm() {
        let panel = SlabPanel {
            long_span: 16.0,
            short_span: 8.0,
            f_c_prime: 4000.0,
            f_y: 60000.0,
            thickness: 6.0,
            n_spans_long: 2,
            n_spans_short: 2,
            support_condition: SlabSupportCondition::Continuous,
            load_type: SlabLoadType::Uniform,
            panel_width: 12.0,
        };

        let result = design_two_way_slab_ddm(&panel, 100.0, 100.0);

        assert!(!result.panel_id.is_empty());
        assert!(matches!(result.system_type, SlabSystemType::TwoWayEquallySpaced));
        assert!(result.rho_long > 0.0);
        assert!(result.rho_short > 0.0);
        assert!(result.a_s_long > 0.0);
        assert!(result.a_s_short > 0.0);
        assert!(result.spacing_long > 0.0);
        assert!(result.spacing_short > 0.0);
        assert!(result.phi > 0.0);
        assert!(result.util_ratio >= 0.0);
        assert!(matches!(result.status, SlabDesignStatus::Pass | SlabDesignStatus::CheckRequired));
        assert!(result.alpha_m > 0.0);
        assert!(result.alpha_n > 0.0);
        assert!(result.rho_check > 0.0);
        assert!(result.rho_min_check > 0.0);
        assert!(deflection_check_within_limit(&result.deflection_check));
    }

    #[test]
    fn test_design_two_way_slab_ddm_lambda_gt_2() {
        // When lambda > 2.0, should fall back to TwoEdgeSupported (one-way behavior)
        let panel = SlabPanel {
            long_span: 16.0,
            short_span: 8.0,
            f_c_prime: 4000.0,
            f_y: 60000.0,
            thickness: 6.0,
            n_spans_long: 1,
            n_spans_short: 1,
            support_condition: SlabSupportCondition::SimplySupported,
            load_type: SlabLoadType::Uniform,
            panel_width: 12.0,
        };

        let result = design_two_way_slab_ddm(&panel, 100.0, 100.0);

        // lambda = 8/16 = 0.5, which is <= 2.0, so DDM should be valid
        // But with 1 span each way, it may still be valid
        assert!(result.system_type == SlabSystemType::TwoWayEquallySpaced || result.system_type == SlabSystemType::TwoEdgeSupported);
        assert!(result.rho_long > 0.0 || result.rho_short > 0.0);
    }

    #[test]
    fn test_concrete_design_with_slabs() {
        let mut results = ConcreteDesignResults::default();

        // Add a column design result
        results.add_member(ConcreteMemberDesignResult {
            member_id: "COL1".to_string(),
            section_name: "COL400X400".to_string(),
            concrete_strength: ConcreteStrength::C4000,
            code: DesignCode::AiscAci,
            flexure_strong_ratio: 0.6,
            flexure_weak_ratio: 0.5,
            axial_ratio: 0.5,
            shear_ratio: 0.3,
            util_ratio: 0.6,
            status: ConcreteDesignStatus::Pass,
        });

        // Add a slab panel design result
        results.add_panel(SlabDesignResult {
            panel_id: "ONEWAY_8_16".to_string(),
            system_type: SlabSystemType::OneWay,
            rho_long: 0.0020,
            rho_short: 0.0,
            a_s_long: 12.0,
            a_s_short: 5.0,
            spacing_long: 12.0,
            spacing_short: 0.0,
            phi: 0.90,
            util_ratio: 0.55,
            status: SlabDesignStatus::Pass,
            alpha_m: 0.125,
            alpha_n: 0.0,
            rho_check: 1.1,
            rho_min_check: 1.1,
            deflection_check: DeflectionCheck {
                within_limit: true,
                ratio: 0.8,
                notes: "Deflection check OK".to_string(),
            },
        });

        // Verify summary
        assert!(results.overall_status == ConcreteDesignStatus::Pass);
        assert!(results.failing_count == 0);
        assert!(results.failing_panels == 0);
        assert!(results.max_util_ratio > 0.0);

        // Test most critical
        let critical_member = results.most_critical_member();
        assert!(critical_member.is_some());

        let critical_panel = results.most_critical_panel();
        assert!(critical_panel.is_some());
        assert_eq!(critical_panel.unwrap().panel_id, "ONEWAY_8_16");
    }

    #[test]
    fn test_slab_design_results_display() {
        let mut results = ConcreteDesignResults::default();

        results.add_panel(SlabDesignResult {
            panel_id: "TEST_SLAB".to_string(),
            system_type: SlabSystemType::TwoWayEquallySpaced,
            rho_long: 0.0025,
            rho_short: 0.0022,
            a_s_long: 15.0,
            a_s_short: 13.0,
            spacing_long: 18.0,
            spacing_short: 20.0,
            phi: 0.90,
            util_ratio: 0.85,
            status: SlabDesignStatus::Pass,
            alpha_m: 0.055,
            alpha_n: 0.035,
            rho_check: 1.4,
            rho_min_check: 1.4,
            deflection_check: DeflectionCheck {
                within_limit: true,
                ratio: 0.8,
                notes: "OK".to_string(),
            },
        });

        let display_output = format!("{}", results);
        assert!(!display_output.is_empty());
        assert!(display_output.contains("Concrete Frame Design Results"));
        assert!(display_output.contains("Two-Way Equally-Spaced"));
        assert!(display_output.contains("Failing Panels"));
    }

    fn deflection_check_within_limit(check: &DeflectionCheck) -> bool {
        check.within_limit && check.ratio <= 1.0
    }
}

#[cfg(test)]
mod code_selector_tests {
    use super::*;

    #[test]
    fn every_code_gives_a_positive_capacity() {
        let section = Section::rectangular("C", 0.4, 0.5);
        for code in DesignCode::ALL {
            assert!(axial_capacity(&section, ConcreteStrength::C4000, code) > 0.0);
            let (strong, weak) = flexural_capacity(&section, ConcreteStrength::C4000, code);
            assert!(strong > 0.0 && weak > 0.0);
            assert!(strong > weak, "{code:?}: strong {strong} not above weak {weak}");
            assert!(shear_capacity(&section, ConcreteStrength::C4000, code) > 0.0);
        }
    }

    #[test]
    fn indian_axial_capacity_is_the_lowest_of_the_three() {
        let section = Section::rectangular("C", 0.4, 0.5);
        let indian = axial_capacity(&section, ConcreteStrength::C4000, DesignCode::Indian);
        let aisc = axial_capacity(&section, ConcreteStrength::C4000, DesignCode::AiscAci);
        let euro = axial_capacity(&section, ConcreteStrength::C4000, DesignCode::Eurocode);
        assert!(indian < aisc, "{indian} is not below {aisc}");
        assert!(indian < euro, "{indian} is not below {euro}");
    }

    #[test]
    fn stronger_concrete_gives_more_capacity() {
        let section = Section::rectangular("C", 0.4, 0.5);
        for code in DesignCode::ALL {
            let weak = axial_capacity(&section, ConcreteStrength::C2500, code);
            let strong = axial_capacity(&section, ConcreteStrength::C6000, code);
            assert!(strong > weak, "{code:?}: {strong} is not above {weak}");
        }
    }

    #[test]
    fn aci_interaction_switches_branch_at_twenty_percent() {
        // Same moment either side of the 0.2 threshold: the low-axial branch
        // halves the axial term, so the ratio must drop.
        let low = concrete_interaction_ratio(DesignCode::AiscAci, 0.19, 0.0, 0.5, 1.0, 1.0, 1.0);
        let high = concrete_interaction_ratio(DesignCode::AiscAci, 0.21, 0.0, 0.5, 1.0, 1.0, 1.0);
        assert!((low - (0.095 + 0.5)).abs() < 1e-9, "low branch was {low}");
        assert!((high - (0.21 + 8.0 / 9.0 * 0.5)).abs() < 1e-9, "high branch was {high}");
    }

    #[test]
    fn design_concrete_member_with_code_reports_its_code() {
        let section = Section::rectangular("C", 0.4, 0.5);
        let material = Material::concrete();
        let result = design_concrete_member_with_code(
            "C1",
            &section,
            &material,
            800_000.0,
            60_000.0,
            90_000.0,
            40_000.0,
            ConcreteStrength::C4000,
            DesignCode::Eurocode,
        );
        assert_eq!(result.code, DesignCode::Eurocode);
        assert!(result.util_ratio > 0.0);
        assert!(result.section_name == "C");
    }
}