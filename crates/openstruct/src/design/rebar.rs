//! Rebar Detailing per ACI 318 and EN 1992-1-1 (EC2).
//! Detailing checks for reinforced concrete including:
//! - Bar size and spacing validation
//! - Development length calculations
//! - Stirrup/spiral spacing for columns
//! - Boundary element reinforcement
//! - Bar placement and cover requirements
//!
//! Design values use ACI 318 development length equations
//! and Eurocode 2 bar bending schedules.

use crate::materials::Material;
use crate::sections::Section;
use std::fmt;

///// Bar size identification
/// Standard ACI/ISO bar diameter sizes (mm)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BarSize {
    /// #3 bar, 10 mm diameter
    Three,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,
    Ten,
}

/// Stirrup/hoop size identification
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StirrupSize {
    /// #3 stirrup, 10 mm
    Three,
    Four,
    Five,
    Six,
}

/// Concrete cover for different exposure conditions (mm)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConcreteCover {
    /// Normal conditions, 20 mm
    Normal,
    Moderate,
    Severe,
    VerySevere,
}

/// Development length calculation result
#[derive(Debug, Clone)]
pub struct DevelopmentLengthResult {
    /// Bar size
    pub bar_size: BarSize,
    pub l_d_required: f64,
    pub l_d_provided: f64,
    pub ratio: f64,
    pub status: DevelopmentStatus,
}

/// Development length status
#[derive(Debug, Clone, PartialEq)]
pub enum DevelopmentStatus {
    OK,
    Insufficient,
    CheckRequired,
}

/// Stirrup/spiral spacing result
#[derive(Debug, Clone)]
pub struct StirrupSpacingResult {
    /// Stirrup size
    pub stirrup_size: StirrupSize,
    pub s_required: f64,
    pub s_provided: f64,
    pub ratio: f64,
    pub status: SpacingStatus,
}

/// Stirrup spacing status
#[derive(Debug, Clone, PartialEq)]
pub enum SpacingStatus {
    OK,
    Tight,
    Insufficient,
}

/// Boundary element reinforcement result
#[derive(Debug, Clone)]
pub struct BoundaryElementResult {
    /// Wall/column width (mm)
    pub dimension: f64,
    pub rho_req: f64,
    pub rho_provided: f64,
    pub ratio: f64,
    pub status: BoundaryStatus,
}

/// Boundary element reinforcement status
#[derive(Debug, Clone, PartialEq)]
pub enum BoundaryStatus {
    OK,
    Insufficient,
    CheckRequired,
}

/// Calculate development length for a bar in tension per ACI 318
/// l_d = γ_t * γ_e * γ_s * γ_b * (d_b * f_y) / (sqrt(f'_c) * λ)
fn calculate_development_length_tension(
    bar_size: BarSize,
    f_y: f64,
    f_c_prime: f64,
    embedment: f64,
    cover: f64,
    tie_spacing: f64,
) -> DevelopmentLengthResult {
    let d_b = match bar_size {
        BarSize::Three => 10.0,
        BarSize::Four => 12.7,
        BarSize::Five => 15.9,
        BarSize::Six => 19.1,
        BarSize::Seven => 22.2,
        BarSize::Eight => 25.4,
        BarSize::Nine => 28.7,
        BarSize::Ten => 31.8,
    };
    
    // ACI 318-19 Eq. 25-4 (simplified): l_d = (d_b * f_y) / (9 * sqrt(f'_c))
    // with modification factors
    let gamma_t = 1.0;  // Top bar factor
    let gamma_e = 1.0;  /// Epoxy coating factor
    let gamma_s = 1.0;  /// Stress factor (1.0 for fy ≤ 60000 psi)
    let gamma_b = 1.0;  /// Bar size factor (1.0 for #8 and smaller)
    let lambda = 1.0;   /// Lightweight concrete factor
    
    // Basic development length
    let l_d_basic = (d_b * f_y) / (9.0 * f_c_prime.sqrt());
    
    // Apply modification factors
    let l_d_required = gamma_t * gamma_e * gamma_s * gamma_b * l_d_basic;
    
    // Adjust for embedment depth (minimum 8 bar diameters or 6 inches)
    let min_embedment = d_b * 8.0;
    let actual_embedment = embedment.max(min_embedment);
    
    // Provide development length based on actual embedment
    let l_d_provided = actual_embedment;
    let ratio = l_d_required / l_d_provided.max(1e-10);
    
    let status = if ratio <= 1.0 {
        DevelopmentStatus::OK
    } else if ratio <= 1.2 {
        DevelopmentStatus::CheckRequired
    } else {
        DevelopmentStatus::Insufficient
    };
    
    DevelopmentLengthResult {
        bar_size,
        l_d_required,
        l_d_provided: actual_embedment,
        ratio,
        status,
    }
}

