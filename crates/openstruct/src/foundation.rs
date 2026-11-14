//! Foundation Design (ACI 318).
//!
//! Implements foundation design calculations:
//! - Isolated (spread) footings
//! - Combined footings
//! - Mat (raft) foundations
//! - Pile caps
//! - Bearing capacity checks
//! - Punching shear and one-way shear checks
//!
//! Reference: ACI 318-19, Chapter 13 (Foundations) and Chapter 22 (Shear)

/// Soil bearing capacity parameters
#[derive(Debug, Clone)]
pub struct SoilProperties {
    /// Allowable bearing pressure (psf or kPa)
    pub allowable_bearing: f64,
    pub gamma_soil: f64,
    pub cohesion: f64,
    pub friction_angle: f64,
    pub k_s: f64,
}

impl SoilProperties {
    pub fn default_clay() -> Self {
        Self {
            allowable_bearing: 2000.0, // psf
            gamma_soil: 120.0,
            cohesion: 500.0,
            friction_angle: 0.0,
            k_s: 100.0, // pci
        }
    }

    pub fn default_sand() -> Self {
        Self {
            allowable_bearing: 3000.0,
            gamma_soil: 120.0,
            cohesion: 0.0,
            friction_angle: 30.0,
            k_s: 200.0,
        }
    }
}

/// Isolated (spread) footing design
#[derive(Debug, Clone)]
pub struct IsolatedFooting {
    pub column_width: f64,
    pub column_depth: f64,
    pub footing_width: f64,
    pub footing_length: f64,
    pub footing_thickness: f64,
    pub concrete_cover: f64,
    pub fc: f64,
    pub fy: f64,
    pub axial_load: f64,
    pub moment_x: f64,
    pub moment_y: f64,
    pub soil: SoilProperties,
}

impl IsolatedFooting {
    pub fn new(col_w: f64, col_d: f64, soil: SoilProperties) -> Self {
        Self {
            column_width: col_w,
            column_depth: col_d,
            footing_width: 0.0,
            footing_length: 0.0,
            footing_thickness: 0.0,
            concrete_cover: 3.0, // inches
            fc: 3000.0, // psi
            fy: 60000.0, // psi
            axial_load: 0.0,
            moment_x: 0.0,
            moment_y: 0.0,
            soil,
        }
    }

    /// Required footing area for bearing
    pub fn required_area(&self) -> f64 {
        let q_net = self.soil.allowable_bearing;
        (self.axial_load / q_net).max(0.0)
    }

    /// Size footing for square shape given required area
    pub fn size_square(&mut self) {
        let area = self.required_area();
        let side = area.sqrt();
        // Round up to nearest 6 inches (0.5 ft)
        self.footing_width = (side / 0.5).ceil() * 0.5;
        self.footing_length = self.footing_width;
    }

    /// Size footing for rectangular shape given aspect ratio
    pub fn size_rectangular(&mut self, aspect_ratio: f64) {
        let area = self.required_area();
        let w = (area / aspect_ratio).sqrt();
        let l = w * aspect_ratio;
        self.footing_width = (w / 0.5).ceil() * 0.5;
        self.footing_length = (l / 0.5).ceil() * 0.5;
    }

    /// Effective depth (d) of footing
    pub fn effective_depth(&self) -> f64 {
        self.footing_thickness - self.concrete_cover - 0.5 // 0.5" bar radius approx
    }

    /// Check one-way shear (ACI 318-19 22.5)
    /// Returns (demand, capacity, ok)
    pub fn check_one_way_shear(&self, direction: ShearDirection) -> (f64, f64, bool) {
        let d = self.effective_depth();
        let (b, cantilever) = match direction {
            ShearDirection::Width => (self.footing_length, (self.footing_width - self.column_width) / 2.0),
            ShearDirection::Length => (self.footing_width, (self.footing_length - self.column_depth) / 2.0),
        };
        // Shear demand: V_u = q_u * b * (cantilever - d) ... simplified
        let q_u = self.axial_load / (self.footing_width * self.footing_length);
        let v_demand = q_u * b * (cantilever - d).max(0.0);

        // Shear capacity: phi * V_c = phi * 2 * lambda * sqrt(fc) * b * d (psi units)
        let phi = 0.75;
        let lambda = 1.0;
        let v_capacity = phi * 2.0 * lambda * self.fc.sqrt() * b * d;

        (v_demand, v_capacity, v_demand <= v_capacity)
    }

