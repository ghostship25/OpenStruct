use serde::{Deserialize, Serialize};

/// Standard gravity, m/s². The material dialog converts between weight density
/// (kN/m³) and mass density (kg/m³) with it, exactly with both.
pub const GRAVITY: f64 = 9.806_65;

/// Material family, used by materials. Code rules key off this
/// rather than off the material's name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MaterialType {
    #[default]
    Steel,
    Concrete,
    Rebar,
    Tendon,
    Masonry,
    Aluminium,
    ColdFormed,
    None,
}

impl MaterialType {
    pub const ALL: [Self; 8] = [
        Self::Steel,
        Self::Concrete,
        Self::Rebar,
        Self::Tendon,
        Self::Masonry,
        Self::Aluminium,
        Self::ColdFormed,
        Self::None,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Steel => "Steel",
            Self::Concrete => "Concrete",
            Self::Rebar => "Rebar",
            Self::Tendon => "Tendon",
            Self::Masonry => "Masonry",
            Self::Aluminium => "Aluminium",
            Self::ColdFormed => "Cold Formed",
            Self::None => "None",
        }
    }

    /// True when `fy` is a yield strength rather than a concrete f'c.
    pub fn is_steel_like(self) -> bool {
        matches!(self, Self::Steel | Self::ColdFormed | Self::Rebar)
    }

    /// True when `fy` holds a concrete compressive strength.
    pub fn is_concrete_like(self) -> bool {
        matches!(self, Self::Concrete | Self::Masonry)
    }
}

/// Damping data attached to a material: one modal ratio and viscous
/// coefficients per local degree of freedom.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MaterialDamping {
    /// Fraction of critical damping used by modal and response-spectrum cases.
    pub modal_ratio: f64,
    /// Viscous damping coefficients for U1, U2, U3, R1, R2, R3.
    pub viscous: [f64; 6],
}