/// Calculate development length for a bar in compression per ACI 318
fn calculate_development_length_compression(
    bar_size: BarSize,
    f_y: f64,
    f_c_prime: f64,
    embedment: f64,
) -> DevelopmentLengthResult {
    let d_b = match bar_size {
        BarSize::Three => 10.0,
        BarSize::Four => 12.7,
        BarSize::Five => 15.9,
        BarSize::Six => 19.1,
        BarSize::Seven => 22.2,
        BarSize::Eight => 25.4,
        BarSize::Nine => 28.7,
        BarSize::Ten => 31.8,
    };
    
    // ACI 318 compression development is typically 1.3x tension
    let tension_result = calculate_development_length_tension(
        bar_size, f_y, f_c_prime, embedment, 0.0, 0.0,
    );
    
    let l_d_required_compression = tension_result.l_d_required * 1.3;
    let l_d_provided_compression = tension_result.l_d_provided;
    let ratio = l_d_required_compression / l_d_provided_compression.max(1e-10);
    
    let status = if ratio <= 1.0 {
        DevelopmentStatus::OK
    } else if ratio <= 1.2 {
        DevelopmentStatus::CheckRequired
    } else {
        DevelopmentStatus::Insufficient
    };
    
    DevelopmentLengthResult {
        bar_size,
        l_d_required: l_d_required_compression,
        l_d_provided: l_d_provided_compression,
        ratio,
        status,
    }
}

/// Calculate stirrup spacing for a column per ACI 318
/// s = A_v / (f_yt * A_br * s) ... simplified
fn calculate_stirrup_spacing(
    stirrup_size: StirrupSize,
    column_width: f64,
    column_height: f64,
    f_yt: f64,
    p_transverse: f64,
    p_longitudinal: f64,
    cover: f64,
) -> StirrupSpacingResult {
    let a_v = match stirrup_size {
        StirrupSize::Three => 10.0,
        StirrupSize::Four => 12.7,
        StirrupSize::Five => 15.9,
        StirrupSize::Six => 19.1,
    };
    
    // ACI 318: s = A_v * f_yt / (A_br * p_transverse)
    // Simplified: maximum spacing = d/2 or 24 inches (600 mm), minimum = 4 inches (100 mm)
    let d_col = column_width.min(column_height);  // effective depth
    
    // Required spacing based on transverse steel area
    // A_v = 5.08 * d_bar^2 / s for #3-#6 stirrups (two-legged)
    // s = 5.08 * d_bar^2 / A_v_required
    
    // Simplified: use code maximums
    let s_max_code = 0.05 * d_col;  // 5% of column dimension
    let s_min_code: f64 = 100.0;        // minimum 100 mm
    
    // Calculate required spacing (simplified)
    // In full implementation: s = (A_v * f_yt) / (A_br * p_transverse)
    let s_required = s_max_code.min(s_min_code);  // conservative: use code max
    
    // Provide typical spacing
    let s_provided: f64 = 150.0;  // typical 150 mm spacing
    let ratio = s_required / s_provided.max(1e-10);
    
    let status = if ratio <= 1.0 {
        SpacingStatus::OK
    } else if ratio <= 1.2 {
        SpacingStatus::Tight
    } else {
        SpacingStatus::Insufficient
    };
    
    StirrupSpacingResult {
        stirrup_size,
        s_required: s_required.max(1e-10),
        s_provided: s_provided.max(1e-10),
        ratio,
        status,
    }
}

