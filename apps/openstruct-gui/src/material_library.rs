//! Region → material type → standard → grade library behind
//! "Add New Material…", mirroring regional material libraries.
//!
//! Every grade carries the numbers the reference grades carry for the same name: modulus,
//! Poisson's ratio, thermal expansion, density, and yield strength (or f'c).
//! Concrete moduli follow the code formulas (5000√f'c MPa for IS 456,
//! 4700√f'c MPa for ACI 318, 22(fcm/10)^0.3 GPa for EN 1992).

use openstruct::materials::{Material, MaterialType};

pub const REGIONS: [&str; 4] = ["India", "United States", "Europe", "Nepal"];

#[derive(Clone, Copy)]
pub struct Grade {
    pub name: &'static str,
    pub e: f64,
    pub fy: f64,
    pub fu: f64,
    pub rho: f64,
    pub nu: f64,
    pub alpha: f64,
}

const fn grade(name: &'static str, e: f64, fy: f64, fu: f64, rho: f64, nu: f64, alpha: f64) -> Grade {
    Grade { name, e, fy, fu, rho, nu, alpha }
}

/// Steel: E 200-210 GPa, ν 0.3, α 1.2e-5, 7850 kg/m³.
const STEEL_INDIA: [Grade; 5] = [
    grade("Fe345", 200.0e9, 345.0e6, 490.0e6, 7850.0, 0.3, 1.2e-5),
    grade("Fe415", 200.0e9, 415.0e6, 540.0e6, 7850.0, 0.3, 1.2e-5),
    grade("Fe500", 200.0e9, 500.0e6, 620.0e6, 7850.0, 0.3, 1.2e-5),
    grade("Fe550", 200.0e9, 550.0e6, 670.0e6, 7850.0, 0.3, 1.2e-5),
    grade("Fe690", 200.0e9, 690.0e6, 770.0e6, 7850.0, 0.3, 1.2e-5),
];

const STEEL_US: [Grade; 4] = [
    grade("A992Fy50", 200.0e9, 345.0e6, 450.0e6, 7850.0, 0.3, 1.2e-5),
    grade("A36", 200.0e9, 250.0e6, 400.0e6, 7850.0, 0.3, 1.2e-5),
    grade("A572Gr50", 200.0e9, 345.0e6, 450.0e6, 7850.0, 0.3, 1.2e-5),
    grade("A913Gr65", 200.0e9, 450.0e6, 550.0e6, 7850.0, 0.3, 1.2e-5),
];

const STEEL_EUROPE: [Grade; 5] = [
    grade("S235", 210.0e9, 235.0e6, 360.0e6, 7850.0, 0.3, 1.2e-5),
    grade("S275", 210.0e9, 275.0e6, 430.0e6, 7850.0, 0.3, 1.2e-5),
    grade("S355", 210.0e9, 355.0e6, 510.0e6, 7850.0, 0.3, 1.2e-5),
    grade("S420", 210.0e9, 420.0e6, 520.0e6, 7850.0, 0.3, 1.2e-5),
    grade("S460", 210.0e9, 460.0e6, 550.0e6, 7850.0, 0.3, 1.2e-5),
];

/// Concrete: f'c in `fy`, E from the regional code formula.
const CONCRETE_INDIA: [Grade; 6] = [
    grade("M20", 22.36e9, 20.0e6, 0.0, 2400.0, 0.2, 1.0e-5),
    grade("M25", 25.00e9, 25.0e6, 0.0, 2400.0, 0.2, 1.0e-5),
    grade("M30", 27.39e9, 30.0e6, 0.0, 2400.0, 0.2, 1.0e-5),
    grade("M35", 29.58e9, 35.0e6, 0.0, 2400.0, 0.2, 1.0e-5),
    grade("M40", 31.62e9, 40.0e6, 0.0, 2400.0, 0.2, 1.0e-5),
    grade("M45", 33.54e9, 45.0e6, 0.0, 2400.0, 0.2, 1.0e-5),
];

const CONCRETE_US: [Grade; 4] = [
    grade("3000Psi", 21.38e9, 20.68e6, 0.0, 2400.0, 0.2, 1.0e-5),
    grade("4000Psi", 24.69e9, 27.58e6, 0.0, 2400.0, 0.2, 1.0e-5),
    grade("5000Psi", 27.60e9, 34.47e6, 0.0, 2400.0, 0.2, 1.0e-5),
    grade("6000Psi", 30.23e9, 41.37e6, 0.0, 2400.0, 0.2, 1.0e-5),
];

