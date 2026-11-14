use serde::{Deserialize, Serialize};

/// Parametric shape of a frame section. All dimensions are in metres.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SectionShape {
    Rectangular { width: f64, depth: f64 },
    Circular { diameter: f64 },
    ISection { depth: f64, width: f64, web: f64, flange: f64 },
    Channel { depth: f64, width: f64, web: f64, flange: f64 },
    Tee { depth: f64, width: f64, web: f64, flange: f64 },
    Box { depth: f64, width: f64, wall: f64 },
    Pipe { diameter: f64, wall: f64 },
    Angle { depth: f64, width: f64, thickness: f64 },
}

fn positive(values: &[f64]) -> bool {
    values.iter().all(|value| value.is_finite() && *value > 0.0)
}

impl SectionShape {
    /// True when the dimensions describe a solid, non-degenerate shape.
    pub fn is_valid(&self) -> bool {
        match self {
            SectionShape::Rectangular { width, depth } => positive(&[*width, *depth]),
            SectionShape::Circular { diameter } => positive(&[*diameter]),
            SectionShape::ISection { depth, width, web, flange }
            | SectionShape::Channel { depth, width, web, flange } => {
                positive(&[*depth, *width, *web, *flange]) && *web < *width && 2.0 * *flange < *depth
            }
            SectionShape::Tee { depth, width, web, flange } => {
                positive(&[*depth, *width, *web, *flange]) && *web < *width && *flange < *depth
            }
            SectionShape::Box { depth, width, wall } => {
                positive(&[*depth, *width, *wall]) && 2.0 * *wall < width.min(*depth)
            }
            SectionShape::Pipe { diameter, wall } => {
                positive(&[*diameter, *wall]) && 2.0 * *wall < *diameter
            }
            SectionShape::Angle { depth, width, thickness } => {
                positive(&[*depth, *width, *thickness]) && *thickness < width.min(*depth)
            }
        }
    }
}

/// One drawn part of a composite section: a rectangle, a circle, or a closed
/// polygon with optional polygonal holes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DesignerPart {
    Rectangle { x: f64, y: f64, width: f64, depth: f64 },
    Circle { x: f64, y: f64, diameter: f64 },
    Polygon { points: Vec<(f64, f64)>, holes: Vec<Vec<(f64, f64)>> },
}

struct PartProperties {
    area: f64,
    x: f64,
    y: f64,
    min_x: f64,
    max_x: f64,
    min_y: f64,
    max_y: f64,
    iy: f64,
    iz: f64,
    j: f64,
}

struct RingProperties {
    area: f64,
    x: f64,
    y: f64,
    iy: f64,
    iz: f64,
}

/// Closed-polygon area, centroid and centroidal inertias from the shoelace sum.
fn ring_properties(points: &[(f64, f64)]) -> Option<RingProperties> {
    if points.len() < 3 {
        return None;
    }
    let mut area2 = 0.0;
    let mut cx = 0.0;
    let mut cy = 0.0;
    let mut ixx = 0.0;
    let mut iyy = 0.0;
    for index in 0..points.len() {
        let (x0, y0) = points[index];
        let (x1, y1) = points[(index + 1) % points.len()];
        if !x0.is_finite() || !y0.is_finite() || !x1.is_finite() || !y1.is_finite() {
            return None;
        }
        let cross = x0 * y1 - x1 * y0;
        area2 += cross;
        cx += (x0 + x1) * cross;
        cy += (y0 + y1) * cross;
        ixx += (y0 * y0 + y0 * y1 + y1 * y1) * cross;
        iyy += (x0 * x0 + x0 * x1 + x1 * x1) * cross;
    }
    if area2.abs() < 1e-12 {
        return None;
    }
    let sign = area2.signum();
    let area = (area2 / 2.0).abs();
    let x = cx * sign / (6.0 * area);
    let y = cy * sign / (6.0 * area);
    let ixx = ixx * sign / 12.0;
    let iyy = iyy * sign / 12.0;
    Some(RingProperties {
        area,
        x,
        y,
        iy: ixx - area * y * y,
        iz: iyy - area * x * x,
    })
}

/// Even-odd point-in-polygon test.
fn point_in_polygon(point: (f64, f64), polygon: &[(f64, f64)]) -> bool {
    let (x, y) = point;
    let mut inside = false;
    let mut previous = polygon.len().wrapping_sub(1);
    for current in 0..polygon.len() {
        let (xi, yi) = polygon[current];
        let (xj, yj) = polygon[previous];
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        previous = current;
    }
    inside
}

