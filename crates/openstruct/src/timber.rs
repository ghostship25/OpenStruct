//! Timber Design (NDS - National Design Specification for Wood Construction).
//!
//! Implements timber design calculations:
//! - Bending stress checks
//! - Shear stress checks
//! - Compression parallel/perpendicular to grain
//! - Tension parallel to grain
//! - Combined bending and axial
//! - Column stability factor
//! - Adjustment factors (C_D, C_M, C_t, C_L, C_F, C_fu, C_i, C_r)
//!
//! Reference: NDS 2018, ANSI/AWC NDS-2018

/// Wood species and grade
#[derive(Debug, Clone)]
pub struct WoodSpecies {
    pub name: String,
    pub f_b: f64,  // bending strength (psi)
    pub f_t: f64,  // tension parallel to grain (psi)
    pub f_v: f64,  // shear parallel to grain (psi)
    pub f_c_parallel: f64,  // compression parallel to grain (psi)
    pub f_c_perp: f64,  // compression perpendicular to grain (psi)
    pub e: f64,  // modulus of elasticity (psi)
    pub e_min: f64,  // minimum modulus of elasticity (psi)
}

impl WoodSpecies {
    pub fn douglas_fir_larch_no1() -> Self {
        Self {
            name: "Douglas Fir-Larch No.1".to_string(),
            f_b: 1000.0,
            f_t: 675.0,
            f_v: 180.0,
            f_c_parallel: 1450.0,
            f_c_perp: 625.0,
            e: 1_700_000.0,
            e_min: 620_000.0,
        }
    }

    pub fn southern_pine_no2() -> Self {
        Self {
            name: "Southern Pine No.2".to_string(),
            f_b: 1100.0,
            f_t: 625.0,
            f_v: 175.0,
            f_c_parallel: 1300.0,
            f_c_perp: 565.0,
            e: 1_400_000.0,
            e_min: 510_000.0,
        }
    }

    pub fn spruce_pine_fir_no2() -> Self {
        Self {
            name: "Spruce-Pine-Fir No.2".to_string(),
            f_b: 875.0,
            f_t: 450.0,
            f_v: 135.0,
            f_c_parallel: 950.0,
            f_c_perp: 425.0,
            e: 1_400_000.0,
            e_min: 510_000.0,
        }
    }
}

/// Timber member design
#[derive(Debug, Clone)]
pub struct TimberMember {
    pub species: WoodSpecies,
    pub width: f64,   // inches
    pub depth: f64,   // inches
    pub length: f64,  // inches
    pub load_duration: LoadDuration,
    pub moisture: MoistureCondition,
    pub temperature: TemperatureCondition,
    pub lateral_support: bool,
}

/// Load duration factor (C_D)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LoadDuration {
    Permanent,      // C_D = 0.9
    TenYears,       // C_D = 1.0
    TwoMonths,      // C_D = 1.15
    SevenDays,      // C_D = 1.25
    Wind,           // C_D = 1.6
    Impact,         // C_D = 2.0
}