    /// Check punching shear (ACI 318-19 22.6)
    /// Returns (demand, capacity, ok)
    pub fn check_punching_shear(&self) -> (f64, f64, bool) {
        let d = self.effective_depth();
        // Critical perimeter: b_o = 2*(col_w + d) + 2*(col_d + d)
        let b_o = 2.0 * (self.column_width + d) + 2.0 * (self.column_depth + d);
        // Punching shear area
        let area_punch = self.footing_width * self.footing_length
            - (self.column_width + d) * (self.column_depth + d);
        let q_u = self.axial_load / (self.footing_width * self.footing_length);
        let v_demand = self.axial_load - q_u * (self.column_width + d) * (self.column_depth + d);

        // Capacity: phi * V_c = phi * 4 * lambda * sqrt(fc) * b_o * d (psi)
        let phi = 0.75;
        let lambda = 1.0;
        let v_capacity = phi * 4.0 * lambda * self.fc.sqrt() * b_o * d;

        (v_demand, v_capacity, v_demand <= v_capacity)
    }

    /// Check footing bearing on column (ACI 318-19 22.8)
    pub fn check_column_bearing(&self) -> (f64, f64, bool) {
        // phi * P_n = phi * 0.85 * fc * A1 (where A1 = column area)
        let phi = 0.65;
        let a1 = self.column_width * self.column_depth;
        let p_capacity = phi * 0.85 * self.fc * a1;
        (self.axial_load, p_capacity, self.axial_load <= p_capacity)
    }