/// Torsion constant of a solid rectangle, with the long side first.
fn rectangle_torsion(width: f64, depth: f64) -> f64 {
    if width <= 0.0 || depth <= 0.0 {
        return 0.0;
    }
    let (long, short) = if width >= depth { (width, depth) } else { (depth, width) };
    let ratio = short / long;
    long * short.powi(3) / 3.0 * (1.0 - 0.63 * ratio + 0.052 * ratio.powi(5))
}

impl DesignerPart {
    pub fn is_valid(&self) -> bool {
        match self {
            DesignerPart::Rectangle { x, y, width, depth } => x.is_finite() && y.is_finite() && positive(&[*width, *depth]),
            DesignerPart::Circle { x, y, diameter } => x.is_finite() && y.is_finite() && positive(&[*diameter]),
            DesignerPart::Polygon { points, holes } => {
                points.len() >= 3
                    && ring_properties(points).is_some()
                    && holes.iter().all(|hole| {
                        hole.len() >= 3
                            && ring_properties(hole).is_some()
                            && hole.iter().all(|vertex| point_in_polygon(*vertex, points))
                    })
            }
        }
    }

    /// True when the point (section-local metres) lies inside the part.
    pub fn contains(&self, x: f64, y: f64) -> bool {
        match self {
            DesignerPart::Rectangle { x: cx, y: cy, width, depth } => {
                (x - cx).abs() <= width / 2.0 && (y - cy).abs() <= depth / 2.0
            }
            DesignerPart::Circle { x: cx, y: cy, diameter } => ((x - cx).powi(2) + (y - cy).powi(2)).sqrt() <= diameter / 2.0,
            DesignerPart::Polygon { points, holes } => {
                point_in_polygon((x, y), points) && !holes.iter().any(|hole| point_in_polygon((x, y), hole))
            }
        }
    }

    fn properties(&self) -> PartProperties {
        match self {
            DesignerPart::Rectangle { x, y, width, depth } => PartProperties {
                area: width * depth,
                x: *x,
                y: *y,
                min_x: x - width / 2.0,
                max_x: x + width / 2.0,
                min_y: y - depth / 2.0,
                max_y: y + depth / 2.0,
                iy: width * depth.powi(3) / 12.0,
                iz: depth * width.powi(3) / 12.0,
                j: rectangle_torsion(*width, *depth),
            },
            DesignerPart::Circle { x, y, diameter } => {
                let r = diameter / 2.0;
                let inertia = std::f64::consts::PI * r.powi(4) / 4.0;
                PartProperties {
                    area: std::f64::consts::PI * r * r,
                    x: *x,
                    y: *y,
                    min_x: x - r,
                    max_x: x + r,
                    min_y: y - r,
                    max_y: y + r,
                    iy: inertia,
                    iz: inertia,
                    j: 2.0 * inertia,
                }
            }
            DesignerPart::Polygon { points, holes } => {
                let outer = match ring_properties(points) {
                    Some(ring) => ring,
                    None => {
                        return PartProperties {
                            area: 0.0,
                            x: 0.0,
                            y: 0.0,
                            min_x: 0.0,
                            max_x: 0.0,
                            min_y: 0.0,
                            max_y: 0.0,
                            iy: 0.0,
                            iz: 0.0,
                            j: 0.0,
                        };
                    }
                };
                let mut area = outer.area;
                let mut moment_x = outer.area * outer.x;
                let mut moment_y = outer.area * outer.y;
                let mut ixx = outer.iy + outer.area * outer.y * outer.y;
                let mut iyy = outer.iz + outer.area * outer.x * outer.x;
                for hole in holes {
                    let Some(ring) = ring_properties(hole) else {
                        continue;
                    };
                    area -= ring.area;
                    moment_x -= ring.area * ring.x;
                    moment_y -= ring.area * ring.y;
                    ixx -= ring.iy + ring.area * ring.y * ring.y;
                    iyy -= ring.iz + ring.area * ring.x * ring.x;
                }
                let (min_x, max_x) = points
                    .iter()
                    .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), point| (lo.min(point.0), hi.max(point.0)));
                let (min_y, max_y) = points
                    .iter()
                    .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), point| (lo.min(point.1), hi.max(point.1)));
                let width = max_x - min_x;
                let depth = max_y - min_y;
                let solidity = if width * depth > 0.0 { (area / (width * depth)).clamp(0.0, 1.0) } else { 0.0 };
                let x = if area > 0.0 { moment_x / area } else { 0.0 };
                let y = if area > 0.0 { moment_y / area } else { 0.0 };
                PartProperties {
                    area,
                    x,
                    y,
                    min_x,
                    max_x,
                    min_y,
                    max_y,
                    iy: ixx - area * y * y,
                    iz: iyy - area * x * x,
                    j: rectangle_torsion(width, depth) * solidity,
                }
            }
        }
    }
}

