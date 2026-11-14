//! Moving Load Analysis via Influence Lines.
//!
//! Implements influence line computation for structural members under moving loads:
//! - Influence lines for shear and moment at any section
//! - Moving load envelopes (max/min response)
//! - Standard AASHTO HL-93 truck load definition
//! - Application of moving loads to frame elements
//!
//! Reference: AASHTO LRFD Bridge Design Specifications, Section 3.6



/// A point on an influence line
#[derive(Debug, Clone, Copy)]
pub struct InfluencePoint {
    /// Position along the member (0.0 to 1.0 normalized, or actual length)
    pub x: f64,
    pub ordinate: f64,
}

/// Influence line for a specific response (shear or moment) at a section
#[derive(Debug, Clone)]
pub struct InfluenceLine {
    /// Member element ID
    pub element_id: usize,
    pub section_x: f64,
    pub response_type: ResponseType,
    pub points: Vec<InfluencePoint>,
}

/// Type of structural response
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ResponseType {
    Shear,
    Moment,
    Deflection,
}

/// Moving load definition (truck or lane load)
#[derive(Debug, Clone)]
pub struct MovingLoad {
    /// Load identifier
    pub id: String,
    pub load_type: MovingLoadType,
    pub axles: Vec<(f64, f64)>, // (load, spacing_from_previous)
    /// Total length of the load (ft or m)
    pub total_length: f64,
}

/// Moving load type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MovingLoadType {
    /// AASHTO HL-93 design truck
    HL93Truck,
    HL93Tandem,
    LaneLoad,
    Custom,
}

impl MovingLoad {
    /// Create AASHTO HL-93 design truck
    /// Axle loads: 8 kips, 32 kips, 32 kips
    /// Spacings: 14 ft, 14-30 ft (variable)
    pub fn hl93_truck() -> Self {
        Self {
            id: "HL-93 Truck".to_string(),
            load_type: MovingLoadType::HL93Truck,
            axles: vec![
                (8.0, 0.0),   // front axle: 8 kips at origin
                (32.0, 14.0), // middle axle: 32 kips, 14 ft behind
                (32.0, 14.0), // rear axle: 32 kips, 14 ft behind middle (min spacing)
            ],
            total_length: 28.0, // minimum 28 ft
        }
    }

    /// Create AASHTO HL-93 design tandem
    /// Two 25-kip axles spaced 4 ft apart
    pub fn hl93_tandem() -> Self {
        Self {
            id: "HL-93 Tandem".to_string(),
            load_type: MovingLoadType::HL93Tandem,
            axles: vec![
                (25.0, 0.0), // first axle
                (25.0, 4.0), // second axle, 4 ft behind
            ],
            total_length: 4.0,
        }
    }

    /// Create uniform lane load (0.64 kips/ft or 9.34 kN/m)
    pub fn lane_load(intensity: f64, length: f64) -> Self {
        Self {
            id: "Lane Load".to_string(),
            load_type: MovingLoadType::LaneLoad,
            axles: vec![(intensity, 0.0)], // treated as distributed
            total_length: length,
        }
    }

    /// Total load magnitude
    pub fn total_load(&self) -> f64 {
        self.axles.iter().map(|(load, _)| load).sum()
    }
}

/// Moving load analysis results
#[derive(Debug, Clone)]
pub struct MovingLoadResults {
    /// Element ID
    pub element_id: usize,
    pub max_shear: Vec<(f64, f64)>, // (section_x, value)
    /// Minimum shear at each section
    pub min_shear: Vec<(f64, f64)>,
    pub max_moment: Vec<(f64, f64)>,
    pub min_moment: Vec<(f64, f64)>,
    pub critical_shear_position: f64,
    pub critical_moment_position: f64,
}