const CONCRETE_EUROPE: [Grade; 5] = [
    grade("C20/25", 29.96e9, 20.0e6, 0.0, 2400.0, 0.2, 1.0e-5),
    grade("C25/30", 31.48e9, 25.0e6, 0.0, 2400.0, 0.2, 1.0e-5),
    grade("C30/37", 32.84e9, 30.0e6, 0.0, 2400.0, 0.2, 1.0e-5),
    grade("C35/45", 34.08e9, 35.0e6, 0.0, 2400.0, 0.2, 1.0e-5),
    grade("C40/50", 35.22e9, 40.0e6, 0.0, 2400.0, 0.2, 1.0e-5),
];

const REBAR_INDIA: [Grade; 3] = [
    grade("Fe415", 200.0e9, 415.0e6, 485.0e6, 7850.0, 0.3, 1.2e-5),
    grade("Fe500", 200.0e9, 500.0e6, 545.0e6, 7850.0, 0.3, 1.2e-5),
    grade("Fe550", 200.0e9, 550.0e6, 585.0e6, 7850.0, 0.3, 1.2e-5),
];

const REBAR_US: [Grade; 3] = [
    grade("A615Gr40", 200.0e9, 280.0e6, 420.0e6, 7850.0, 0.3, 1.2e-5),
    grade("A615Gr60", 200.0e9, 420.0e6, 620.0e6, 7850.0, 0.3, 1.2e-5),
    grade("A615Gr75", 200.0e9, 520.0e6, 690.0e6, 7850.0, 0.3, 1.2e-5),
];

const REBAR_EUROPE: [Grade; 2] = [
    grade("B500A", 200.0e9, 500.0e6, 550.0e6, 7850.0, 0.3, 1.2e-5),
    grade("B500B", 200.0e9, 500.0e6, 550.0e6, 7850.0, 0.3, 1.2e-5),
];

const TENDON_INDIA: [Grade; 2] = [
    grade("7-wire 1670", 195.0e9, 1670.0e6, 1860.0e6, 7850.0, 0.3, 1.2e-5),
    grade("7-wire 1770", 195.0e9, 1770.0e6, 1960.0e6, 7850.0, 0.3, 1.2e-5),
];

const TENDON_US: [Grade; 2] = [
    grade("A416Gr250", 195.0e9, 1725.0e6, 1860.0e6, 7850.0, 0.3, 1.2e-5),
    grade("A416Gr270", 195.0e9, 1860.0e6, 1860.0e6, 7850.0, 0.3, 1.2e-5),
];

const TENDON_EUROPE: [Grade; 2] = [
    grade("Y1770S7", 195.0e9, 1770.0e6, 1960.0e6, 7850.0, 0.3, 1.2e-5),
    grade("Y1860S7", 195.0e9, 1860.0e6, 2060.0e6, 7850.0, 0.3, 1.2e-5),
];

const MASONRY_GRADES: [Grade; 3] = [
    grade("Masonry 10", 8.0e9, 10.0e6, 0.0, 2000.0, 0.2, 1.0e-5),
    grade("Masonry 15", 10.0e9, 15.0e6, 0.0, 2000.0, 0.2, 1.0e-5),
    grade("Masonry 20", 12.0e9, 20.0e6, 0.0, 2000.0, 0.2, 1.0e-5),
];

const ALUMINIUM_US: [Grade; 3] = [
    grade("6061T6", 69.0e9, 240.0e6, 290.0e6, 2700.0, 0.33, 2.3e-5),
    grade("6063T5", 69.0e9, 145.0e6, 185.0e6, 2700.0, 0.33, 2.3e-5),
    grade("5052H32", 70.0e9, 193.0e6, 228.0e6, 2680.0, 0.33, 2.4e-5),
];

const ALUMINIUM_EUROPE: [Grade; 2] = [
    grade("EN AW-6060 T6", 69.0e9, 150.0e6, 190.0e6, 2700.0, 0.33, 2.3e-5),
    grade("EN AW-6082 T6", 70.0e9, 260.0e6, 310.0e6, 2700.0, 0.33, 2.3e-5),
];

const COLD_FORMED_US: [Grade; 2] = [
    grade("A653SQGr50", 203.0e9, 345.0e6, 450.0e6, 7850.0, 0.3, 1.2e-5),
    grade("A653SQGr33", 203.0e9, 228.0e6, 310.0e6, 7850.0, 0.3, 1.2e-5),
];

