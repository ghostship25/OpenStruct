//! Geometry cleanup: split crossing members so they share joints.
//!
//! Frames are split at any joint lying on their span, crossing frames create a
//! shared joint, horizontal slabs are divided along same-level beams, and wall
//! edges receive joints wherever a frame crosses them. The frame solver does not
//! use slabs yet, but the shared geometry is what Phase 4 shell assembly needs.

use crate::areas::{Area, AreaCurve, AreaKind};
use crate::model::Model;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SplitReport {
    pub joints_added: usize,
    pub frames_split: usize,
    pub slabs_split: usize,
    pub areas_updated: usize,
}

impl SplitReport {
    pub fn is_empty(&self) -> bool {
        self.joints_added == 0
            && self.frames_split == 0
            && self.slabs_split == 0
            && self.areas_updated == 0
    }
}

/// Split frames and slabs at their geometric intersections.
pub fn split_at_intersections(model: &mut Model, tolerance: f64) -> SplitReport {
    let tol = tolerance.max(1e-9);
    let mut report = SplitReport::default();
    let frames: Vec<([f64; 3], [f64; 3])> = model
        .elements
        .values()
        .filter_map(|element| {
            let a = model.nodes.get(&element.node_i)?;
            let b = model.nodes.get(&element.node_j)?;
            Some(([a.x, a.y, a.z], [b.x, b.y, b.z]))
        })
        .collect();

    // 1. Crossing frames share a joint.
    for i in 0..frames.len() {
        for j in i + 1..frames.len() {
            if let Some(point) =
                segment_intersection(frames[i].0, frames[i].1, frames[j].0, frames[j].1, tol)
            {
                find_or_add_joint(model, point, tol, &mut report);
            }
        }
    }

    // 2. Same-level beams divide horizontal slabs.
    report.slabs_split += split_slabs(model, &frames, tol, &mut report);

    // 3. Frames crossing a wall edge create a joint on that edge.
    let wall_edges: Vec<([f64; 3], [f64; 3])> = model
        .areas
        .values()
        .filter(|area| area.kind == AreaKind::Wall)
        .flat_map(|area| {
            let points: Vec<[f64; 3]> = area
                .node_ids
                .iter()
                .filter_map(|id| model.nodes.get(id).map(|node| [node.x, node.y, node.z]))
                .collect();
            let mut edges = Vec::new();
            if points.len() >= 2 {
                for index in 0..points.len() {
                    edges.push((points[index], points[(index + 1) % points.len()]));
                }
            }
            edges
        })
        .collect();
    for (a, b) in &wall_edges {
        for (c, d) in &frames {
            if let Some(point) = segment_intersection(*a, *b, *c, *d, tol) {
                find_or_add_joint(model, point, tol, &mut report);
            }
        }
    }

    // 4. Existing joints on area perimeters become perimeter vertices.
    report.areas_updated += insert_area_edge_joints(model, tol);

    // 5. Every frame is divided at joints lying on its span.
    report.frames_split += split_frames_at_joints(model, tol);
    report
}