/// Compute influence line for a simply supported beam
/// For a beam of length L, section at distance a from left support:
/// - Shear influence: IL_V(x) = (L-a)/L for x < a, IL_V(x) = -a/L for x > a
/// - Moment influence: IL_M(x) = (L-a)*x/L for x < a, IL_M(x) = a*(L-x)/L for x > a
pub fn influence_line_simple_beam(
    length: f64,
    section_a: f64,
    response_type: ResponseType,
    num_points: usize,
) -> Vec<InfluencePoint> {
    let mut points = Vec::with_capacity(num_points);
    for i in 0..num_points {
        let x = length * (i as f64) / ((num_points - 1) as f64).max(1.0);
        let ordinate = match response_type {
            ResponseType::Shear => {
                if x < section_a - 1e-9 {
                    (length - section_a) / length
                } else if x > section_a + 1e-9 {
                    -section_a / length
                } else {
                    // At the section, shear has a discontinuity; use average
                    0.5 * ((length - section_a) / length - section_a / length)
                }
            }
            ResponseType::Moment => {
                if x <= section_a {
                    (length - section_a) * x / length
                } else {
                    section_a * (length - x) / length
                }
            }
            ResponseType::Deflection => {
                // Simplified deflection influence (unit load at x)
                if x <= section_a {
                    x * (3.0 * section_a - x) / (6.0 * length)
                } else {
                    section_a * (3.0 * x - section_a) / (6.0 * length)
                }
            }
        };
        points.push(InfluencePoint { x, ordinate });
    }
    points
}

/// Compute moving load envelope for a simply supported beam
/// Returns max/min shear and moment at each section
pub fn moving_load_envelope(
    length: f64,
    loads: &[MovingLoad],
    num_sections: usize,
    num_positions: usize,
) -> MovingLoadResults {
    let mut max_shear = vec![(0.0, f64::NEG_INFINITY); num_sections];
    let mut min_shear = vec![(0.0, f64::INFINITY); num_sections];
    let mut max_moment = vec![(0.0, f64::NEG_INFINITY); num_sections];
    let mut min_moment = vec![(0.0, f64::INFINITY); num_sections];

    let mut critical_shear_pos = 0.0;
    let mut critical_moment_pos = 0.0;
    let mut max_overall_shear = f64::NEG_INFINITY;
    let mut max_overall_moment = f64::NEG_INFINITY;

    // For each load position
    for pos_idx in 0..num_positions {
        let load_position = length * (pos_idx as f64) / ((num_positions - 1) as f64).max(1.0);

        // For each section
        for sec_idx in 0..num_sections {
            let section_x = length * (sec_idx as f64) / ((num_sections - 1) as f64).max(1.0);

            // Compute shear and moment at this section for this load position
            let (shear, moment) = compute_response_at_section(
                length,
                section_x,
                loads,
                load_position,
            );

            let x_val = section_x;
            if shear > max_shear[sec_idx].1 {
                max_shear[sec_idx] = (x_val, shear);
            }
            if shear < min_shear[sec_idx].1 {
                min_shear[sec_idx] = (x_val, shear);
            }
            if moment > max_moment[sec_idx].1 {
                max_moment[sec_idx] = (x_val, moment);
            }
            if moment < min_moment[sec_idx].1 {
                min_moment[sec_idx] = (x_val, moment);
            }

            if shear > max_overall_shear {
                max_overall_shear = shear;
                critical_shear_pos = load_position;
            }
            if moment > max_overall_moment {
                max_overall_moment = moment;
                critical_moment_pos = load_position;
            }
        }
    }

    // Fill in section x coordinates
    for sec_idx in 0..num_sections {
        let section_x = length * (sec_idx as f64) / ((num_sections - 1) as f64).max(1.0);
        max_shear[sec_idx].0 = section_x;
        min_shear[sec_idx].0 = section_x;
        max_moment[sec_idx].0 = section_x;
        min_moment[sec_idx].0 = section_x;
    }

    MovingLoadResults {
        element_id: 0,
        max_shear,
        min_shear,
        max_moment,
        min_moment,
        critical_shear_position: critical_shear_pos,
        critical_moment_position: critical_moment_pos,
    }
}