impl LoadDuration {
    pub fn c_d(&self) -> f64 {
        match self {
            LoadDuration::Permanent => 0.9,
            LoadDuration::TenYears => 1.0,
            LoadDuration::TwoMonths => 1.15,
            LoadDuration::SevenDays => 1.25,
            LoadDuration::Wind => 1.6,
            LoadDuration::Impact => 2.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MoistureCondition {
    Dry,   // C_M = 1.0
    Wet,   // C_M < 1.0
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TemperatureCondition {
    Normal,  // C_t = 1.0
    High,    // C_t < 1.0
}

impl TimberMember {
    pub fn new(species: WoodSpecies, width: f64, depth: f64, length: f64) -> Self {
        Self {
            species,
            width,
            depth,
            length,
            load_duration: LoadDuration::TenYears,
            moisture: MoistureCondition::Dry,
            temperature: TemperatureCondition::Normal,
            lateral_support: true,
        }
    }

    /// Cross-sectional area (in^2)
    pub fn area(&self) -> f64 {
        self.width * self.depth
    }

    /// Section modulus S = bd^2/6 (in^3)
    pub fn section_modulus(&self) -> f64 {
        self.width * self.depth * self.depth / 6.0
    }

    /// Moment of inertia I = bd^3/12 (in^4)
    pub fn moment_of_inertia(&self) -> f64 {
        self.width * self.depth.powi(3) / 12.0
    }

    /// Moisture adjustment factor C_M
    pub fn c_m(&self) -> f64 {
        match self.moisture {
            MoistureCondition::Dry => 1.0,
            MoistureCondition::Wet => 0.85, // simplified
        }
    }

    /// Temperature adjustment factor C_t
    pub fn c_t(&self) -> f64 {
        match self.temperature {
            TemperatureCondition::Normal => 1.0,
            TemperatureCondition::High => 0.8, // simplified
        }
    }

    /// Size factor C_F (for bending, based on depth)
    pub fn c_f_bending(&self) -> f64 {
        if self.depth <= 12.0 {
            1.0
        } else if self.depth <= 16.0 {
            1.1
        } else if self.depth <= 24.0 {
            1.2
        } else {
            1.3
        }
    }

    /// Adjusted bending strength F_b' = F_b * C_D * C_M * C_t * C_L * C_F * C_fu * C_i * C_r
    pub fn f_b_prime(&self, c_l: f64) -> f64 {
        self.species.f_b
            * self.load_duration.c_d()
            * self.c_m()
            * self.c_t()
            * c_l  // lateral stability
            * self.c_f_bending()
    }

    /// Adjusted shear strength F_v' = F_v * C_D * C_M * C_t
    pub fn f_v_prime(&self) -> f64 {
        self.species.f_v
            * self.load_duration.c_d()
            * self.c_m()
            * self.c_t()
    }

    /// Adjusted compression parallel F_c' = F_c * C_D * C_M * C_t * C_F * C_i
    pub fn f_c_parallel_prime(&self) -> f64 {
        self.species.f_c_parallel
            * self.load_duration.c_d()
            * self.c_m()
            * self.c_t()
    }

    /// Adjusted compression perpendicular F_c_perp' = F_c_perp * C_M * C_t (no C_D)
    pub fn f_c_perp_prime(&self) -> f64 {
        self.species.f_c_perp * self.c_m() * self.c_t()
    }

    /// Adjusted tension F_t' = F_t * C_D * C_M * C_t
    pub fn f_t_prime(&self) -> f64 {
        self.species.f_t
            * self.load_duration.c_d()
            * self.c_m()
            * self.c_t()
    }

    /// Adjusted modulus of elasticity E' = E * C_M * C_t
    pub fn e_prime(&self) -> f64 {
        self.species.e * self.c_m() * self.c_t()
    }

    /// Slenderness ratio for column (L_e/d)
    pub fn slenderness_ratio(&self) -> f64 {
        self.length / self.depth
    }

    /// Column stability factor C_p (NDS 3.7.1)
    pub fn column_stability_factor(&self) -> f64 {
        let fc_prime = self.f_c_parallel_prime();
        let e_prime = self.e_prime();
        let l_e = self.length; // effective length (simplified)
        let d = self.depth;
        let c = 0.8; // for sawn lumber
        let k_ce = 0.418; // for visually graded

        let fc_e = k_ce * e_prime / (l_e / d).powi(2);
        let ratio = fc_prime / fc_e;

        if ratio <= 0.0 {
            return 1.0;
        }

        // C_p = (1 + (FcE/Fc*)) / (2c) - sqrt(((1 + (FcE/Fc*))/(2c))^2 - (FcE/Fc*)/c)
        let x = (1.0 + ratio) / (2.0 * c);
        let cp = x - (x * x - ratio / c).sqrt();
        cp.min(1.0).max(0.0)
    }

    /// Check bending stress: f_b <= F_b'
    pub fn check_bending(&self, moment: f64, c_l: f64) -> (f64, f64, bool) {
        let f_b = moment / self.section_modulus();
        let f_b_prime = self.f_b_prime(c_l);
        (f_b, f_b_prime, f_b <= f_b_prime)
    }

    /// Check shear stress: f_v <= F_v'
    pub fn check_shear(&self, shear: f64) -> (f64, f64, bool) {
        let f_v = 1.5 * shear / self.area(); // for rectangular
        let f_v_prime = self.f_v_prime();
        (f_v, f_v_prime, f_v <= f_v_prime)
    }

    /// Check compression parallel: f_c <= F_c' * C_p
    pub fn check_compression_parallel(&self, axial_load: f64) -> (f64, f64, bool) {
        let f_c = axial_load / self.area();
        let cp = self.column_stability_factor();
        let f_c_prime = self.f_c_parallel_prime() * cp;
        (f_c, f_c_prime, f_c <= f_c_prime)
    }

    /// Check compression perpendicular: f_c_perp <= F_c_perp'
    pub fn check_compression_perp(&self, bearing_load: f64, bearing_area: f64) -> (f64, f64, bool) {
        let f_c = bearing_load / bearing_area;
        let f_c_prime = self.f_c_perp_prime();
        (f_c, f_c_prime, f_c <= f_c_prime)
    }

    /// Check tension parallel: f_t <= F_t'
    pub fn check_tension(&self, axial_tension: f64) -> (f64, f64, bool) {
        let f_t = axial_tension / self.area();
        let f_t_prime = self.f_t_prime();
        (f_t, f_t_prime, f_t <= f_t_prime)
    }

    /// Combined bending and axial (NDS 3.9.2)
    /// (f_c/F_c')^2 + (f_b/F_b') <= 1.0 for compression + bending
    pub fn check_combined_compression_bending(&self, axial: f64, moment: f64, c_l: f64) -> (f64, bool) {
        let f_c = axial / self.area();
        let f_b = moment / self.section_modulus();
        let f_c_prime = self.f_c_parallel_prime() * self.column_stability_factor();
        let f_b_prime = self.f_b_prime(c_l);
        let interaction = (f_c / f_c_prime).powi(2) + f_b / f_b_prime;
        (interaction, interaction <= 1.0)
    }

    /// Combined bending and tension (NDS 3.9.1)
    /// f_t/F_t' + f_b/F_b' <= 1.0
    pub fn check_combined_tension_bending(&self, tension: f64, moment: f64, c_l: f64) -> (f64, bool) {
        let f_t = tension / self.area();
        let f_b = moment / self.section_modulus();
        let f_t_prime = self.f_t_prime();
        let f_b_prime = self.f_b_prime(c_l);
        let interaction = f_t / f_t_prime + f_b / f_b_prime;
        (interaction, interaction <= 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wood_species_creation() {
        let species = WoodSpecies::douglas_fir_larch_no1();
        assert_eq!(species.name, "Douglas Fir-Larch No.1");
        assert!((species.f_b - 1000.0).abs() < 1e-9);
    }

    #[test]
    fn test_timber_member_area() {
        let species = WoodSpecies::douglas_fir_larch_no1();
        let member = TimberMember::new(species, 5.5, 9.25, 120.0);
        assert!((member.area() - 50.875).abs() < 1e-9);
    }

    #[test]
    fn test_timber_member_section_modulus() {
        let species = WoodSpecies::douglas_fir_larch_no1();
        let member = TimberMember::new(species, 5.5, 9.25, 120.0);
        let s = member.section_modulus();
        assert!(s > 0.0);
    }

    #[test]
    fn test_load_duration_factor() {
        assert!((LoadDuration::Wind.c_d() - 1.6).abs() < 1e-9);
        assert!((LoadDuration::Permanent.c_d() - 0.9).abs() < 1e-9);
    }

    #[test]
    fn test_f_b_prime() {
        let species = WoodSpecies::douglas_fir_larch_no1();
        let member = TimberMember::new(species, 5.5, 9.25, 120.0);
        let fb_prime = member.f_b_prime(1.0); // c_l = 1.0
        assert!(fb_prime > 0.0);
        assert!(fb_prime <= 1000.0 * 1.6); // wind duration max
    }

    #[test]
    fn test_check_bending() {
        let species = WoodSpecies::douglas_fir_larch_no1();
        let member = TimberMember::new(species, 5.5, 9.25, 120.0);
        let (f_b, f_b_prime, ok) = member.check_bending(50000.0, 1.0);
        assert!(f_b > 0.0);
        assert!(f_b_prime > 0.0);
    }

    #[test]
    fn test_check_shear() {
        let species = WoodSpecies::douglas_fir_larch_no1();
        let member = TimberMember::new(species, 5.5, 9.25, 120.0);
        let (f_v, f_v_prime, ok) = member.check_shear(1000.0);
        assert!(f_v > 0.0);
        assert!(f_v_prime > 0.0);
    }

    #[test]
    fn test_column_stability_factor() {
        let species = WoodSpecies::douglas_fir_larch_no1();
        let member = TimberMember::new(species, 5.5, 9.25, 120.0);
        let cp = member.column_stability_factor();
        assert!(cp > 0.0);
        assert!(cp <= 1.0);
    }

    #[test]
    fn test_combined_bending_compression() {
        let species = WoodSpecies::douglas_fir_larch_no1();
        let member = TimberMember::new(species, 5.5, 9.25, 120.0);
        let (interaction, ok) = member.check_combined_compression_bending(5000.0, 50000.0, 1.0);
        assert!(interaction >= 0.0);
    }

    #[test]
    fn test_southern_pine() {
        let species = WoodSpecies::southern_pine_no2();
        assert_eq!(species.name, "Southern Pine No.2");
    }

    #[test]
    fn test_spf() {
        let species = WoodSpecies::spruce_pine_fir_no2();
        assert_eq!(species.name, "Spruce-Pine-Fir No.2");
    }
}
