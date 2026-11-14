//! Area objects: slabs, walls and decks.
//!
//! Phase 2 treats areas as geometry: they are drawn, selected, saved and
//! displayed in 3D, but the frame solver ignores them until shell elements
//! land in Phase 4.

use crate::model::Model;
use serde::{Deserialize, Serialize};
use std::f64::consts::TAU;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AreaKind {
    Slab,
    Wall,
    Deck,
}

impl AreaKind {
    pub fn label(self) -> &'static str {
        match self {
            AreaKind::Slab => "Slab",
            AreaKind::Wall => "Wall",
            AreaKind::Deck => "Deck",
        }
    }
}

/// Exact plan curve attached to a slab boundary. `node_ids` remain the
/// topological joints used by editing and splitting; this descriptor keeps the
/// original ellipse/arc available for smooth drawing and exact area.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AreaCurve {
    pub center: [f64; 3],
    pub rx: f64,
    pub ry: f64,
    pub start_angle: f64,
    pub sweep: f64,
    pub closed: bool,
    /// parameter angle before this rotation is applied.
    #[serde(default)]
    pub rotation: f64,
}

impl AreaCurve {
    pub fn ellipse(center: [f64; 3], rx: f64, ry: f64) -> Self {
        Self {
            center,
            rx: rx.abs(),
            ry: ry.abs(),
            start_angle: 0.0,
            sweep: TAU,
            closed: true,
            rotation: 0.0,
        }
    }

    pub fn arc(center: [f64; 3], rx: f64, ry: f64, start_angle: f64, sweep: f64) -> Self {
        Self {
            center,
            rx: rx.abs(),
            ry: ry.abs(),
            start_angle,
            sweep,
            closed: false,
            rotation: 0.0,
        }
    }

    pub fn translate(&mut self, delta: [f64; 3]) {
        for axis in 0..3 {
            self.center[axis] += delta[axis];
        }
    }

    /// Rotate the whole boundary about a vertical axis through `pivot`.
    pub fn rotate_z(&mut self, pivot: (f64, f64), radians: f64) {
        let (sin, cos) = radians.sin_cos();
        let dx = self.center[0] - pivot.0;
        let dy = self.center[1] - pivot.1;
        self.center[0] = pivot.0 + dx * cos - dy * sin;
        self.center[1] = pivot.1 + dx * sin + dy * cos;
        self.rotation = (self.rotation + radians).rem_euclid(TAU);
    }

    /// Reflect across the vertical plane X = pivot.
    pub fn mirror_x(&mut self, pivot: f64) {
        self.center[0] = 2.0 * pivot - self.center[0];
        if self.closed {
            self.rotation = (std::f64::consts::PI - self.rotation).rem_euclid(TAU);
        } else {
            self.start_angle = (std::f64::consts::PI - self.start_angle).rem_euclid(TAU);
            self.sweep = -self.sweep;
        }
    }

    /// Reflect across the vertical plane Y = pivot.
    pub fn mirror_y(&mut self, pivot: f64) {
        self.center[1] = 2.0 * pivot - self.center[1];
        if self.closed {
            self.rotation = (-self.rotation).rem_euclid(TAU);
        } else {
            self.start_angle = (-self.start_angle).rem_euclid(TAU);
            self.sweep = -self.sweep;
        }
    }

    /// Smooth boundary samples used for drawing and picking. The implicit chord
    /// of an open arc closes the polygon.
    pub fn points(&self, segments: usize) -> Vec<[f64; 3]> {
        let count = segments.clamp(8, 512);
        let steps = if self.closed { count } else { count + 1 };
        let (sin_rotation, cos_rotation) = self.rotation.sin_cos();
        (0..steps)
            .map(|index| {
                let t = index as f64 / count as f64;
                let angle = self.start_angle + self.sweep * t;
                let local_x = self.rx * angle.cos();
                let local_y = self.ry * angle.sin();
                [
                    self.center[0] + local_x * cos_rotation - local_y * sin_rotation,
                    self.center[1] + local_x * sin_rotation + local_y * cos_rotation,
                    self.center[2],
                ]
            })
            .collect()
    }