/// The standard a region pairs with a material type.
pub fn standards(region: usize, material_type: MaterialType) -> &'static [&'static str] {
    match material_type {
        MaterialType::None => &["—"],
        MaterialType::Steel => match region {
            1 => &["ASTM"],
            2 => &["EN 10025"],
            3 => &["IS 2062 (NBC 105)"],
            _ => &["IS 2062"],
        },
        MaterialType::Concrete => match region {
            1 => &["ACI 318"],
            2 => &["EN 1992"],
            3 => &["IS 456 (NBC 105)"],
            _ => &["IS 456"],
        },
        MaterialType::Rebar => match region {
            1 => &["ASTM A615"],
            2 => &["EN 10080"],
            _ => &["IS 1786"],
        },
        MaterialType::Tendon => match region {
            1 => &["ASTM A416"],
            2 => &["EN 10138"],
            _ => &["IS 6006"],
        },
        MaterialType::Masonry => match region {
            1 => &["TMS 402"],
            2 => &["EN 1996"],
            _ => &["IS 1905"],
        },
        MaterialType::Aluminium => match region {
            1 => &["AA ADM"],
            2 => &["EN 755"],
            _ => &["IS 733"],
        },
        MaterialType::ColdFormed => match region {
            1 => &["AISI S100"],
            2 => &["EN 1993-1-3"],
            _ => &["IS 801"],
        },
    }
}

/// Grades available for a region, material type and standard.
pub fn grades(region: usize, material_type: MaterialType, _standard: usize) -> &'static [Grade] {
    match material_type {
        MaterialType::Steel => match region {
            1 => &STEEL_US,
            2 => &STEEL_EUROPE,
            _ => &STEEL_INDIA,
        },
        MaterialType::Concrete => match region {
            1 => &CONCRETE_US,
            2 => &CONCRETE_EUROPE,
            _ => &CONCRETE_INDIA,
        },
        MaterialType::Rebar => match region {
            1 => &REBAR_US,
            2 => &REBAR_EUROPE,
            _ => &REBAR_INDIA,
        },
        MaterialType::Tendon => match region {
            1 => &TENDON_US,
            2 => &TENDON_EUROPE,
            _ => &TENDON_INDIA,
        },
        MaterialType::Masonry => &MASONRY_GRADES,
        MaterialType::Aluminium => match region {
            0 => &ALUMINIUM_US,
            _ if region == 1 => &ALUMINIUM_US,
            2 => &ALUMINIUM_EUROPE,
            _ => &ALUMINIUM_US,
        },
        MaterialType::ColdFormed => &COLD_FORMED_US,
        MaterialType::None => &[],
    }
}

/// Build a material from a library choice, ready to be named by the caller.
pub fn material_from(region: usize, material_type: MaterialType, standard: usize, grade_index: usize) -> Material {
    let list = grades(region, material_type, standard);
    let picked = list.get(grade_index).copied().unwrap_or(Grade {
        name: "Material",
        e: 200.0e9,
        fy: 0.0,
        fu: 0.0,
        rho: 7850.0,
        nu: 0.3,
        alpha: 1.2e-5,
    });
    let mut material = Material::of_type(picked.name, material_type, picked.e, picked.nu, picked.rho);
    material.fy = picked.fy;
    material.fu = picked.fu;
    material.alpha = picked.alpha;
    if material_type.is_concrete_like() {
        material.damping.modal_ratio = 0.05;
    } else if material_type.is_steel_like() {
        material.damping.modal_ratio = 0.02;
    }
    material
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_region_offers_grades_for_steel_and_concrete() {
        for region in 0..REGIONS.len() {
            let steel = grades(region, MaterialType::Steel, 0);
            let concrete = grades(region, MaterialType::Concrete, 0);
            assert!(!steel.is_empty(), "no steel grades for region {region}");
            assert!(!concrete.is_empty(), "no concrete grades for region {region}");
            assert!(!standards(region, MaterialType::Steel)[0].is_empty());
        }
    }

    #[test]
    fn india_steel_starts_at_fe345_and_europe_at_s235() {
        assert_eq!(grades(0, MaterialType::Steel, 0)[0].name, "Fe345");
        assert_eq!(grades(2, MaterialType::Steel, 0)[0].name, "S235");
        assert_eq!(grades(1, MaterialType::Steel, 0)[0].name, "A992Fy50");
    }

    #[test]
    fn grade_becomes_a_typed_material() {
        let material = material_from(0, MaterialType::Concrete, 0, 2);
        assert_eq!(material.name, "M30");
        assert_eq!(material.material_type, MaterialType::Concrete);
        assert!((material.fy - 30.0e6).abs() < 1.0);
        assert!((material.g - material.shear_modulus()).abs() < 1.0);
    }

    #[test]
    fn unknown_grade_falls_back_instead_of_panicking() {
        let material = material_from(0, MaterialType::Steel, 0, 99);
        assert!(material.e > 0.0);
    }
}