impl SectionShape {
    /// Equivalent drawn parts, so the Section Designer can start from a
    /// parametric shape and let the user reshape it instead of starting blank.
    pub fn designer_parts(&self) -> Vec<DesignerPart> {
        match *self {
            SectionShape::Rectangular { width, depth } => {
                vec![DesignerPart::Rectangle { x: 0.0, y: 0.0, width, depth }]
            }
            SectionShape::Circular { diameter } => {
                vec![DesignerPart::Circle { x: 0.0, y: 0.0, diameter }]
            }
            SectionShape::ISection { depth, width, web, flange } => {
                let web_depth = depth - 2.0 * flange;
                vec![
                    DesignerPart::Rectangle { x: 0.0, y: depth / 2.0 - flange / 2.0, width, depth: flange },
                    DesignerPart::Rectangle { x: 0.0, y: -(depth / 2.0 - flange / 2.0), width, depth: flange },
                    DesignerPart::Rectangle { x: 0.0, y: 0.0, width: web, depth: web_depth },
                ]
            }
            SectionShape::Channel { depth, width, web, flange } => {
                let web_depth = depth - 2.0 * flange;
                vec![
                    DesignerPart::Rectangle { x: 0.0, y: depth / 2.0 - flange / 2.0, width, depth: flange },
                    DesignerPart::Rectangle { x: 0.0, y: -(depth / 2.0 - flange / 2.0), width, depth: flange },
                    DesignerPart::Rectangle { x: -width / 2.0 + web / 2.0, y: 0.0, width: web, depth: web_depth },
                ]
            }
            SectionShape::Tee { depth, width, web, flange } => {
                let web_depth = depth - flange;
                vec![
                    DesignerPart::Rectangle { x: 0.0, y: depth / 2.0 - flange / 2.0, width, depth: flange },
                    DesignerPart::Rectangle { x: 0.0, y: -depth / 2.0 + web_depth / 2.0, width: web, depth: web_depth },
                ]
            }
            SectionShape::Box { depth, width, wall } => {
                let inner_depth = depth - 2.0 * wall;
                vec![
                    DesignerPart::Rectangle { x: 0.0, y: depth / 2.0 - wall / 2.0, width, depth: wall },
                    DesignerPart::Rectangle { x: 0.0, y: -(depth / 2.0 - wall / 2.0), width, depth: wall },
                    DesignerPart::Rectangle { x: width / 2.0 - wall / 2.0, y: 0.0, width: wall, depth: inner_depth },
                    DesignerPart::Rectangle { x: -(width / 2.0 - wall / 2.0), y: 0.0, width: wall, depth: inner_depth },
                ]
            }
            SectionShape::Pipe { diameter, wall } => {
                let outer = ring_points(diameter / 2.0, 32);
                let inner = ring_points(diameter / 2.0 - wall, 32);
                vec![DesignerPart::Polygon { points: outer, holes: vec![inner] }]
            }
            SectionShape::Angle { depth, width, thickness } => {
                let leg_x = -width / 2.0 + thickness / 2.0;
                let toe_x = -width / 2.0 + thickness + (width - thickness) / 2.0;
                vec![
                    DesignerPart::Rectangle { x: leg_x, y: 0.0, width: thickness, depth },
                    DesignerPart::Rectangle {
                        x: toe_x,
                        y: -depth / 2.0 + thickness / 2.0,
                        width: width - thickness,
                        depth: thickness,
                    },
                ]
            }
        }
    }
}

/// Vertices of a regular polygon ring, used to approximate circular voids.
fn ring_points(radius: f64, segments: usize) -> Vec<(f64, f64)> {
    (0..segments)
        .map(|step| {
            let angle = std::f64::consts::TAU * step as f64 / segments as f64;
            (radius * angle.cos(), radius * angle.sin())
        })
        .collect()
}

/// Stiffness modifiers: scale the properties the solver uses without changing
/// the geometric properties reported for the section.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SectionModifiers {
    pub area: f64,
    pub iy: f64,
    pub iz: f64,
    pub j: f64,
}