    /// Exact area: ellipse, or the area between an elliptic arc and its chord.
    pub fn area(&self) -> f64 {
        if self.closed {
            std::f64::consts::PI * self.rx * self.ry
        } else {
            (0.5 * self.rx * self.ry * (self.sweep - self.sweep.sin())).abs()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Area {
    pub id: usize,
    pub node_ids: Vec<usize>,
    pub kind: AreaKind,
    pub material_id: usize,
    pub thickness: f64,
    #[serde(default)]
    pub curve: Option<AreaCurve>,
}

impl Area {
    pub fn new(node_ids: Vec<usize>, kind: AreaKind, material_id: usize, thickness: f64) -> Self {
        Self {
            id: 0,
            node_ids,
            kind,
            material_id,
            thickness: thickness.max(0.001),
            curve: None,
        }
    }

    pub fn quadrilateral(
        a: usize,
        b: usize,
        c: usize,
        d: usize,
        material_id: usize,
        thickness: f64,
    ) -> Self {
        Self::new(vec![a, b, c, d], AreaKind::Slab, material_id, thickness)
    }

    /// Perimeter coordinates; empty when a referenced joint is missing.
    /// Curved slabs use their preserved ellipse/arc instead of the tessellated joints.
    pub fn polygon(&self, model: &Model) -> Vec<[f64; 3]> {
        if let Some(curve) = &self.curve {
            return curve.points(96);
        }
        let mut points = Vec::with_capacity(self.node_ids.len());
        for id in &self.node_ids {
            let Some(node) = model.nodes.get(id) else {
                return Vec::new();
            };
            points.push([node.x, node.y, node.z]);
        }
        points
    }

    /// Planar area (m²); exact for preserved ellipse and arc boundaries.
    pub fn area(&self, model: &Model) -> f64 {
        if let Some(curve) = &self.curve {
            return curve.area();
        }
        let points = self.polygon(model);
        polygon_area(&points)
    }

    pub fn centroid(&self, model: &Model) -> Option<[f64; 3]> {
        if let Some(curve) = &self.curve {
            if curve.closed {
                return Some(curve.center);
            }
        }
        let points = self.polygon(model);
        if points.len() < 3 {
            return None;
        }
        let mut sum = [0.0; 3];
        for point in &points {
            for axis in 0..3 {
                sum[axis] += point[axis];
            }
        }
        let count = points.len() as f64;
        Some([sum[0] / count, sum[1] / count, sum[2] / count])
    }

    /// True when the area has at least three distinct joints and non-zero area.
    pub fn is_valid(&self, model: &Model) -> bool {
        let mut unique: Vec<[f64; 3]> = Vec::new();
        for point in self.polygon(model) {
            if !unique.iter().any(|other| {
                (other[0] - point[0]).abs() < 1e-9
                    && (other[1] - point[1]).abs() < 1e-9
                    && (other[2] - point[2]).abs() < 1e-9
            }) {
                unique.push(point);
            }
        }
        unique.len() >= 3 && self.area(model) > 1e-9
    }
}

/// Newell's unnormalised areal vector of a closed 3D polygon. Its length is
/// twice the area; its direction is the polygon normal.
fn newell_vector(points: &[[f64; 3]]) -> [f64; 3] {
    let mut normal = [0.0; 3];
    if points.len() < 3 {
        return normal;
    }
    for index in 0..points.len() {
        let current = points[index];
        let next = points[(index + 1) % points.len()];
        normal[0] += (current[1] - next[1]) * (current[2] + next[2]);
        normal[1] += (current[2] - next[2]) * (current[0] + next[0]);
        normal[2] += (current[0] - next[0]) * (current[1] + next[1]);
    }
    normal
}

/// Area of a closed 3D polygon (Newell's method).
pub fn polygon_area(points: &[[f64; 3]]) -> f64 {
    let normal = newell_vector(points);
    0.5 * (normal[0].powi(2) + normal[1].powi(2) + normal[2].powi(2)).sqrt()
}

/// Outward unit normal of a closed 3D polygon; zero for degenerate input.
pub fn polygon_normal(points: &[[f64; 3]]) -> [f64; 3] {
    let normal = newell_vector(points);
    let length = (normal[0].powi(2) + normal[1].powi(2) + normal[2].powi(2)).sqrt();
    if length <= 1e-12 {
        [0.0; 3]
    } else {
        [normal[0] / length, normal[1] / length, normal[2] / length]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quad_model() -> (Model, Area) {
        let mut model = Model::new("area-test");
        let a = model.add_node(0.0, 0.0, 0.0);
        let b = model.add_node(2.0, 0.0, 0.0);
        let c = model.add_node(2.0, 3.0, 0.0);
        let d = model.add_node(0.0, 3.0, 0.0);
        let area = Area::quadrilateral(a, b, c, d, 1, 0.2);
        (model, area)
    }

    #[test]
    fn slab_area_and_centroid() {
        let (model, area) = quad_model();
        assert!((area.area(&model) - 6.0).abs() < 1e-9);
        let centroid = area.centroid(&model).unwrap();
        assert!((centroid[0] - 1.0).abs() < 1e-9);
        assert!((centroid[1] - 1.5).abs() < 1e-9);
        assert!(area.is_valid(&model));
    }

    #[test]
    fn vertical_wall_area_uses_3d_polygon() {
        let mut model = Model::new("wall-test");
        let a = model.add_node(0.0, 0.0, 0.0);
        let b = model.add_node(3.0, 0.0, 0.0);
        let c = model.add_node(3.0, 0.0, 4.0);
        let d = model.add_node(0.0, 0.0, 4.0);
        let wall = Area::new(vec![a, b, c, d], AreaKind::Wall, 1, 0.15);
        assert!((wall.area(&model) - 12.0).abs() < 1e-9);
    }

    #[test]
    fn curved_ellipse_keeps_exact_area_and_smooth_boundary() {
        let model = Model::new("curve");
        let mut area = Area::new(vec![], AreaKind::Slab, 1, 0.2);
        area.curve = Some(AreaCurve::ellipse([1.0, 2.0, 3.0], 2.0, 3.0));
        assert!((area.area(&model) - std::f64::consts::PI * 6.0).abs() < 1e-9);
        assert_eq!(area.polygon(&model).len(), 96);
        assert_eq!(area.centroid(&model).unwrap(), [1.0, 2.0, 3.0]);
    }

    #[test]
    fn curved_arc_uses_the_exact_segment_area() {
        let model = Model::new("arc");
        let mut area = Area::new(vec![], AreaKind::Slab, 1, 0.2);
        area.curve = Some(AreaCurve::arc([0.0, 0.0, 0.0], 2.0, 2.0, 0.0, std::f64::consts::FRAC_PI_2));
        let expected = std::f64::consts::PI - 2.0;
        assert!((area.area(&model) - expected).abs() < 1e-9);
    }

    #[test]
    fn curved_boundary_follows_rigid_edits() {
        let mut curve = AreaCurve::ellipse([0.0, 0.0, 0.0], 2.0, 1.0);
        curve.rotate_z((0.0, 0.0), std::f64::consts::FRAC_PI_2);
        let points = curve.points(16);
        assert!((curve.area() - std::f64::consts::PI * 2.0).abs() < 1e-9);
        // The +X radius now points along +Y.
        assert!(points[0][0].abs() < 1e-9);
        assert!((points[0][1] - 2.0).abs() < 1e-9);

        let mut arc = AreaCurve::arc([0.0, 0.0, 0.0], 2.0, 1.0, 0.0, std::f64::consts::FRAC_PI_2);
        arc.mirror_y(0.0);
        let points = arc.points(8);
        assert!((points[0][0] - 2.0).abs() < 1e-9);
        assert!(points[0][1].abs() < 1e-9);
        assert!(points[points.len() - 1][1] < -0.9);
    }

    #[test]
    fn areas_round_trip_through_json() {
        let (mut model, area) = quad_model();
        let id = model.add_area(area);
        let json = serde_json::to_string(&model).unwrap();
        let mut loaded: Model = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.areas.len(), 1);
        let stored = loaded.areas.get(&id).unwrap();
        assert_eq!(stored.node_ids.len(), 4);
        assert!((stored.area(&loaded) - 6.0).abs() < 1e-9);

        let mut curved = Area::new(vec![], AreaKind::Slab, 1, 0.2);
        curved.curve = Some(AreaCurve::ellipse([0.0, 0.0, 0.0], 2.0, 1.0));
        let curved_id = loaded.add_area(curved);
        let json = serde_json::to_string(&loaded).unwrap();
        let reloaded: Model = serde_json::from_str(&json).unwrap();
        let stored = reloaded.areas.get(&curved_id).unwrap();
        assert!(stored.curve.is_some());
        assert!((stored.area(&reloaded) - std::f64::consts::PI * 2.0).abs() < 1e-9);
    }

    #[test]
    fn degenerate_area_is_invalid() {
        let mut model = Model::new("degenerate");
        let a = model.add_node(0.0, 0.0, 0.0);
        let b = model.add_node(1.0, 0.0, 0.0);
        let c = model.add_node(2.0, 0.0, 0.0);
        let area = Area::new(vec![a, b, c], AreaKind::Slab, 1, 0.2);
        assert!(!area.is_valid(&model));
    }
}