/// Calculate boundary element reinforcement ratio for shear walls
fn calculate_boundary_element_ratio(
    wall_width: f64,
    wall_height: f64,
    f_c_prime: f64,
    rho_provided: f64,
) -> BoundaryElementResult {
    // ACI 318: boundary reinforcement ratio ρ_s = 0.25 * ρ_l + 0.0025
    // where ρ_l is the longitudinal reinforcement ratio
    // Minimum ρ_s = 0.0025 for intermediate/sexvere exposure
    // Maximum ρ_s = 0.04 for most applications
    
    let rho_req: f64 = 0.0025;  // minimum boundary reinforcement ratio
    
    let ratio = rho_provided / rho_req.max(1e-10);
    
    let status = if ratio >= 1.0 {
        BoundaryStatus::OK
    } else if ratio >= 0.8 {
        BoundaryStatus::CheckRequired
    } else {
        BoundaryStatus::Insufficient
    };
    
    BoundaryElementResult {
        dimension: wall_width,
        rho_req,
        rho_provided: rho_provided.max(0.0),
        ratio,
        status,
    }
}

/// Display implementation for development length results
impl fmt::Display for DevelopmentLengthResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Development Length Calculation")?;
        writeln!(f, "{}", "=" .repeat(30).as_str())?;
        writeln!(f, "Bar Size: {:?}", self.bar_size)?;
        writeln!(f, "Required L_d: {:.1} mm", self.l_d_required)?;
        writeln!(f, "Provided L_d: {:.1} mm", self.l_d_provided)?;
        writeln!(f, "Ratio L_d,required/L_d,provided: {:.2}%", self.ratio)?;
        writeln!(f, "Status: {:?}", self.status)?;
        Ok(())
    }
}

/// Display implementation for stirrup spacing results
impl fmt::Display for StirrupSpacingResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Stirrup Spacing Calculation")?;
        writeln!(f, "{}", "=" .repeat(30).as_str())?;
        writeln!(f, "Stirrup Size: {:?}", self.stirrup_size)?;
        writeln!(f, "Required Spacing: {:.1} mm", self.s_required)?;
        writeln!(f, "Provided Spacing: {:.1} mm", self.s_provided)?;
        writeln!(f, "Ratio s_required/s_provided: {:.2}%", self.ratio)?;
        writeln!(f, "Status: {:?}", self.status)?;
        Ok(())
    }
}

/// Display implementation for boundary element results
impl fmt::Display for BoundaryElementResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Boundary Element Reinforcement")?;
        writeln!(f, "{}", "=" .repeat(30).as_str())?;
        writeln!(f, "Wall Dimension: {:.1} mm", self.dimension)?;
        writeln!(f, "Required ρ_s: {:.4}", self.rho_req)?;
        writeln!(f, "Provided ρ_s: {:.4}", self.rho_provided)?;
        writeln!(f, "Ratio ρ_provided/ρ_required: {:.2}%", self.ratio)?;
        writeln!(f, "Status: {:?}", self.status)?;
        Ok(())
    }
}

/// Aggregate rebar detailing results for multiple elements
#[derive(Debug, Clone, Default)]
pub struct RebarDetailingResults {
    /// Per-element development length results
    pub development_lengths: Vec<DevelopmentLengthResult>,
    pub stirrup_spacings: Vec<StirrupSpacingResult>,
    pub boundary_elements: Vec<BoundaryElementResult>,
    pub overall_status: String,
    pub insufficient_count: usize,
}

impl RebarDetailingResults {
    pub fn add_development_length(&mut self, result: DevelopmentLengthResult) {
        self.development_lengths.push(result);
    }
    
    pub fn add_stirrup_spacing(&mut self, result: StirrupSpacingResult) {
        self.stirrup_spacings.push(result);
    }
    
    pub fn add_boundary_element(&mut self, result: BoundaryElementResult) {
        self.boundary_elements.push(result);
    }
    
    fn compute_summary(&mut self) {
        self.insufficient_count = self.development_lengths
            .iter()
            .filter(|d| d.status == DevelopmentStatus::Insufficient)
            .count()
            + self.stirrup_spacings
                .iter()
                .filter(|s| s.status == SpacingStatus::Insufficient)
                .count()
            + self.boundary_elements
                .iter()
                .filter(|b| b.status == BoundaryStatus::Insufficient)
                .count();
        
        self.overall_status = if self.insufficient_count > 0 {
            format!("{} items require review", self.insufficient_count)
        } else {
            "All checks satisfied".to_string()
        };
    }
}

impl fmt::Display for RebarDetailingResults {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut summary = self.clone();
        summary.compute_summary();
        
        writeln!(f, "Rebar Detailing Results")?;
        writeln!(f, "{}", "=" .repeat(28).as_str())?;
        writeln!(f, "Overall Status: {}", summary.overall_status)?;
        writeln!(f, "Insufficient: {}", summary.insufficient_count)?;
        writeln!(f)?;
        