fn split_slabs(
    model: &mut Model,
    frames: &[([f64; 3], [f64; 3])],
    tol: f64,
    report: &mut SplitReport,
) -> usize {
    let ids: Vec<usize> = model
        .areas
        .iter()
        .filter(|(_, area)| area.kind != AreaKind::Wall)
        .map(|(&id, _)| id)
        .collect();
    let mut split_count = 0;
    for id in ids {
        let Some(area) = model.areas.get(&id).cloned() else {
            continue;
        };
        if area.curve.is_some() {
            if split_curved_slab(model, id, &area, frames, tol, report) {
                split_count += 1;
            }
            continue;
        }
        let points: Option<Vec<[f64; 3]>> = area
            .node_ids
            .iter()
            .map(|node_id| {
                model
                    .nodes
                    .get(node_id)
                    .map(|node| [node.x, node.y, node.z])
            })
            .collect();
        let Some(points) = points else {
            continue;
        };
        if points.len() < 3 {
            continue;
        }
        let level = points[0][2];
        if points.iter().any(|point| (point[2] - level).abs() > tol) {
            continue;
        }
        let mut pieces: Vec<Vec<(f64, f64)>> =
            vec![points.iter().map(|point| (point[0], point[1])).collect()];
        for (a, b) in frames {
            if (a[2] - level).abs() > tol || (b[2] - level).abs() > tol {
                continue;
            }
            let (a2, b2) = ((a[0], a[1]), (b[0], b[1]));
            if distance_2d(a2, b2) < tol {
                continue;
            }
            let mut next = Vec::new();
            for piece in pieces {
                if !segment_crosses_polygon(a2, b2, &piece, tol) {
                    next.push(piece);
                    continue;
                }
                match split_polygon_by_line(&piece, a2, b2, tol) {
                    Some((left, right)) => {
                        next.push(left);
                        next.push(right);
                    }
                    None => next.push(piece),
                }
            }
            pieces = next;
        }
        if pieces.len() <= 1 {
            continue;
        }

        let mut piece_ids: Vec<Vec<usize>> = Vec::new();
        for piece in &pieces {
            let mut ids = Vec::with_capacity(piece.len());
            for (x, y) in piece {
                let node = find_or_add_joint(model, [*x, *y, level], tol, report);
                if !ids.contains(&node) {
                    ids.push(node);
                }
            }
            if ids.len() >= 3 {
                piece_ids.push(ids);
            }
        }
        if piece_ids.len() <= 1 {
            continue;
        }
        model.areas.remove(&id);
        for piece in piece_ids {
            let mut area = area.clone();
            area.node_ids = piece;
            model.add_area(area);
        }
        split_count += 1;
    }
    split_count
}

/// Cut a closed elliptic slab with a same-level straight member. The cut is
/// solved on the true ellipse and produces two arc-plus-chord pieces, each with
/// exact geometry and exact area. Open arc pieces are left untouched because a
/// second cut can require more than one arc on one boundary.
fn split_curved_slab(
    model: &mut Model,
    id: usize,
    area: &Area,
    frames: &[([f64; 3], [f64; 3])],
    tol: f64,
    report: &mut SplitReport,
) -> bool {
    let Some(curve) = area.curve.as_ref() else {
        return false;
    };
    if !curve.closed || curve.rx <= tol || curve.ry <= tol {
        return false;
    }
    let level = curve.center[2];
    for (a, b) in frames {
        if (a[2] - level).abs() > tol || (b[2] - level).abs() > tol {
            continue;
        }
        let a2 = (a[0], a[1]);
        let b2 = (b[0], b[1]);
        if distance_2d(a2, b2) < tol {
            continue;
        }
        let Some((p1, p2)) = line_ellipse_intersections(curve, a2, b2, tol) else {
            continue;
        };
        // The infinite line always meets a closed ellipse twice; the frame must
        // actually span both crossings. This also handles cuts through a
        // tessellated vertex, where a strict polygon-edge test reports no hit.
        if !point_on_segment_2d(p1, a2, b2, tol) || !point_on_segment_2d(p2, a2, b2, tol) {
            continue;
        }
        let n1 = find_or_add_joint(model, [p1.0, p1.1, level], tol, report);
        let n2 = find_or_add_joint(model, [p2.0, p2.1, level], tol, report);
        if n1 == n2 {
            continue;
        }
        let angle1 = curve_parameter(curve, p1);
        let angle2 = curve_parameter(curve, p2);
        let sweep = (angle2 - angle1).rem_euclid(std::f64::consts::TAU);
        if sweep <= 1e-6 || std::f64::consts::TAU - sweep <= 1e-6 {
            continue;
        }
        model.areas.remove(&id);
        let mut first = area.clone();
        first.node_ids = vec![n1, n2];
        first.curve = Some(AreaCurve {
            center: curve.center,
            rx: curve.rx,
            ry: curve.ry,
            start_angle: angle1,
            sweep,
            closed: false,
            rotation: curve.rotation,
        });
        let mut second = area.clone();
        second.node_ids = vec![n2, n1];
        second.curve = Some(AreaCurve {
            center: curve.center,
            rx: curve.rx,
            ry: curve.ry,
            start_angle: angle2,
            sweep: std::f64::consts::TAU - sweep,
            closed: false,
            rotation: curve.rotation,
        });
        model.add_area(first);
        model.add_area(second);
        return true;
    }
    false
}