impl Default for SectionModifiers {
    fn default() -> Self {
        Self { area: 1.0, iy: 1.0, iz: 1.0, j: 1.0 }
    }
}

impl SectionModifiers {
    pub fn is_default(&self) -> bool {
        self.area == 1.0 && self.iy == 1.0 && self.iz == 1.0 && self.j == 1.0
    }

    /// Modifiers must be finite and non-negative; zero means "no stiffness".
    pub fn is_valid(&self) -> bool {
        [self.area, self.iy, self.iz, self.j].iter().all(|value| value.is_finite() && *value >= 0.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    pub name: String,
    pub area: f64, // Cross-sectional area
    pub iy: f64,   // Strong-axis moment of inertia
    pub iz: f64,   // Weak-axis moment of inertia
    pub j: f64,    // Torsional constant
    pub depth: f64,
    pub width: f64,
    #[serde(default)]
    pub shape: Option<SectionShape>,
    #[serde(default)]
    pub designer: Vec<DesignerPart>,
    #[serde(default)]
    pub modifiers: SectionModifiers,
}

impl Section {
    /// Build a section from a parametric shape, rejecting degenerate dimensions.
    pub fn from_shape(name: &str, shape: SectionShape) -> Option<Self> {
        if !shape.is_valid() {
            return None;
        }
        let (area, iy, iz, j, depth, width) = match shape {
            SectionShape::Rectangular { width: b, depth: h } => {
                let area = b * h;
                let ratio = b / h;
                let j = b * h.powi(3) / 3.0 * (1.0 - 0.63 * ratio + 0.052 * ratio.powi(5));
                (area, b * h.powi(3) / 12.0, h * b.powi(3) / 12.0, j, h, b)
            }
            SectionShape::Circular { diameter } => {
                let r = diameter / 2.0;
                let inertia = std::f64::consts::PI * r.powi(4) / 4.0;
                (std::f64::consts::PI * r * r, inertia, inertia, 2.0 * inertia, diameter, diameter)
            }
            SectionShape::ISection { depth: h, width: b, web: tw, flange: tf } => {
                let hw = h - 2.0 * tf;
                let area = 2.0 * b * tf + tw * hw;
                let iy = (b * h.powi(3) - (b - tw) * hw.powi(3)) / 12.0;
                let iz = (2.0 * tf * b.powi(3) + hw * tw.powi(3)) / 12.0;
                let j = (2.0 * b * tf.powi(3) + hw * tw.powi(3)) / 3.0;
                (area, iy, iz, j, h, b)
            }
            SectionShape::Channel { depth: h, width: b, web: tw, flange: tf } => {
                let hw = h - 2.0 * tf;
                let area = 2.0 * b * tf + tw * hw;
                let iy = (b * h.powi(3) - (b - tw) * hw.powi(3)) / 12.0;
                let flange_area = 2.0 * b * tf;
                let web_area = tw * hw;
                let x_bar = (flange_area * b / 2.0 + web_area * tw / 2.0) / area;
                let iz = 2.0 * (tf * b.powi(3) / 12.0 + b * tf * (b / 2.0 - x_bar).powi(2))
                    + (hw * tw.powi(3) / 12.0 + web_area * (x_bar - tw / 2.0).powi(2));
                let j = (2.0 * b * tf.powi(3) + hw * tw.powi(3)) / 3.0;
                (area, iy, iz, j, h, b)
            }
            SectionShape::Tee { depth: h, width: b, web: tw, flange: tf } => {
                let hw = h - tf;
                let area = b * tf + tw * hw;
                let y_bar = (b * tf * tf / 2.0 + tw * hw * (tf + hw / 2.0)) / area;
                let iy = b * tf.powi(3) / 12.0
                    + b * tf * (y_bar - tf / 2.0).powi(2)
                    + tw * hw.powi(3) / 12.0
                    + tw * hw * (tf + hw / 2.0 - y_bar).powi(2);
                let iz = (tf * b.powi(3) + hw * tw.powi(3)) / 12.0;
                let j = (b * tf.powi(3) + hw * tw.powi(3)) / 3.0;
                (area, iy, iz, j, h, b)
            }
            SectionShape::Box { depth: h, width: b, wall: t } => {
                let (bi, hi) = (b - 2.0 * t, h - 2.0 * t);
                let area = b * h - bi * hi;
                let iy = (b * h.powi(3) - bi * hi.powi(3)) / 12.0;
                let iz = (h * b.powi(3) - hi * bi.powi(3)) / 12.0;
                let (bm, hm) = (b - t, h - t);
                let j = 2.0 * t * bm * bm * hm * hm / (bm + hm);
                (area, iy, iz, j, h, b)
            }
            SectionShape::Pipe { diameter, wall: t } => {
                let inner = diameter - 2.0 * t;
                let area = std::f64::consts::PI / 4.0 * (diameter * diameter - inner * inner);
                let inertia = std::f64::consts::PI / 64.0 * (diameter.powi(4) - inner.powi(4));
                (area, inertia, inertia, 2.0 * inertia, diameter, diameter)
            }
            SectionShape::Angle { depth: h, width: b, thickness: t } => {
                // Two rectangles: the full-depth leg and the remaining width of the other.
                let leg = t * h;
                let toe = (b - t) * t;
                let area = leg + toe;
                let x_bar = (leg * t / 2.0 + toe * (t + (b - t) / 2.0)) / area;
                let y_bar = (leg * h / 2.0 + toe * t / 2.0) / area;
                let iy = t * h.powi(3) / 12.0
                    + leg * (h / 2.0 - y_bar).powi(2)
                    + (b - t) * t.powi(3) / 12.0
                    + toe * (y_bar - t / 2.0).powi(2);
                let iz = h * t.powi(3) / 12.0
                    + leg * (x_bar - t / 2.0).powi(2)
                    + t * (b - t).powi(3) / 12.0
                    + toe * (x_bar - (t + (b - t) / 2.0)).powi(2);
                let j = (2.0 * (b + h) - 2.0 * t) * t.powi(3) / 3.0;
                (area, iy, iz, j, h, b)
            }
        };
        Some(Self {
            name: name.to_string(),
            area,
            iy,
            iz,
            j,
            depth,
            width,
            shape: Some(shape),
            designer: Vec::new(),
            modifiers: SectionModifiers::default(),
        })
    }

    /// Build a composite section from drawn parts, computing the composite area
    /// and centroidal inertias by the parallel-axis theorem.
    ///
    /// The torsion constant is the sum of the part constants (a conservative
    /// approximation for a built-up open section).
    pub fn from_designer(name: &str, parts: &[DesignerPart]) -> Option<Self> {
        if parts.is_empty() || !parts.iter().all(DesignerPart::is_valid) {
            return None;
        }
        let mut area = 0.0;
        let mut moment_x = 0.0;
        let mut moment_y = 0.0;
        let mut j = 0.0;
        let mut min_x = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for part in parts {
            let properties = part.properties();
            area += properties.area;
            moment_x += properties.area * properties.x;
            moment_y += properties.area * properties.y;
            j += properties.j;
            min_x = min_x.min(properties.min_x);
            max_x = max_x.max(properties.max_x);
            min_y = min_y.min(properties.min_y);
            max_y = max_y.max(properties.max_y);
        }
        if area <= 0.0 {
            return None;
        }
        let centroid_x = moment_x / area;
        let centroid_y = moment_y / area;
        let mut iy = 0.0;
        let mut iz = 0.0;
        for part in parts {
            let properties = part.properties();
            iy += properties.iy + properties.area * (properties.y - centroid_y).powi(2);
            iz += properties.iz + properties.area * (properties.x - centroid_x).powi(2);
        }
        let depth = max_y - min_y;
        let width = max_x - min_x;
        if depth <= 0.0 || width <= 0.0 {
            return None;
        }
        Some(Self {
            name: name.to_string(),
            area,
            iy,
            iz,
            j,
            depth,
            width,
            shape: None,
            designer: parts.to_vec(),
            modifiers: SectionModifiers::default(),
        })
    }

    /// Elastic section moduli about the strong (y) and weak (z) axes.
    pub fn section_moduli(&self) -> (f64, f64) {
        let strong = if self.depth > 0.0 { self.iy / (self.depth / 2.0) } else { 0.0 };
        let weak = if self.width > 0.0 { self.iz / (self.width / 2.0) } else { 0.0 };
        (strong, weak)
    }

    pub fn rectangular(name: &str, b: f64, h: f64) -> Self {
        let area = b * h;
        let iy = b * h.powi(3) / 12.0; // about y
        let iz = h * b.powi(3) / 12.0; // about z
        let j = b * h.powi(3) / 3.0 * (1.0 - 0.63 * (b / h) + 0.052 * (b / h).powi(5)); // approximation
        Self {
            name: name.to_string(),
            area,
            iy,
            iz,
            j,
            depth: h,
            width: b,
            shape: Some(SectionShape::Rectangular { width: b, depth: h }),
            designer: Vec::new(),
            modifiers: SectionModifiers::default(),
        }
    }

    pub fn circular(name: &str, diameter: f64) -> Self {
        let r = diameter / 2.0;
        let area = std::f64::consts::PI * r.powi(2);
        let i = std::f64::consts::PI * r.powi(4) / 4.0;
        let j = std::f64::consts::PI * r.powi(4) / 2.0;
        Self {
            name: name.to_string(),
            area,
            iy: i,
            iz: i,
            j,
            depth: diameter,
            width: diameter,
            shape: Some(SectionShape::Circular { diameter }),
            designer: Vec::new(),
            modifiers: SectionModifiers::default(),
        }
    }

    /// A parametric wide-flange (I) section.
    pub fn wide_flange(name: &str, depth: f64, width: f64, web: f64, flange: f64) -> Option<Self> {
        Self::from_shape(name, SectionShape::ISection { depth, width, web, flange })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rectangular_properties_match_the_classic_formulas() {
        let section = Section::rectangular("R", 0.3, 0.5);
        assert!((section.area - 0.15).abs() < 1e-12);
        assert!((section.iy - 0.3 * 0.5_f64.powi(3) / 12.0).abs() < 1e-12);
        assert!((section.iz - 0.5 * 0.3_f64.powi(3) / 12.0).abs() < 1e-12);
        assert!(matches!(section.shape, Some(SectionShape::Rectangular { .. })));
    }

    #[test]
    fn i_section_properties_follow_the_web_and_flange_geometry() {
        let section = Section::from_shape(
            "I",
            SectionShape::ISection { depth: 0.5, width: 0.3, web: 0.01, flange: 0.02 },
        )
        .expect("valid I section");
        // 2 × 0.3 × 0.02 flanges + 0.01 × 0.46 web.
        assert!((section.area - 0.0166).abs() < 1e-12);
        assert!((section.iy - 7.7271333e-4).abs() < 1e-9, "iy={}", section.iy);
        assert!(section.iy > section.iz, "strong axis must exceed the weak axis");
        assert!(section.j > 0.0);
    }

    #[test]
    fn box_and_pipe_properties_are_closed_sections() {
        let box_section = Section::from_shape(
            "B",
            SectionShape::Box { depth: 0.5, width: 0.3, wall: 0.01 },
        )
        .expect("valid box");
        assert!((box_section.area - 0.0156).abs() < 1e-12);
        assert!((box_section.iy - 5.4452e-4).abs() < 1e-9, "iy={}", box_section.iy);
        assert!(box_section.iy > box_section.iz);

        let pipe = Section::from_shape("P", SectionShape::Pipe { diameter: 0.2, wall: 0.01 })
            .expect("valid pipe");
        let expected_area = std::f64::consts::PI / 4.0 * (0.2_f64.powi(2) - 0.18_f64.powi(2));
        assert!((pipe.area - expected_area).abs() < 1e-12);
        assert!((pipe.j - 2.0 * pipe.iy).abs() < 1e-12);
        assert!((pipe.iy - pipe.iz).abs() < 1e-15);
    }

    #[test]
    fn angle_with_equal_legs_has_equal_principal_moments() {
        let angle = Section::from_shape(
            "L",
            SectionShape::Angle { depth: 0.1, width: 0.1, thickness: 0.01 },
        )
        .expect("valid angle");
        assert!((angle.area - 0.0019).abs() < 1e-12);
        assert!((angle.iy - angle.iz).abs() < 1e-12, "iy={} iz={}", angle.iy, angle.iz);
    }

    #[test]
    fn channel_and_tee_are_asymmetric_about_the_weak_axis() {
        let channel = Section::from_shape(
            "C",
            SectionShape::Channel { depth: 0.3, width: 0.1, web: 0.01, flange: 0.015 },
        )
        .expect("valid channel");
        assert!(channel.iy > channel.iz);
        let tee = Section::from_shape(
            "T",
            SectionShape::Tee { depth: 0.3, width: 0.2, web: 0.01, flange: 0.02 },
        )
        .expect("valid tee");
        assert!((tee.area - (0.2 * 0.02 + 0.01 * 0.28)).abs() < 1e-12);
        assert!(tee.iy > tee.iz);
    }

    #[test]
    fn drawn_i_shape_matches_the_parametric_i_section() {
        let parts = vec![
            DesignerPart::Rectangle { x: 0.0, y: -0.24, width: 0.3, depth: 0.02 },
            DesignerPart::Rectangle { x: 0.0, y: 0.24, width: 0.3, depth: 0.02 },
            DesignerPart::Rectangle { x: 0.0, y: 0.0, width: 0.01, depth: 0.46 },
        ];
        let drawn = Section::from_designer("DRAWN", &parts).expect("valid composite");
        let parametric = Section::from_shape(
            "I",
            SectionShape::ISection { depth: 0.5, width: 0.3, web: 0.01, flange: 0.02 },
        )
        .expect("valid I section");
        assert!((drawn.area - parametric.area).abs() < 1e-12);
        assert!((drawn.iy - parametric.iy).abs() < 1e-12, "iy {} vs {}", drawn.iy, parametric.iy);
        assert!((drawn.iz - parametric.iz).abs() < 1e-12, "iz {} vs {}", drawn.iz, parametric.iz);
        assert!((drawn.depth - 0.5).abs() < 1e-12);
        assert!((drawn.width - 0.3).abs() < 1e-12);
        assert_eq!(drawn.designer.len(), 3);
        assert!(drawn.shape.is_none());
    }

    #[test]
    fn drawn_centroid_shifts_toward_the_larger_part() {
        let parts = vec![
            DesignerPart::Rectangle { x: 0.0, y: 0.0, width: 0.2, depth: 0.2 },
            DesignerPart::Rectangle { x: 0.5, y: 0.0, width: 0.1, depth: 0.1 },
        ];
        let section = Section::from_designer("TWO", &parts).expect("valid composite");
        assert!((section.area - 0.05).abs() < 1e-12);
        // Bounds span −0.1 … 0.55, so the outline is 0.65 m wide.
        assert!((section.width - 0.65).abs() < 1e-12, "width {}", section.width);
        // The centroid lies at x = 0.01 × 0.5 / 0.05 = 0.1, so the weak-axis
        // inertia is larger than a single 0.2 × 0.2 square.
        assert!(section.iz > 0.2 * 0.2_f64.powi(3) / 12.0);
    }

    #[test]
    fn modifiers_default_to_unity_and_round_trip_through_json() {
        let section = Section::rectangular("R", 0.3, 0.5);
        assert!(section.modifiers.is_default());
        assert!(section.modifiers.is_valid());
        let mut modified = Section::rectangular("M", 0.3, 0.5);
        modified.modifiers.iz = 0.5;
        let json = serde_json::to_string(&modified).expect("json");
        let loaded: Section = serde_json::from_str(&json).expect("load");
        assert_eq!(loaded.modifiers.iz, 0.5);
        assert_eq!(loaded.modifiers.area, 1.0);
        // Legacy files without the field load with unit modifiers.
        let legacy = "{\"name\":\"L\",\"area\":1.0,\"iy\":1.0,\"iz\":1.0,\"j\":1.0,\"depth\":1.0,\"width\":1.0}";
        let parsed: Section = serde_json::from_str(legacy).expect("legacy");
        assert!(parsed.modifiers.is_default());
    }

    #[test]
    fn designer_parts_reproduce_the_parametric_shapes() {
        let shapes = [
            SectionShape::Rectangular { width: 0.3, depth: 0.5 },
            SectionShape::Circular { diameter: 0.2 },
            SectionShape::ISection { depth: 0.5, width: 0.3, web: 0.01, flange: 0.02 },
            SectionShape::Channel { depth: 0.4, width: 0.15, web: 0.012, flange: 0.018 },
            SectionShape::Tee { depth: 0.3, width: 0.2, web: 0.01, flange: 0.02 },
            SectionShape::Box { depth: 0.5, width: 0.3, wall: 0.01 },
            SectionShape::Angle { depth: 0.1, width: 0.1, thickness: 0.01 },
        ];
        for shape in shapes {
            let parametric = Section::from_shape("P", shape).expect("valid shape");
            let drawn = Section::from_designer("P", &shape.designer_parts()).expect("valid drawn parts");
            assert!((drawn.area - parametric.area).abs() < 1e-12, "{:?} area {} vs {}", shape, drawn.area, parametric.area);
            assert!((drawn.iy - parametric.iy).abs() < 1e-12, "{:?} iy {} vs {}", shape, drawn.iy, parametric.iy);
            assert!((drawn.iz - parametric.iz).abs() < 1e-12, "{:?} iz {} vs {}", shape, drawn.iz, parametric.iz);
            assert!((drawn.width - parametric.width).abs() < 1e-12, "{:?} width {} vs {}", shape, drawn.width, parametric.width);
            assert!((drawn.depth - parametric.depth).abs() < 1e-12, "{:?} depth {} vs {}", shape, drawn.depth, parametric.depth);
        }
    }

    #[test]
    fn pipe_seeds_a_polygon_with_a_void() {
        let shape = SectionShape::Pipe { diameter: 0.2, wall: 0.01 };
        let parts = shape.designer_parts();
        assert!(matches!(parts[0], DesignerPart::Polygon { .. }));
        let drawn = Section::from_designer("PIPE", &parts).expect("valid pipe parts");
        let parametric = Section::from_shape("PIPE", shape).expect("valid pipe");
        // Two 32-segment rings approximate the pipe to well under a percent.
        assert!((drawn.area - parametric.area).abs() / parametric.area < 1e-2, "area {} vs {}", drawn.area, parametric.area);
    }

    #[test]
    fn polygon_with_hole_subtracts_the_void() {
        let outer = vec![(-0.2, -0.2), (0.2, -0.2), (0.2, 0.2), (-0.2, 0.2)];
        let hole = vec![(-0.1, -0.1), (0.1, -0.1), (0.1, 0.1), (-0.1, 0.1)];
        let parts = vec![DesignerPart::Polygon { points: outer, holes: vec![hole] }];
        let section = Section::from_designer("VOID", &parts).expect("valid voided polygon");
        assert!((section.area - 0.12).abs() < 1e-12, "area {}", section.area);
        // A hollow 0.4 × 0.4 square with a 0.2 × 0.2 void: (0.4⁴ − 0.2⁴)/12 each way.
        assert!((section.iy - 2.0e-3).abs() < 1e-12, "iy {}", section.iy);
        assert!((section.iz - 2.0e-3).abs() < 1e-12, "iz {}", section.iz);
        assert!((section.depth - 0.4).abs() < 1e-12);
        assert!((section.width - 0.4).abs() < 1e-12);
        let part = &section.designer[0];
        assert!(part.contains(0.15, 0.0));
        assert!(!part.contains(0.0, 0.0));
    }

    #[test]
    fn holes_outside_the_outline_are_rejected() {
        let outer = vec![(0.0, 0.0), (0.2, 0.0), (0.2, 0.2), (0.0, 0.2)];
        let hole = vec![(0.4, 0.4), (0.5, 0.4), (0.5, 0.5), (0.4, 0.5)];
        let parts = vec![DesignerPart::Polygon { points: outer, holes: vec![hole] }];
        assert!(Section::from_designer("BAD", &parts).is_none());
        assert!(!DesignerPart::Polygon { points: vec![], holes: vec![] }.is_valid());
    }

    #[test]
    fn section_moduli_use_the_extreme_fibres() {
        let section = Section::rectangular("R", 0.3, 0.5);
        let (sy, sz) = section.section_moduli();
        assert!((sy - section.iy / 0.25).abs() < 1e-12);
        assert!((sz - section.iz / 0.15).abs() < 1e-12);
        let drawn = Section::from_designer(
            "C",
            &[DesignerPart::Circle { x: 0.0, y: 0.0, diameter: 0.2 }],
        )
        .expect("valid circle");
        assert!(drawn.section_moduli().0 > 0.0);
    }

    #[test]
    fn empty_or_degenerate_designs_are_rejected() {
        assert!(Section::from_designer("NONE", &[]).is_none());
        assert!(Section::from_designer("FLAT", &[DesignerPart::Rectangle { x: 0.0, y: 0.0, width: 0.0, depth: 0.2 }]).is_none());
        assert!(Section::from_designer("HOLE", &[DesignerPart::Circle { x: 0.0, y: 0.0, diameter: 0.0 }]).is_none());
    }

    #[test]
    fn degenerate_shapes_are_rejected() {
        assert!(Section::from_shape("B", SectionShape::Box { depth: 0.2, width: 0.2, wall: 0.1 }).is_none());
        assert!(Section::from_shape("I", SectionShape::ISection { depth: 0.3, width: 0.2, web: 0.25, flange: 0.01 }).is_none());
        assert!(Section::from_shape("P", SectionShape::Pipe { diameter: 0.2, wall: 0.1 }).is_none());
        assert!(Section::from_shape("A", SectionShape::Angle { depth: 0.1, width: 0.1, thickness: 0.1 }).is_none());
        assert!(Section::from_shape("Z", SectionShape::Rectangular { width: 0.0, depth: 0.5 }).is_none());
    }
}