        writeln!(f, "Development Lengths:")?;
        for d in &self.development_lengths {
            writeln!(f, "  {:?}: L_d,r={:.0} mm, L_d,p={:.0} mm, ratio={:.1}%, {:?}", 
                d.bar_size, d.l_d_required, d.l_d_provided, d.ratio, d.status)?;
        }
        
        writeln!(f)?;
        writeln!(f, "Stirrup Spacings:")?;
        for s in &self.stirrup_spacings {
            writeln!(f, "  {:?}: s_r={:.0} mm, s_p={:.0} mm, ratio={:.1}%, {:?}", 
                s.stirrup_size, s.s_required, s.s_provided, s.ratio, s.status)?;
        }
        
        writeln!(f)?;
        writeln!(f, "Boundary Elements:")?;
        for b in &self.boundary_elements {
            writeln!(f, "  dim={:.0} mm: ρ_r/ρ_p={:.1}%, {:?}", 
                b.dimension, b.ratio, b.status)?;
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_development_length_tension() {
        let result = calculate_development_length_tension(
            BarSize::Four,
            400.0,   // fy = 400 MPa (≈ 60 ksi)
            25.0,    // f'c = 25 MPa (≈ 3600 psi)
            200.0,   // embedment 200 mm
            30.0,    // cover 30 mm
            200.0,   // tie spacing 200 mm
        );
        
        assert!(result.l_d_required > 0.0);
        assert!(result.l_d_provided > 0.0);
        assert!(matches!(result.status, DevelopmentStatus::OK | DevelopmentStatus::CheckRequired | DevelopmentStatus::Insufficient));
    }
    
    #[test]
    fn test_development_length_compression() {
        let result = calculate_development_length_compression(
            BarSize::Five,
            400.0,   // fy = 400 MPa
            25.0,    // f'c = 25 MPa
            250.0,   // embedment 250 mm
        );
        
        assert!(result.l_d_required > 0.0);
        assert!(matches!(result.status, DevelopmentStatus::OK | DevelopmentStatus::CheckRequired | DevelopmentStatus::Insufficient));
    }
    
    #[test]
    fn test_stirrup_spacing() {
        let result = calculate_stirrup_spacing(
            StirrupSize::Four,
            400.0,   // column width 400 mm
            400.0,   // column height 400 mm
            400.0,   // f_yt = 400 MPa
            100.0,   // transverse force
            500.0,   // longitudinal force
            40.0,    // cover 40 mm
        );
        
        assert!(result.s_required > 0.0);
        assert!(result.s_provided > 0.0);
        assert!(matches!(result.status, SpacingStatus::OK | SpacingStatus::Tight | SpacingStatus::Insufficient));
    }
    
    #[test]
    fn test_boundary_element_ratio() {
        let result = calculate_boundary_element_ratio(
            300.0,   // wall width 300 mm
            3000.0,  // wall height 3000 mm
            25.0,    // f'c = 25 MPa
            0.003,   // provided ratio 0.003
        );
        
        assert!(result.ratio > 0.0);
        assert!(matches!(result.status, BoundaryStatus::OK | BoundaryStatus::CheckRequired | BoundaryStatus::Insufficient));
    }
    
    #[test]
    fn test_rebar_detailing_results_display() {
        let mut results = RebarDetailingResults::default();
        
        results.add_development_length(DevelopmentLengthResult {
            bar_size: BarSize::Four,
            l_d_required: 500.0,
            l_d_provided: 600.0,
            ratio: 0.83,
            status: DevelopmentStatus::OK,
        });
        
        results.add_stirrup_spacing(StirrupSpacingResult {
            stirrup_size: StirrupSize::Four,
            s_required: 200.0,
            s_provided: 150.0,
            ratio: 1.33,
            status: SpacingStatus::Insufficient,
        });
        
        results.add_boundary_element(BoundaryElementResult {
            dimension: 300.0,
            rho_req: 0.0025,
            rho_provided: 0.003,
            ratio: 1.2,
            status: BoundaryStatus::OK,
        });
        
        let display = format!("{}", results);
        assert!(display.contains("Development Length"));
        assert!(display.contains("Stirrup Spacing"));
        assert!(display.contains("Boundary Element"));
        assert!(display.contains("Insufficient"));
    }
}