/// Intersect an infinite line with an ellipse, returning both crossing points.
fn line_ellipse_intersections(
    curve: &AreaCurve,
    a: (f64, f64),
    b: (f64, f64),
    tol: f64,
) -> Option<((f64, f64), (f64, f64))> {
    let (sin_rotation, cos_rotation) = curve.rotation.sin_cos();
    let local = |point: (f64, f64)| {
        let dx = point.0 - curve.center[0];
        let dy = point.1 - curve.center[1];
        (dx * cos_rotation + dy * sin_rotation, -dx * sin_rotation + dy * cos_rotation)
    };
    let p = local(a);
    let q = local(b);
    let d = (q.0 - p.0, q.1 - p.1);
    let rx = curve.rx;
    let ry = curve.ry;
    let aa = d.0 * d.0 / (rx * rx) + d.1 * d.1 / (ry * ry);
    let bb = 2.0 * (p.0 * d.0 / (rx * rx) + p.1 * d.1 / (ry * ry));
    let cc = p.0 * p.0 / (rx * rx) + p.1 * p.1 / (ry * ry) - 1.0;
    if aa <= tol * tol {
        return None;
    }
    let discriminant = bb * bb - 4.0 * aa * cc;
    if discriminant <= tol * tol {
        return None;
    }
    let root = discriminant.sqrt();
    let t1 = (-bb - root) / (2.0 * aa);
    let t2 = (-bb + root) / (2.0 * aa);
    if (t1 - t2).abs() <= tol {
        return None;
    }
    let world = |t: f64| {
        let local_x = p.0 + d.0 * t;
        let local_y = p.1 + d.1 * t;
        (
            curve.center[0] + local_x * cos_rotation - local_y * sin_rotation,
            curve.center[1] + local_x * sin_rotation + local_y * cos_rotation,
        )
    };
    Some((world(t1), world(t2)))
}

fn curve_parameter(curve: &AreaCurve, point: (f64, f64)) -> f64 {
    let (sin_rotation, cos_rotation) = curve.rotation.sin_cos();
    let dx = point.0 - curve.center[0];
    let dy = point.1 - curve.center[1];
    let local_x = dx * cos_rotation + dy * sin_rotation;
    let local_y = -dx * sin_rotation + dy * cos_rotation;
    (local_y / curve.ry).atan2(local_x / curve.rx)
}