impl Default for MaterialDamping {
    fn default() -> Self {
        Self {
            modal_ratio: 0.05,
            viscous: [0.0; 6],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Material {
    pub name: String,
    pub e: f64,   // Modulus of elasticity, Pa
    pub nu: f64,  // Poisson's ratio
    pub g: f64,   // Shear modulus, Pa (derived from e and nu)
    pub rho: f64, // Mass density, kg/m^3
    pub fy: f64,  // Yield strength, or f'c for concrete, Pa
    pub fu: f64,  // Ultimate strength, Pa
    /// Coefficient of thermal expansion, 1/°C.
    #[serde(default = "default_alpha")]
    pub alpha: f64,
    /// Material family. Files written before this field existed default to steel.
    #[serde(default)]
    pub material_type: MaterialType,
    /// Modal and viscous damping.
    #[serde(default)]
    pub damping: MaterialDamping,
    /// Display colour in the material list, RGB.
    #[serde(default = "default_material_color")]
    pub color: [u8; 3],
    /// Free-form notes kept with the material.
    #[serde(default)]
    pub notes: String,
}

fn default_alpha() -> f64 {
    1.2e-5
}

/// Neutral material colour for materials written before colours were stored.
pub fn default_material_color() -> [u8; 3] {
    [158, 158, 158]
}

impl Material {
    pub fn new(name: &str, e: f64, nu: f64, rho: f64) -> Self {
        Self::of_type(name, MaterialType::Steel, e, nu, rho)
    }

    pub fn of_type(name: &str, material_type: MaterialType, e: f64, nu: f64, rho: f64) -> Self {
        let g = e / (2.0 * (1.0 + nu));
        Self {
            name: name.to_string(),
            e,
            nu,
            g,
            rho,
            fy: 0.0,
            fu: 0.0,
            alpha: default_alpha(),
            material_type,
            damping: MaterialDamping::default(),
            color: default_material_color(),
            notes: String::new(),
        }
    }

    /// Shear modulus implied by E and ν.
    pub fn shear_modulus(&self) -> f64 {
        self.e / (2.0 * (1.0 + self.nu))
    }

    /// Take G from the current E and ν, so it can never drift from them.
    pub fn sync_shear_modulus(&mut self) {
        self.g = self.shear_modulus();
    }

    /// Weight density in kN/m³, the other way the app lets you enter density.
    pub fn weight_density_kn(&self) -> f64 {
        self.rho * GRAVITY / 1000.0
    }

    /// Set the mass density from a weight density in kN/m³.
    pub fn set_weight_density_kn(&mut self, weight: f64) {
        self.rho = weight * 1000.0 / GRAVITY;
    }

    /// SI units: E and G in Pa, rho in kg/m^3, fy/fu in Pa.
    pub fn steel() -> Self {
        let mut mat = Self::of_type("A992Fy50 STEEL", MaterialType::Steel, 2.0e11, 0.3, 7850.0);
        mat.fy = 345.0e6;
        mat.fu = 450.0e6;
        mat.damping.modal_ratio = 0.02;
        mat
    }

    pub fn concrete() -> Self {
        let mut mat = Self::of_type("4000PSI CONCRETE", MaterialType::Concrete, 2.5e10, 0.2, 2400.0);
        mat.fy = 27.6e6; // f'c ~ 4000 psi
        mat.alpha = 1.0e-5;
        mat
    }

    pub fn aluminum() -> Self {
        let mut mat = Self::of_type("ALUMINUM", MaterialType::Aluminium, 7.0e10, 0.33, 2700.0);
        mat.fy = 240.0e6;
        mat.alpha = 2.3e-5;
        mat.damping.modal_ratio = 0.02;
        mat
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shear_modulus_follows_e_and_nu() {
        let material = Material::of_type("S355", MaterialType::Steel, 210.0e9, 0.3, 7850.0);
        assert!((material.shear_modulus() - 80.769_230_769e9).abs() < 1.0e6);
        let mut material = material;
        material.nu = 0.25;
        material.sync_shear_modulus();
        assert!((material.g - 84.0e9).abs() < 1.0e3);
    }

    #[test]
    fn weight_and_mass_density_convert_with_standard_gravity() {
        // The display shows 7849.047 kg/m³ as 76.9729 kN/m³, which is ρ·g/1000.
        let expected = 7849.047 * GRAVITY / 1000.0;
        assert!((expected - 76.9729).abs() < 0.001, "the screenshot pair must agree");

        let material = Material::steel();
        assert!((material.weight_density_kn() - 7850.0 * GRAVITY / 1000.0).abs() < 1.0e-9);

        let mut other = Material::steel();
        other.set_weight_density_kn(76.9729);
        assert!((other.rho - 7849.047).abs() < 0.01);
    }

    #[test]
    fn presets_carry_their_material_type() {
        assert_eq!(Material::steel().material_type, MaterialType::Steel);
        assert_eq!(Material::concrete().material_type, MaterialType::Concrete);
        assert_eq!(Material::aluminum().material_type, MaterialType::Aluminium);
        assert!(MaterialType::Concrete.is_concrete_like());
        assert!(!MaterialType::Concrete.is_steel_like());
    }

    #[test]
    fn older_files_without_type_or_damping_still_load() {
        let json = r#"{"name":"A992Fy50","e":200000000000.0,"nu":0.3,"g":76923076923.0,"rho":7850.0,"fy":345000000.0,"fu":450000000.0}"#;
        let material: Material = serde_json::from_str(json).expect("old material must load");
        assert_eq!(material.material_type, MaterialType::Steel);
        assert_eq!(material.damping, MaterialDamping::default());
        assert!((material.alpha - 1.2e-5).abs() < 1.0e-12);
        assert_eq!(material.color, default_material_color());
        assert!(material.notes.is_empty());
    }

    #[test]
    fn colour_and_notes_round_trip() {
        let mut material = Material::steel();
        material.color = [12, 200, 90];
        material.notes = "Mill certificate on file.".to_string();
        let text = serde_json::to_string(&material).unwrap();
        let back: Material = serde_json::from_str(&text).unwrap();
        assert_eq!(back.color, material.color);
        assert_eq!(back.notes, material.notes);
    }
}