/// Compute shear and moment at a section for a given load position
fn compute_response_at_section(
    length: f64,
    section_x: f64,
    loads: &[MovingLoad],
    load_position: f64,
) -> (f64, f64) {
    let mut total_shear = 0.0;
    let mut total_moment = 0.0;

    for load in loads {
        match load.load_type {
            MovingLoadType::LaneLoad => {
                // Uniform distributed load
                let intensity = load.axles[0].0;
                // Load covers entire span
                let (shear, moment) = distributed_load_response(length, section_x, intensity, 0.0, length);
                total_shear += shear;
                total_moment += moment;
            }
            _ => {
                // Point loads (truck/tandem)
                for (axle_load, spacing) in &load.axles {
                    let axle_x = load_position + spacing;
                    if axle_x < 0.0 || axle_x > length {
                        continue;
                    }
                    // Shear influence at section for load at axle_x
                    let shear_inf = if axle_x < section_x - 1e-9 {
                        (length - section_x) / length
                    } else {
                        -section_x / length
                    };
                    // Moment influence at section for load at axle_x
                    let moment_inf = if axle_x <= section_x {
                        (length - section_x) * axle_x / length
                    } else {
                        section_x * (length - axle_x) / length
                    };
                    total_shear += axle_load * shear_inf;
                    total_moment += axle_load * moment_inf;
                }
            }
        }
    }

    (total_shear, total_moment)
}

/// Response of a simply supported beam to a uniformly distributed load
fn distributed_load_response(
    length: f64,
    section_x: f64,
    intensity: f64,
    start: f64,
    end: f64,
) -> (f64, f64) {
    if length <= 0.0 {
        return (0.0, 0.0);
    }
    let start = start.clamp(0.0, length);
    let end = end.clamp(0.0, length);
    if end <= start {
        return (0.0, 0.0);
    }
    // Simply supported span: the load resultants and their lever arms give the
    // left reaction, then the section shear and moment follow from statics.
    let load_total = intensity * (end - start);
    let centroid = 0.5 * (start + end);
    let reaction_left = load_total * (length - centroid) / length;
    let loaded_to_section = (section_x.min(end) - start).max(0.0);
    let shear = reaction_left - intensity * loaded_to_section;
    let moment = reaction_left * section_x - 0.5 * intensity * loaded_to_section.powi(2);
    (shear, moment)
}

/// Moving load case combining truck + lane load (AASHTO HL-93)
#[derive(Debug, Clone)]
pub struct HL93Case {
    pub truck: MovingLoad,
    pub tandem: MovingLoad,
    pub lane_load: MovingLoad,
}

impl HL93Case {
    pub fn new(lane_length: f64) -> Self {
        Self {
            truck: MovingLoad::hl93_truck(),
            tandem: MovingLoad::hl93_tandem(),
            lane_load: MovingLoad::lane_load(0.64, lane_length), // 0.64 kips/ft
        }
    }