fn insert_area_edge_joints(model: &mut Model, tol: f64) -> usize {
    let ids: Vec<usize> = model.areas.keys().copied().collect();
    let mut updated = 0;
    for id in ids {
        let Some(area) = model.areas.get(&id).cloned() else {
            continue;
        };
        // Curved boundaries keep their true edge; the chord between two arc
        // endpoints is not a physical edge to insert joints into.
        if area.curve.is_some() {
            continue;
        }
        let count = area.node_ids.len();
        if count < 3 {
            continue;
        }
        let mut expanded: Vec<usize> = Vec::with_capacity(count);
        let mut added = 0;
        for index in 0..count {
            let a_id = area.node_ids[index];
            let b_id = area.node_ids[(index + 1) % count];
            expanded.push(a_id);
            let (Some(a), Some(b)) = (model.nodes.get(&a_id), model.nodes.get(&b_id)) else {
                continue;
            };
            let pa = [a.x, a.y, a.z];
            let pb = [b.x, b.y, b.z];
            let mut candidates: Vec<(f64, usize)> = Vec::new();
            for (&node_id, node) in &model.nodes {
                if node_id == a_id || node_id == b_id {
                    continue;
                }
                if let Some(t) = point_on_segment([node.x, node.y, node.z], pa, pb, tol) {
                    candidates.push((t, node_id));
                }
            }
            candidates.sort_by(|left, right| {
                left.0
                    .partial_cmp(&right.0)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            for (_, node_id) in candidates {
                if !expanded.contains(&node_id) {
                    expanded.push(node_id);
                    added += 1;
                }
            }
        }
        if added > 0 {
            let mut area = area;
            area.node_ids = expanded;
            model.areas.insert(id, area);
            updated += 1;
        }
    }
    updated
}

fn split_frames_at_joints(model: &mut Model, tol: f64) -> usize {
    let ids: Vec<usize> = model.elements.keys().copied().collect();
    let mut split = 0;
    for id in ids {
        let Some(element) = model.elements.get(&id).cloned() else {
            continue;
        };
        let (Some(a), Some(b)) = (
            model.nodes.get(&element.node_i),
            model.nodes.get(&element.node_j),
        ) else {
            continue;
        };
        let pa = [a.x, a.y, a.z];
        let pb = [b.x, b.y, b.z];
        let mut cuts: Vec<(f64, usize)> = Vec::new();
        for (&node_id, node) in &model.nodes {
            if node_id == element.node_i || node_id == element.node_j {
                continue;
            }
            if let Some(t) = point_on_segment([node.x, node.y, node.z], pa, pb, tol) {
                cuts.push((t, node_id));
            }
        }
        if cuts.is_empty() {
            continue;
        }
        cuts.sort_by(|left, right| {
            left.0
                .partial_cmp(&right.0)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        cuts.dedup_by_key(|(_, node_id)| *node_id);
        model.elements.remove(&id);
        let mut chain = vec![element.node_i];
        chain.extend(cuts.into_iter().map(|(_, node_id)| node_id));
        chain.push(element.node_j);
        let mut added = 0;
        for pair in chain.windows(2) {
            if pair[0] == pair[1] {
                continue;
            }
            let mut segment = element.clone();
            segment.node_i = pair[0];
            segment.node_j = pair[1];
            let new_id = model.add_element(segment);
            if let Some(stored) = model.elements.get_mut(&new_id) {
                stored.id = new_id;
            }
            added += 1;
        }
        if added > 1 {
            split += 1;
        }
    }
    split
}

fn find_or_add_joint(
    model: &mut Model,
    point: [f64; 3],
    tol: f64,
    report: &mut SplitReport,
) -> usize {
    let tol_sq = tol * tol;
    for (&id, node) in &model.nodes {
        let dx = node.x - point[0];
        let dy = node.y - point[1];
        let dz = node.z - point[2];
        if dx * dx + dy * dy + dz * dz <= tol_sq {
            return id;
        }
    }
    report.joints_added += 1;
    model.add_node(point[0], point[1], point[2])
}

/// Closest-approach intersection of two 3D segments, strictly inside both.
fn segment_intersection(
    p1: [f64; 3],
    p2: [f64; 3],
    q1: [f64; 3],
    q2: [f64; 3],
    tol: f64,
) -> Option<[f64; 3]> {
    let u = sub(p2, p1);
    let v = sub(q2, q1);
    let w = sub(p1, q1);
    let a = dot(u, u);
    let b = dot(u, v);
    let c = dot(v, v);
    let d = dot(u, w);
    let e = dot(v, w);
    let denom = a * c - b * b;
    if denom.abs() <= 1e-12 * a.max(1.0) * c.max(1.0) {
        return None;
    }
    let s = (b * e - c * d) / denom;
    let t = (a * e - b * d) / denom;
    let margin = 1e-6;
    if s <= margin || s >= 1.0 - margin || t <= margin || t >= 1.0 - margin {
        return None;
    }
    let point_p = add(p1, scale(u, s));
    let point_q = add(q1, scale(v, t));
    if distance(point_p, point_q) > tol {
        return None;
    }
    Some(scale(add(point_p, point_q), 0.5))
}

fn point_on_segment(point: [f64; 3], a: [f64; 3], b: [f64; 3], tol: f64) -> Option<f64> {
    let ab = sub(b, a);
    let length_sq = dot(ab, ab);
    if length_sq <= tol * tol {
        return None;
    }
    let ap = sub(point, a);
    let t = dot(ap, ab) / length_sq;
    if t <= 1e-6 || t >= 1.0 - 1e-6 {
        return None;
    }
    let closest = add(a, scale(ab, t));
    if distance(closest, point) <= tol {
        Some(t)
    } else {
        None
    }
}

fn segment_crosses_polygon(a: (f64, f64), b: (f64, f64), polygon: &[(f64, f64)], tol: f64) -> bool {
    if polygon.len() < 3 {
        return false;
    }
    if point_in_polygon(a, polygon) || point_in_polygon(b, polygon) {
        return true;
    }
    for index in 0..polygon.len() {
        let c = polygon[index];
        let d = polygon[(index + 1) % polygon.len()];
        if segments_cross_2d(a, b, c, d, tol) {
            return true;
        }
    }
    false
}

fn split_polygon_by_line(
    points: &[(f64, f64)],
    a: (f64, f64),
    b: (f64, f64),
    tol: f64,
) -> Option<(Vec<(f64, f64)>, Vec<(f64, f64)>)> {
    let left = clip_half_plane(points, a, b, true, tol);
    let right = clip_half_plane(points, a, b, false, tol);
    if left.len() < 3 || right.len() < 3 {
        return None;
    }
    let area_left = crate::shapes::signed_area(&left).abs();
    let area_right = crate::shapes::signed_area(&right).abs();
    if area_left <= tol * tol || area_right <= tol * tol {
        return None;
    }
    Some((left, right))
}

fn clip_half_plane(
    points: &[(f64, f64)],
    a: (f64, f64),
    b: (f64, f64),
    keep_positive: bool,
    tol: f64,
) -> Vec<(f64, f64)> {
    let side = |point: (f64, f64)| (b.0 - a.0) * (point.1 - a.1) - (b.1 - a.1) * (point.0 - a.0);
    let inside = |value: f64| {
        if keep_positive {
            value >= -tol
        } else {
            value <= tol
        }
    };
    let mut out: Vec<(f64, f64)> = Vec::new();
    for index in 0..points.len() {
        let p = points[index];
        let q = points[(index + 1) % points.len()];
        let sp = side(p);
        let sq = side(q);
        let p_in = inside(sp);
        let q_in = inside(sq);
        if p_in {
            out.push(p);
        }
        if p_in != q_in {
            let denominator = sp - sq;
            if denominator.abs() > 1e-15 {
                let t = sp / denominator;
                out.push((p.0 + (q.0 - p.0) * t, p.1 + (q.1 - p.1) * t));
            }
        }
    }
    dedupe_ring(out, tol)
}

fn dedupe_ring(points: Vec<(f64, f64)>, tol: f64) -> Vec<(f64, f64)> {
    let mut out: Vec<(f64, f64)> = Vec::with_capacity(points.len());
    for point in points {
        if out
            .last()
            .is_none_or(|last| distance_2d(*last, point) > tol)
        {
            out.push(point);
        }
    }
    if out.len() >= 2 && distance_2d(out[0], out[out.len() - 1]) <= tol {
        out.pop();
    }
    out
}

fn point_in_polygon(point: (f64, f64), polygon: &[(f64, f64)]) -> bool {
    let mut inside = false;
    let mut j = polygon.len() - 1;
    for i in 0..polygon.len() {
        let a = polygon[i];
        let b = polygon[j];
        if ((a.1 > point.1) != (b.1 > point.1))
            && (point.0 < (b.0 - a.0) * (point.1 - a.1) / (b.1 - a.1) + a.0)
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn segments_cross_2d(a: (f64, f64), b: (f64, f64), c: (f64, f64), d: (f64, f64), tol: f64) -> bool {
    let cross = |p: (f64, f64), q: (f64, f64), r: (f64, f64)| {
        (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0)
    };
    let o1 = cross(a, b, c);
    let o2 = cross(a, b, d);
    let o3 = cross(c, d, a);
    let o4 = cross(c, d, b);
    ((o1 > tol && o2 < -tol) || (o1 < -tol && o2 > tol))
        && ((o3 > tol && o4 < -tol) || (o3 < -tol && o4 > tol))
}

fn distance_2d(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

fn point_on_segment_2d(point: (f64, f64), a: (f64, f64), b: (f64, f64), tol: f64) -> bool {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let length_sq = dx * dx + dy * dy;
    if length_sq <= tol * tol {
        return false;
    }
    let t = ((point.0 - a.0) * dx + (point.1 - a.1) * dy) / length_sq;
    if t < -tol || t > 1.0 + tol {
        return false;
    }
    let closest = (a.0 + dx * t, a.1 + dy * t);
    distance_2d(point, closest) <= tol
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scale(a: [f64; 3], factor: f64) -> [f64; 3] {
    [a[0] * factor, a[1] * factor, a[2] * factor]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    dot(sub(a, b), sub(a, b)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::areas::Area;
    use crate::elements::Element;
    use crate::shapes;

    #[test]
    fn crossing_frames_split_and_share_a_joint() {
        let mut model = Model::new("crossing-frames");
        let n1 = model.add_node(0.0, 2.0, 0.0);
        let n2 = model.add_node(4.0, 2.0, 0.0);
        let n3 = model.add_node(2.0, 0.0, 0.0);
        let n4 = model.add_node(2.0, 4.0, 0.0);
        model.add_element(Element::frame(1, n1, n2, 1, 1));
        model.add_element(Element::frame(2, n3, n4, 1, 1));

        let report = split_at_intersections(&mut model, 0.001);
        assert_eq!(report.joints_added, 1);
        assert_eq!(report.frames_split, 2);
        assert_eq!(model.elements.len(), 4);
        assert_eq!(model.nodes.len(), 5);
        assert!(model
            .nodes
            .values()
            .any(|node| (node.x - 2.0).abs() < 1e-9 && (node.y - 2.0).abs() < 1e-9));
        // A shared joint is used by four segments.
        let shared = model
            .nodes
            .iter()
            .find(|(_, node)| (node.x - 2.0).abs() < 1e-9 && (node.y - 2.0).abs() < 1e-9)
            .map(|(&id, _)| id)
            .unwrap();
        let touching = model
            .elements
            .values()
            .filter(|element| element.node_i == shared || element.node_j == shared)
            .count();
        assert_eq!(touching, 4);
    }

    #[test]
    fn frame_splits_at_existing_midspan_joint() {
        let mut model = Model::new("midspan-joint");
        let n1 = model.add_node(0.0, 0.0, 0.0);
        let n2 = model.add_node(4.0, 0.0, 0.0);
        let n3 = model.add_node(2.0, 0.0, 0.0);
        model.add_element(Element::frame(1, n1, n2, 1, 1));

        let report = split_at_intersections(&mut model, 0.001);
        assert_eq!(report.joints_added, 0);
        assert_eq!(report.frames_split, 1);
        assert_eq!(model.nodes.len(), 3);
        assert_eq!(model.elements.len(), 2);
        assert!(model
            .elements
            .values()
            .all(|element| element.node_i == n3 || element.node_j == n3));
    }

    #[test]
    fn beam_splits_slab_and_itself_at_the_boundary() {
        let mut model = Model::new("beam-slab");
        let a = model.add_node(0.0, 0.0, 0.0);
        let b = model.add_node(4.0, 0.0, 0.0);
        let c = model.add_node(4.0, 3.0, 0.0);
        let d = model.add_node(0.0, 3.0, 0.0);
        let slab = model.add_area(Area::quadrilateral(a, b, c, d, 1, 0.2));
        let start = model.add_node(2.0, -1.0, 0.0);
        let end = model.add_node(2.0, 4.0, 0.0);
        model.add_element(Element::frame(1, start, end, 1, 1));

        let report = split_at_intersections(&mut model, 0.001);
        assert_eq!(report.slabs_split, 1);
        assert_eq!(report.frames_split, 1);
        assert_eq!(model.areas.len(), 2);
        assert!(!model.areas.contains_key(&slab));
        let total: f64 = model.areas.values().map(|area| area.area(&model)).sum();
        assert!((total - 12.0).abs() < 1e-6);
        // Beam: outside -> boundary -> inside -> boundary -> outside.
        assert_eq!(model.elements.len(), 3);
        // Both slab pieces and the middle beam segment share the boundary joints.
        let boundary_joints = model
            .nodes
            .values()
            .filter(|node| {
                (node.x - 2.0).abs() < 1e-9 && (node.y - 0.0).abs() < 1e-9
                    || (node.x - 2.0).abs() < 1e-9 && (node.y - 3.0).abs() < 1e-9
            })
            .count();
        assert_eq!(boundary_joints, 2);
    }

    #[test]
    fn area_perimeter_gains_joints_lying_on_its_edges() {
        let mut model = Model::new("edge-joint");
        let a = model.add_node(0.0, 0.0, 0.0);
        let b = model.add_node(4.0, 0.0, 0.0);
        let c = model.add_node(4.0, 3.0, 0.0);
        let d = model.add_node(0.0, 3.0, 0.0);
        let mid = model.add_node(2.0, 0.0, 0.0);
        let area = model.add_area(Area::quadrilateral(a, b, c, d, 1, 0.2));

        let report = split_at_intersections(&mut model, 0.001);
        assert_eq!(report.areas_updated, 1);
        let stored = model.areas.get(&area).unwrap();
        assert_eq!(stored.node_ids.len(), 5);
        assert!(stored.node_ids.contains(&mid));
        assert!((stored.area(&model) - 12.0).abs() < 1e-6);
    }

    #[test]
    fn curved_circle_splits_at_true_ellipse_intersections() {
        let mut model = Model::new("curve-split");
        let curve = crate::areas::AreaCurve::ellipse([2.0, 2.0, 0.0], 2.0, 2.0);
        let ids: Vec<usize> = curve
            .points(64)
            .into_iter()
            .map(|point| model.add_node(point[0], point[1], point[2]))
            .collect();
        let mut area = Area::new(ids, AreaKind::Slab, 1, 0.2);
        area.curve = Some(curve);
        model.add_area(area);
        let start = model.add_node(2.0, -1.0, 0.0);
        let end = model.add_node(2.0, 5.0, 0.0);
        model.add_element(Element::frame(1, start, end, 1, 1));

        let report = split_at_intersections(&mut model, 0.001);
        assert_eq!(report.slabs_split, 1);
        assert_eq!(model.areas.len(), 2);
        let total: f64 = model.areas.values().map(|area| area.area(&model)).sum();
        assert!((total - std::f64::consts::PI * 4.0).abs() < 1e-6);
        for area in model.areas.values() {
            let curve = area.curve.as_ref().unwrap();
            assert!(!curve.closed);
            assert_eq!(area.node_ids.len(), 2);
            for id in &area.node_ids {
                let node = model.nodes.get(id).unwrap();
                assert!((node.x - 2.0).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn polygon_split_preserves_total_area() {
        let square = shapes::rectangle((0.0, 0.0), (4.0, 3.0));
        let (left, right) = split_polygon_by_line(&square, (2.0, -1.0), (2.0, 5.0), 0.001).unwrap();
        let total = shapes::signed_area(&left).abs() + shapes::signed_area(&right).abs();
        assert!((total - 12.0).abs() < 1e-9);
    }
}