    /// Calculate required flexural reinforcement (simplified)
    pub fn required_reinforcement(&self, direction: ReinforcementDirection) -> f64 {
        let d = self.effective_depth();
        let (b, l, cantilever) = match direction {
            ReinforcementDirection::Width => {
                (self.footing_length, self.footing_width, (self.footing_width - self.column_width) / 2.0)
            }
            ReinforcementDirection::Length => {
                (self.footing_width, self.footing_length, (self.footing_length - self.column_depth) / 2.0)
            }
        };
        let q_u = self.axial_load / (self.footing_width * self.footing_length);
        let m_u = q_u * b * cantilever * cantilever / 2.0;
        // As = M_u / (phi * fy * (d - a/2)) simplified: As = M_u / (phi * fy * 0.9*d)
        let phi = 0.9;
        let as_req = m_u / (phi * self.fy * 0.9 * d);
        as_req.max(0.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShearDirection {
    Width,
    Length,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ReinforcementDirection {
    Width,
    Length,
}

/// Combined footing design
#[derive(Debug, Clone)]
pub struct CombinedFooting {
    pub width: f64,
    pub length: f64,
    pub thickness: f64,
    pub columns: Vec<(f64, f64, f64)>, // (x, y, load) for each column
    pub soil: SoilProperties,
    pub fc: f64,
    pub fy: f64,
}

impl CombinedFooting {
    pub fn new(soil: SoilProperties) -> Self {
        Self {
            width: 0.0,
            length: 0.0,
            thickness: 0.0,
            columns: Vec::new(),
            soil,
            fc: 3000.0,
            fy: 60000.0,
        }
    }

    pub fn add_column(&mut self, x: f64, y: f64, load: f64) {
        self.columns.push((x, y, load));
    }

    /// Total load
    pub fn total_load(&self) -> f64 {
        self.columns.iter().map(|(_, _, load)| load).sum()
    }

    /// Required area
    pub fn required_area(&self) -> f64 {
        self.total_load() / self.soil.allowable_bearing
    }

    /// Size footing (uniform width, length covers all columns plus margins)
    pub fn size(&mut self, margin: f64) {
        let area = self.required_area();
        let min_x = self.columns.iter().map(|(x, _, _)| *x).fold(f64::INFINITY, f64::min);
        let max_x = self.columns.iter().map(|(x, _, _)| *x).fold(f64::NEG_INFINITY, f64::max);
        self.length = max_x - min_x + 2.0 * margin;
        self.width = (area / self.length / 0.5).ceil() * 0.5;
        if self.width < 1.0 { self.width = 1.0; }
    }

    /// Check if resultant is within middle third (no tension)
    pub fn check_middle_third(&self) -> bool {
        if self.columns.is_empty() { return true; }
        let total = self.total_load();
        let moment = self.columns.iter().map(|(x, _, load)| x * load).sum::<f64>();
        let centroid = moment / total;
        let middle_third_start = self.length / 3.0;
        let middle_third_end = 2.0 * self.length / 3.0;
        centroid >= middle_third_start && centroid <= middle_third_end
    }
}

/// Pile foundation
#[derive(Debug, Clone)]
pub struct PileFoundation {
    pub pile_diameter: f64,
    pub pile_length: f64,
    pub pile_capacity: f64, // allowable capacity per pile
    pub piles: Vec<(f64, f64)>, // (x, y) positions
    pub cap_width: f64,
    pub cap_length: f64,
    pub cap_thickness: f64,
    pub total_load: f64,
}

impl PileFoundation {
    pub fn new(pile_dia: f64, pile_len: f64, capacity: f64) -> Self {
        Self {
            pile_diameter: pile_dia,
            pile_length: pile_len,
            pile_capacity: capacity,
            piles: Vec::new(),
            cap_width: 0.0,
            cap_length: 0.0,
            cap_thickness: 0.0,
            total_load: 0.0,
        }
    }

    pub fn add_pile(&mut self, x: f64, y: f64) {
        self.piles.push((x, y));
    }

    /// Number of piles required
    pub fn required_piles(&self) -> usize {
        (self.total_load / self.pile_capacity).ceil() as usize
    }

    /// Check pile group efficiency (simplified)
    pub fn group_efficiency(&self) -> f64 {
        // Converse-Labarre formula (simplified)
        let n = self.piles.len() as f64;
        if n <= 1.0 { return 1.0; }
        let rows = (n.sqrt()).ceil();
        let cols = (n / rows).ceil();
        let theta = 0.0; // angle between piles (simplified)
        let efficiency = 1.0 - (rows - 1.0) * cols * theta / (90.0 * n)
            - (cols - 1.0) * rows * theta / (90.0 * n);
        efficiency.max(0.4)
    }

    /// Check if any pile exceeds capacity
    pub fn check_pile_capacities(&self) -> Vec<(usize, f64, bool)> {
        let mut results = Vec::new();
        let n = self.piles.len() as f64;
        let load_per_pile = self.total_load / n;
        for (i, _) in self.piles.iter().enumerate() {
            results.push((i, load_per_pile, load_per_pile <= self.pile_capacity));
        }
        results
    }
}

/// Mat (raft) foundation
#[derive(Debug, Clone)]
pub struct MatFoundation {
    pub width: f64,
    pub length: f64,
    pub thickness: f64,
    pub soil: SoilProperties,
    pub column_loads: Vec<(f64, f64, f64)>, // (x, y, load)
    pub fc: f64,
    pub fy: f64,
}

impl MatFoundation {
    pub fn new(width: f64, length: f64, soil: SoilProperties) -> Self {
        Self {
            width,
            length,
            thickness: 0.0,
            soil,
            column_loads: Vec::new(),
            fc: 3000.0,
            fy: 60000.0,
        }
    }

    pub fn add_column_load(&mut self, x: f64, y: f64, load: f64) {
        self.column_loads.push((x, y, load));
    }

    /// Total load
    pub fn total_load(&self) -> f64 {
        self.column_loads.iter().map(|(_, _, load)| load).sum()
    }

    /// Average soil pressure
    pub fn average_pressure(&self) -> f64 {
        self.total_load() / (self.width * self.length)
    }

    /// Check bearing
    pub fn check_bearing(&self) -> bool {
        self.average_pressure() <= self.soil.allowable_bearing
    }

    /// Effective depth
    pub fn effective_depth(&self) -> f64 {
        self.thickness - 3.0 - 0.5 // cover + bar
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_isolated_footing_creation() {
        let soil = SoilProperties::default_clay();
        let footing = IsolatedFooting::new(12.0, 12.0, soil);
        assert_eq!(footing.column_width, 12.0);
    }

    #[test]
    fn test_isolated_footing_area() {
        let mut soil = SoilProperties::default_clay();
        soil.allowable_bearing = 2000.0;
        let mut footing = IsolatedFooting::new(12.0, 12.0, soil);
        footing.axial_load = 100000.0; // 100 kips
        let area = footing.required_area();
        assert!((area - 50.0).abs() < 1e-9);
    }

    #[test]
    fn test_isolated_footing_size_square() {
        let mut soil = SoilProperties::default_clay();
        soil.allowable_bearing = 2000.0;
        let mut footing = IsolatedFooting::new(12.0, 12.0, soil);
        footing.axial_load = 100000.0;
        footing.size_square();
        assert!(footing.footing_width >= 7.0);
        assert_eq!(footing.footing_width, footing.footing_length);
    }

    #[test]
    fn test_punching_shear() {
        let mut soil = SoilProperties::default_clay();
        soil.allowable_bearing = 2000.0;
        let mut footing = IsolatedFooting::new(12.0, 12.0, soil);
        footing.axial_load = 100000.0;
        footing.size_square();
        footing.footing_thickness = 18.0;
        let (demand, capacity, ok) = footing.check_punching_shear();
        // Just verify it runs
        assert!(capacity > 0.0);
    }

    #[test]
    fn test_combined_footing() {
        let soil = SoilProperties::default_clay();
        let mut cf = CombinedFooting::new(soil);
        cf.add_column(2.0, 0.0, 50000.0);
        cf.add_column(8.0, 0.0, 50000.0);
        assert!((cf.total_load() - 100000.0).abs() < 1e-9);
        cf.size(1.0);
        assert!(cf.length > 0.0);
        assert!(cf.width > 0.0);
    }

    #[test]
    fn test_combined_footing_middle_third() {
        let soil = SoilProperties::default_clay();
        let mut cf = CombinedFooting::new(soil);
        cf.add_column(2.0, 0.0, 50000.0);
        cf.add_column(8.0, 0.0, 50000.0);
        cf.size(1.0);
        assert!(cf.check_middle_third());
    }

    #[test]
    fn test_pile_foundation() {
        let mut pf = PileFoundation::new(12.0, 30.0, 100000.0);
        pf.total_load = 500000.0;
        pf.add_pile(0.0, 0.0);
        pf.add_pile(5.0, 0.0);
        pf.add_pile(0.0, 5.0);
        pf.add_pile(5.0, 5.0);
        assert_eq!(pf.required_piles(), 5);
        let results = pf.check_pile_capacities();
        assert_eq!(results.len(), 4);
    }

    #[test]
    fn test_mat_foundation() {
        let soil = SoilProperties::default_clay();
        let mut mf = MatFoundation::new(20.0, 30.0, soil);
        mf.add_column_load(5.0, 5.0, 100000.0);
        mf.add_column_load(15.0, 5.0, 100000.0);
        mf.add_column_load(5.0, 25.0, 100000.0);
        mf.add_column_load(15.0, 25.0, 100000.0);
        assert!((mf.total_load() - 400000.0).abs() < 1e-9);
        assert!(mf.check_bearing());
    }

    #[test]
    fn test_soil_properties() {
        let soil = SoilProperties::default_sand();
        assert_eq!(soil.friction_angle, 30.0);
        assert_eq!(soil.cohesion, 0.0);
    }
}