    /// Compute combined envelope (truck + lane, or tandem + lane, whichever governs)
    pub fn envelope(
        &self,
        length: f64,
        num_sections: usize,
        num_positions: usize,
    ) -> MovingLoadResults {
        // Run truck + lane
        let truck_lane_loads = vec![self.truck.clone(), self.lane_load.clone()];
        let truck_results = moving_load_envelope(length, &truck_lane_loads, num_sections, num_positions);

        // Run tandem + lane
        let tandem_lane_loads = vec![self.tandem.clone(), self.lane_load.clone()];
        let tandem_results = moving_load_envelope(length, &tandem_lane_loads, num_sections, num_positions);

        // Take governing (max absolute) from each
        let mut max_shear = vec![(0.0, f64::NEG_INFINITY); num_sections];
        let mut min_shear = vec![(0.0, f64::INFINITY); num_sections];
        let mut max_moment = vec![(0.0, f64::NEG_INFINITY); num_sections];
        let mut min_moment = vec![(0.0, f64::INFINITY); num_sections];

        for i in 0..num_sections {
            let section_x = length * (i as f64) / ((num_sections - 1) as f64).max(1.0);
            max_shear[i].0 = section_x;
            min_shear[i].0 = section_x;
            max_moment[i].0 = section_x;
            min_moment[i].0 = section_x;

            // Truck + lane
            max_shear[i].1 = max_shear[i].1.max(truck_results.max_shear[i].1);
            min_shear[i].1 = min_shear[i].1.min(truck_results.min_shear[i].1);
            max_moment[i].1 = max_moment[i].1.max(truck_results.max_moment[i].1);
            min_moment[i].1 = min_moment[i].1.min(truck_results.min_moment[i].1);

            // Tandem + lane
            max_shear[i].1 = max_shear[i].1.max(tandem_results.max_shear[i].1);
            min_shear[i].1 = min_shear[i].1.min(tandem_results.min_shear[i].1);
            max_moment[i].1 = max_moment[i].1.max(tandem_results.max_moment[i].1);
            min_moment[i].1 = min_moment[i].1.min(tandem_results.min_moment[i].1);
        }

        MovingLoadResults {
            element_id: 0,
            max_shear,
            min_shear,
            max_moment,
            min_moment,
            critical_shear_position: truck_results.critical_shear_position,
            critical_moment_position: truck_results.critical_moment_position,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_influence_line_simple_beam_shear() {
        let il = influence_line_simple_beam(10.0, 5.0, ResponseType::Shear, 11);
        assert_eq!(il.len(), 11);
        // At x=0, shear influence should be (L-a)/L = 5/10 = 0.5
        assert!((il[0].ordinate - 0.5).abs() < 1e-9);
        // At x=10, shear influence should be -a/L = -5/10 = -0.5
        assert!((il[10].ordinate - (-0.5)).abs() < 1e-9);
    }

    #[test]
    fn test_influence_line_simple_beam_moment() {
        let il = influence_line_simple_beam(10.0, 5.0, ResponseType::Moment, 11);
        assert_eq!(il.len(), 11);
        // At x=5 (section), moment influence should be a*(L-a)/L = 5*5/10 = 2.5
        assert!((il[5].ordinate - 2.5).abs() < 1e-9);
        // At x=0, moment influence should be 0
        assert!(il[0].ordinate.abs() < 1e-9);
        // At x=10, moment influence should be 0
        assert!(il[10].ordinate.abs() < 1e-9);
    }

    #[test]
    fn test_hl93_truck() {
        let truck = MovingLoad::hl93_truck();
        assert_eq!(truck.axles.len(), 3);
        assert!((truck.total_load() - 72.0).abs() < 1e-9);
        assert_eq!(truck.load_type, MovingLoadType::HL93Truck);
    }

    #[test]
    fn test_hl93_tandem() {
        let tandem = MovingLoad::hl93_tandem();
        assert_eq!(tandem.axles.len(), 2);
        assert!((tandem.total_load() - 50.0).abs() < 1e-9);
    }

    #[test]
    fn test_lane_load() {
        let lane = MovingLoad::lane_load(0.64, 100.0);
        assert_eq!(lane.load_type, MovingLoadType::LaneLoad);
        assert!((lane.total_length - 100.0).abs() < 1e-9);
    }

    #[test]
    fn test_moving_load_envelope() {
        let truck = MovingLoad::hl93_truck();
        let results = moving_load_envelope(50.0, &[truck], 11, 51);
        assert_eq!(results.max_shear.len(), 11);
        assert_eq!(results.max_moment.len(), 11);
        // Max moment should be positive
        assert!(results.max_moment.iter().any(|(_, m)| *m > 0.0));
    }

    #[test]
    fn test_hl93_envelope() {
        let hl93 = HL93Case::new(100.0);
        let results = hl93.envelope(50.0, 11, 51);
        assert_eq!(results.max_shear.len(), 11);
        assert_eq!(results.max_moment.len(), 11);
        // Max moment should be positive and larger than truck-only
        assert!(results.max_moment.iter().any(|(_, m)| *m > 0.0));
    }

    #[test]
    fn test_influence_line_points_count() {
        let il = influence_line_simple_beam(20.0, 10.0, ResponseType::Moment, 21);
        assert_eq!(il.len(), 21);
        for pt in &il {
            assert!(pt.x >= 0.0 && pt.x <= 20.0);
        }
    }

    #[test]
    fn test_distributed_load_response() {
        let (shear, moment) = distributed_load_response(10.0, 5.0, 1.0, 0.0, 10.0);
        // For UDL over entire span, shear at midspan should be 0
        assert!(shear.abs() < 1e-9);
        // Moment at midspan should be w*L^2/8 = 1*100/8 = 12.5
        assert!((moment - 12.5).abs() < 1e-9);
    }
}
