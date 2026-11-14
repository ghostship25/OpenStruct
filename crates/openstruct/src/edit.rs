//! Model editing operations shared by the GUI tools.
//!
//! These are pure model transforms: no UI state, no undo bookkeeping. The GUI
//! records an undo snapshot before calling them.

use crate::model::Model;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MirrorAxis {
    /// Reflect across the vertical plane X = pivot.
    X,
    Y,
}

/// Result of a copy/mirror/replicate operation, with old → new id pairs.
///
/// Pairs (rather than a map) keep every copy of a replicated selection, so the
/// caller can carry loads to all of them and select every new object.
#[derive(Debug, Default)]
pub struct CopyResult {
    pub node_pairs: Vec<(usize, usize)>,
    pub element_pairs: Vec<(usize, usize)>,
    pub area_pairs: Vec<(usize, usize)>,
}

impl CopyResult {
    pub fn new_nodes(&self) -> HashSet<usize> {
        self.node_pairs.iter().map(|&(_, new_id)| new_id).collect()
    }
    pub fn new_elements(&self) -> HashSet<usize> {
        self.element_pairs
            .iter()
            .map(|&(_, new_id)| new_id)
            .collect()
    }

    pub fn new_areas(&self) -> HashSet<usize> {
        self.area_pairs.iter().map(|&(_, new_id)| new_id).collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlignAxis {
    X,
    Y,
    Z,
}

impl AlignAxis {
    pub fn label(self) -> &'static str {
        match self {
            AlignAxis::X => "X",
            AlignAxis::Y => "Y",
            AlignAxis::Z => "Z",
        }
    }

    pub fn index(self) -> usize {
        match self {
            AlignAxis::X => 0,
            AlignAxis::Y => 1,
            AlignAxis::Z => 2,
        }
    }
}

/// Set one coordinate of every listed joint, e.g. align a row of joints to X = 6.
pub fn align_nodes(
    model: &mut Model,
    node_ids: &HashSet<usize>,
    axis: AlignAxis,
    target: f64,
) -> usize {
    let index = axis.index();
    let mut aligned = 0;
    for &id in node_ids {
        if let Some(node) = model.nodes.get_mut(&id) {
            match index {
                0 => node.x = target,
                1 => node.y = target,
                _ => node.z = target,
            }
            aligned += 1;
        }
    }
    aligned
}

/// Report from [`trim_extend_to_boundaries`].
#[derive(Debug, Default)]
pub struct TrimReport {
    pub ends_moved: usize,
    pub ends_skipped: usize,
}

/// Trim or extend the free ends of the listed frames to the nearest crossing
/// with any frame that is not in `element_ids`.
///
/// A free end is a joint used by no other frame, no area and carrying no
/// restraint. The end moves along its own frame axis to the nearest intersection
/// with a boundary frame, so a long member is shortened and a short one is
/// extended. Ends already at a crossing are skipped.
pub fn trim_extend_to_boundaries(
    model: &mut Model,
    element_ids: &HashSet<usize>,
    tolerance: f64,
) -> TrimReport {
    let tol = tolerance.max(1e-9);
    let mut report = TrimReport::default();

    let mut selected: Vec<(usize, usize, usize)> = Vec::new();
    for &id in element_ids {
        if let Some(element) = model.elements.get(&id) {
            selected.push((id, element.node_i, element.node_j));
        }
    }
    selected.sort_unstable();

    let boundaries: Vec<([f64; 3], [f64; 3])> = model
        .elements
        .iter()
        .filter(|(id, _)| !element_ids.contains(id))
        .filter_map(|(_, element)| {
            let a = model.nodes.get(&element.node_i)?;
            let b = model.nodes.get(&element.node_j)?;
            Some(([a.x, a.y, a.z], [b.x, b.y, b.z]))
        })
        .collect();

    let mut moved_nodes: HashSet<usize> = HashSet::new();
    for (element_id, node_i, node_j) in selected {
        for (fixed, moving) in [(node_i, node_j), (node_j, node_i)] {
            if moved_nodes.contains(&moving) {
                continue;
            }
            let shared = model.elements.values().any(|other| {
                other.id != element_id && (other.node_i == moving || other.node_j == moving)
            });
            let in_area = model
                .areas
                .values()
                .any(|area| area.node_ids.contains(&moving));
            let restrained = model
                .nodes
                .get(&moving)
                .is_some_and(|node| node.restrained.iter().any(|&fixed| fixed));
            if shared || in_area || restrained {
                report.ends_skipped += 1;
                continue;
            }
            let (Some(fixed_node), Some(moving_node)) =
                (model.nodes.get(&fixed), model.nodes.get(&moving))
            else {
                report.ends_skipped += 1;
                continue;
            };
            let p0 = [fixed_node.x, fixed_node.y, fixed_node.z];
            let p1 = [moving_node.x, moving_node.y, moving_node.z];
            if distance(p0, p1) < tol {
                report.ends_skipped += 1;
                continue;
            }

            let mut best: Option<(f64, [f64; 3])> = None;
            for &(q0, q1) in &boundaries {
                let Some((u, point)) = line_segment_crossing(p0, p1, q0, q1, tol) else {
                    continue;
                };
                if u <= 1e-6 {
                    continue;
                }
                match best {
                    Some((best_u, _)) if (u - 1.0).abs() >= (best_u - 1.0).abs() => {}
                    _ => best = Some((u, point)),
                }
            }

            let Some((u, point)) = best else {
                report.ends_skipped += 1;
                continue;
            };
            if (u - 1.0).abs() < 1e-6 {
                report.ends_skipped += 1;
                continue;
            }
            if let Some(node) = model.nodes.get_mut(&moving) {
                node.x = point[0];
                node.y = point[1];
                node.z = point[2];
                moved_nodes.insert(moving);
                report.ends_moved += 1;
            }
        }
    }
    report
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    let d = sub(a, b);
    dot(d, d).sqrt()
}

/// Intersection of the infinite line through `p0`–`p1` with the segment
/// `q0`–`q1`. Returns the parametric position on the first line (1.0 = `p1`)
/// and the intersection point.
fn line_segment_crossing(
    p0: [f64; 3],
    p1: [f64; 3],
    q0: [f64; 3],
    q1: [f64; 3],
    tolerance: f64,
) -> Option<(f64, [f64; 3])> {
    let r = sub(p1, p0);
    let s = sub(q1, q0);
    let a = dot(r, r);
    let e = dot(s, s);
    if a < 1e-18 || e < 1e-18 {
        return None;
    }
    let w = sub(p0, q0);
    let b = dot(r, s);
    let c = dot(r, w);
    let f = dot(s, w);
    let denom = a * e - b * b;
    if denom.abs() < 1e-12 {
        return None;
    }
    let u = (b * f - c * e) / denom;
    let t = (a * f - b * c) / denom;
    if !(0.0..=1.0).contains(&t) {
        return None;
    }
    let on_selected = [p0[0] + r[0] * u, p0[1] + r[1] * u, p0[2] + r[2] * u];
    let on_boundary = [q0[0] + s[0] * t, q0[1] + s[1] * t, q0[2] + s[2] * t];
    if distance(on_selected, on_boundary) > tolerance {
        return None;
    }
    Some((u, on_boundary))
}

/// Move the given joints by a global delta.
pub fn move_nodes(model: &mut Model, node_ids: &HashSet<usize>, delta: [f64; 3]) -> usize {
    let mut moved = 0;
    for &id in node_ids {
        if let Some(node) = model.nodes.get_mut(&id) {
            node.x += delta[0];
            node.y += delta[1];
            node.z += delta[2];
            moved += 1;
        }
    }
    moved
}

/// Rotate joints about a vertical axis through `pivot` by `angle_degrees`
/// (counter-clockwise seen from +Z). Frames and areas follow their joints.
pub fn rotate_nodes(
    model: &mut Model,
    node_ids: &HashSet<usize>,
    pivot: (f64, f64),
    angle_degrees: f64,
) -> usize {
    let radians = angle_degrees.to_radians();
    let (sin, cos) = radians.sin_cos();
    let mut rotated = 0;
    for &id in node_ids {
        if let Some(node) = model.nodes.get_mut(&id) {
            let dx = node.x - pivot.0;
            let dy = node.y - pivot.1;
            node.x = pivot.0 + dx * cos - dy * sin;
            node.y = pivot.1 + dx * sin + dy * cos;
            rotated += 1;
        }
    }
    rotated
}

/// Copy the selected joints and frames, transforming every copied coordinate.
///
/// Joints used by selected frames are copied too, even when they are not
/// themselves selected. Restraints are carried over; loads are handled by the
/// caller through [`CopyResult::node_map`].
pub fn copy_selection<F>(
    model: &mut Model,
    node_ids: &HashSet<usize>,
    element_ids: &HashSet<usize>,
    area_ids: &HashSet<usize>,
    transform: F,
) -> CopyResult
where
    F: Fn(f64, f64, f64) -> (f64, f64, f64),
{
    let mut involved: HashSet<usize> = node_ids.clone();
    for &element_id in element_ids {
        if let Some(element) = model.elements.get(&element_id) {
            involved.insert(element.node_i);
            involved.insert(element.node_j);
        }
    }
    for &area_id in area_ids {
        if let Some(area) = model.areas.get(&area_id) {
            involved.extend(area.node_ids.iter().copied());
        }
    }

    let mut result = CopyResult::default();
    let mut node_map: HashMap<usize, usize> = HashMap::new();

    let mut node_ids_sorted: Vec<usize> = involved.into_iter().collect();
    node_ids_sorted.sort_unstable();
    for old_id in node_ids_sorted {
        let Some(node) = model.nodes.get(&old_id) else {
            continue;
        };
        let (x, y, z, restrained, springs) = (node.x, node.y, node.z, node.restrained, node.springs);
        let (nx, ny, nz) = transform(x, y, z);
        let new_id = model.add_node(nx, ny, nz);
        if let Some(copy) = model.nodes.get_mut(&new_id) {
            copy.restrained = restrained;
            copy.springs = springs;
        }
        node_map.insert(old_id, new_id);
        result.node_pairs.push((old_id, new_id));
    }

    let mut element_ids_sorted: Vec<usize> = element_ids.iter().copied().collect();
    element_ids_sorted.sort_unstable();
    for old_id in element_ids_sorted {
        let Some(element) = model.elements.get(&old_id) else {
            continue;
        };
        let (Some(&ni), Some(&nj)) = (node_map.get(&element.node_i), node_map.get(&element.node_j))
        else {
            continue;
        };
        let next_id = model.element_counter + 1;
        let mut copy = element.clone();
        copy.id = next_id;
        copy.node_i = ni;
        copy.node_j = nj;
        let new_id = model.add_element(copy);
        result.element_pairs.push((old_id, new_id));
    }

    let mut area_ids_sorted: Vec<usize> = area_ids.iter().copied().collect();
    area_ids_sorted.sort_unstable();
    for old_id in area_ids_sorted {
        let Some(area) = model.areas.get(&old_id).cloned() else {
            continue;
        };
        let mut mapped = Vec::with_capacity(area.node_ids.len());
        for node_id in &area.node_ids {
            let Some(new_id) = node_map.get(node_id) else {
                mapped.clear();
                break;
            };
            mapped.push(*new_id);
        }
        let minimum = if area.curve.is_some() { 2 } else { 3 };
        if mapped.len() < minimum {
            continue;
        }
        let mut copy = area;
        copy.node_ids = mapped;
        let new_id = model.add_area(copy);
        result.area_pairs.push((old_id, new_id));
    }

    result
}

/// Replicate the selection `copies` times along a linear delta.
pub fn replicate_linear(
    model: &mut Model,
    node_ids: &HashSet<usize>,
    element_ids: &HashSet<usize>,
    area_ids: &HashSet<usize>,
    delta: [f64; 3],
    copies: usize,
) -> CopyResult {
    let mut result = CopyResult::default();
    for step in 1..=copies {
        let offset = step as f64;
        let copy = copy_selection(model, node_ids, element_ids, area_ids, |x, y, z| {
            (
                x + delta[0] * offset,
                y + delta[1] * offset,
                z + delta[2] * offset,
            )
        });
        result.node_pairs.extend(copy.node_pairs);
        for &(_, new_id) in &copy.area_pairs {
            if let Some(area) = model.areas.get_mut(&new_id) {
                if let Some(curve) = area.curve.as_mut() {
                    curve.translate([
                        delta[0] * offset,
                        delta[1] * offset,
                        delta[2] * offset,
                    ]);
                }
            }
        }
        result.element_pairs.extend(copy.element_pairs);
        result.area_pairs.extend(copy.area_pairs);
    }
    result
}

/// Mirror the selection across a vertical plane, reversing copied frame ends so
/// their local axes stay right-handed.
pub fn mirror_selection(
    model: &mut Model,
    node_ids: &HashSet<usize>,
    element_ids: &HashSet<usize>,
    area_ids: &HashSet<usize>,
    axis: MirrorAxis,
    pivot: f64,
) -> CopyResult {
    let transform = move |x: f64, y: f64, z: f64| match axis {
        MirrorAxis::X => (2.0 * pivot - x, y, z),
        MirrorAxis::Y => (x, 2.0 * pivot - y, z),
    };
    let result = copy_selection(model, node_ids, element_ids, area_ids, transform);
    for &(_, new_id) in &result.element_pairs {
        if let Some(element) = model.elements.get_mut(&new_id) {
            std::mem::swap(&mut element.node_i, &mut element.node_j);
        }
    }
    // Reverse straight perimeters so mirrored areas keep their winding (and
    // normal). A curve owns its arc direction; mirroring it explicitly keeps
    // the endpoints and the rendered boundary in sync.
    for &(_, new_id) in &result.area_pairs {
        if let Some(area) = model.areas.get_mut(&new_id) {
            match area.curve.as_mut() {
                Some(curve) => match axis {
                    MirrorAxis::X => curve.mirror_x(pivot),
                    MirrorAxis::Y => curve.mirror_y(pivot),
                },
                None => area.node_ids.reverse(),
            }
        }
    }
    result
}

/// Merge coincident joints (within `tolerance`) and remap frames.
///
/// With an empty `node_ids` set every joint is considered; otherwise only the
/// listed joints are merged. Returns the number of joints removed and the full
/// old → representative id map.
pub fn merge_joints(
    model: &mut Model,
    node_ids: &HashSet<usize>,
    tolerance: f64,
) -> (usize, HashMap<usize, usize>) {
    let mut ids: Vec<usize> = if node_ids.is_empty() {
        model.nodes.keys().copied().collect()
    } else {
        node_ids.iter().copied().collect()
    };
    ids.sort_unstable();
    if ids.len() < 2 {
        return (0, model.nodes.keys().map(|&id| (id, id)).collect());
    }

    let tolerance = tolerance.max(1e-9);
    let tolerance_sq = tolerance * tolerance;
    let mut map: HashMap<usize, usize> = model.nodes.keys().map(|&id| (id, id)).collect();
    let mut representatives: Vec<usize> = Vec::new();

    for id in ids {
        let Some(node) = model.nodes.get(&id) else {
            continue;
        };
        let (x, y, z) = (node.x, node.y, node.z);
        let mut merged_into = None;
        for &representative in &representatives {
            if let Some(other) = model.nodes.get(&representative) {
                let distance_sq =
                    (other.x - x).powi(2) + (other.y - y).powi(2) + (other.z - z).powi(2);
                if distance_sq <= tolerance_sq {
                    merged_into = Some(representative);
                    break;
                }
            }
        }
        match merged_into {
            Some(representative) => {
                map.insert(id, representative);
            }
            None => representatives.push(id),
        }
    }

    // Areas follow their joints; drop any that collapse below three corners.
    let area_ids: Vec<usize> = model.areas.keys().copied().collect();
    for area_id in area_ids {
        let Some(mut area) = model.areas.get(&area_id).cloned() else {
            continue;
        };
        let mut remapped: Vec<usize> = Vec::with_capacity(area.node_ids.len());
        for node_id in &area.node_ids {
            let mapped = *map.get(node_id).unwrap_or(node_id);
            if !remapped.contains(&mapped) {
                remapped.push(mapped);
            }
        }
        area.node_ids = remapped;
        let minimum = if area.curve.is_some() { 2 } else { 3 };
        if area.node_ids.len() < minimum || !area.is_valid(model) {
            model.areas.remove(&area_id);
        } else {
            model.areas.insert(area_id, area);
        }
    }

    let mut removed = 0;
    for (&old_id, &representative) in &map {
        if old_id != representative && model.nodes.remove(&old_id).is_some() {
            removed += 1;
        }
    }

    let element_ids: Vec<usize> = model.elements.keys().copied().collect();
    for element_id in element_ids {
        let Some((ni, nj)) = model
            .elements
            .get(&element_id)
            .map(|element| (element.node_i, element.node_j))
        else {
            continue;
        };
        let new_i = *map.get(&ni).unwrap_or(&ni);
        let new_j = *map.get(&nj).unwrap_or(&nj);
        if new_i == new_j {
            model.elements.remove(&element_id);
        } else if let Some(element) = model.elements.get_mut(&element_id) {
            element.node_i = new_i;
            element.node_j = new_j;
        }
    }

    (removed, map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elements::Element;
    use crate::materials::Material;
    use crate::sections::Section;

    fn model_with_frame() -> Model {
        let mut model = Model::new("edit-test");
        model.add_material(Material::steel());
        model.add_section(Section::rectangular("RECT", 0.3, 0.5));
        let n1 = model.add_node(0.0, 0.0, 0.0);
        let n2 = model.add_node(4.0, 0.0, 0.0);
        model.add_element(Element::frame(1, n1, n2, 1, 1));
        model
    }

    fn all_nodes(model: &Model) -> HashSet<usize> {
        model.nodes.keys().copied().collect()
    }

    fn all_elements(model: &Model) -> HashSet<usize> {
        model.elements.keys().copied().collect()
    }

    #[test]
    fn copy_moves_geometry_and_keeps_topology() {
        let mut model = model_with_frame();
        let before_nodes = model.nodes.len();
        let nodes = all_nodes(&model);
        let elements = all_elements(&model);
        let result = copy_selection(&mut model, &nodes, &elements, &HashSet::new(), |x, y, z| {
            (x, y, z + 3.0)
        });
        assert_eq!(result.new_nodes().len(), 2);
        assert_eq!(result.new_elements().len(), 1);
        assert_eq!(model.nodes.len(), before_nodes * 2);
        let copy = model
            .elements
            .get(&result.new_elements().into_iter().next().unwrap())
            .unwrap();
        assert!(model.nodes.get(&copy.node_i).unwrap().z == 3.0);
    }

    #[test]
    fn mirror_reflects_across_plane() {
        let mut model = model_with_frame();
        let nodes = all_nodes(&model);
        let elements = all_elements(&model);
        let result = mirror_selection(
            &mut model,
            &nodes,
            &elements,
            &HashSet::new(),
            MirrorAxis::X,
            0.0,
        );
        let mirrored_x: Vec<f64> = result
            .new_nodes()
            .into_iter()
            .map(|id| model.nodes.get(&id).unwrap().x)
            .collect();
        assert!(mirrored_x.contains(&-4.0));
        assert!(mirrored_x.contains(&0.0));
    }

    #[test]
    fn merge_removes_coincident_joint_and_degenerate_frame() {
        let mut model = model_with_frame();
        let n3 = model.add_node(4.001, 0.0, 0.0);
        let n4 = model.add_node(8.0, 0.0, 0.0);
        let n2 = model
            .nodes
            .keys()
            .find(|&&id| model.nodes.get(&id).unwrap().x == 4.0)
            .copied()
            .unwrap();
        model.add_element(Element::frame(2, n2, n3, 1, 1));
        model.add_element(Element::frame(3, n3, n4, 1, 1));

        let (removed, _) = merge_joints(&mut model, &HashSet::new(), 0.01);
        assert_eq!(removed, 1);
        assert_eq!(model.nodes.len(), 3);
        // n2→n3 frame collapses and is removed; n3→n4 is remapped.
        assert_eq!(model.elements.len(), 2);
    }

    #[test]
    fn rotate_turns_joints_about_the_pivot() {
        let mut model = Model::new("rotate");
        let a = model.add_node(1.0, 0.0, 0.0);
        let b = model.add_node(3.0, 0.0, 0.0);
        let ids: HashSet<usize> = [a, b].into_iter().collect();
        let rotated = rotate_nodes(&mut model, &ids, (0.0, 0.0), 90.0);
        assert_eq!(rotated, 2);
        let node = model.nodes.get(&a).unwrap();
        assert!(node.x.abs() < 1e-9);
        assert!((node.y - 1.0).abs() < 1e-9);
        let node = model.nodes.get(&b).unwrap();
        assert!(node.x.abs() < 1e-9);
        assert!((node.y - 3.0).abs() < 1e-9);
    }

    #[test]
    fn copy_includes_area_objects() {
        let mut model = model_with_frame();
        let a = model.add_node(0.0, 0.0, 0.0);
        let b = model.add_node(2.0, 0.0, 0.0);
        let c = model.add_node(2.0, 3.0, 0.0);
        let d = model.add_node(0.0, 3.0, 0.0);
        let area = model.add_area(crate::areas::Area::quadrilateral(a, b, c, d, 1, 0.2));
        let nodes = all_nodes(&model);
        let elements = all_elements(&model);
        let areas: HashSet<usize> = model.areas.keys().copied().collect();
        let result = copy_selection(&mut model, &nodes, &elements, &areas, |x, y, z| {
            (x, y, z + 3.0)
        });
        assert_eq!(result.new_areas().len(), 1);
        let copy = model
            .areas
            .get(&result.new_areas().into_iter().next().unwrap())
            .unwrap();
        assert_ne!(copy.id, area);
        assert_eq!(copy.node_ids.len(), 4);
        assert!((copy.area(&model) - 6.0).abs() < 1e-9);
    }

    #[test]
    fn curved_areas_follow_replicate_and_mirror() {
        use crate::areas::{Area, AreaCurve, AreaKind};
        let mut model = Model::new("curved-copy");
        let a = model.add_node(4.0, 0.0, 0.0);
        let b = model.add_node(0.0, 1.0, 0.0);
        let mut area = Area::new(vec![a, b], AreaKind::Slab, 1, 0.2);
        area.curve = Some(AreaCurve::ellipse([2.0, 0.0, 0.0], 2.0, 1.0));
        let id = model.add_area(area);
        let areas: HashSet<usize> = [id].into_iter().collect();

        let result = replicate_linear(
            &mut model,
            &HashSet::new(),
            &HashSet::new(),
            &areas,
            [0.0, 0.0, 3.0],
            1,
        );
        let copy = model.areas.get(&result.area_pairs[0].1).unwrap();
        assert_eq!(copy.curve.as_ref().unwrap().center, [2.0, 0.0, 3.0]);

        let result = mirror_selection(
            &mut model,
            &HashSet::new(),
            &HashSet::new(),
            &areas,
            MirrorAxis::X,
            0.0,
        );
        let mirrored = model.areas.get(&result.area_pairs[0].1).unwrap();
        let curve = mirrored.curve.as_ref().unwrap();
        assert!((curve.center[0] + 2.0).abs() < 1e-9);
        assert!((curve.area() - std::f64::consts::PI * 2.0).abs() < 1e-9);
    }

    #[test]
    fn merge_remaps_area_joints() {
        let mut model = Model::new("merge-area");
        let n1 = model.add_node(0.0, 0.0, 0.0);
        let n2 = model.add_node(4.0, 0.0, 0.0);
        let n3 = model.add_node(4.0, 3.0, 0.0);
        let n4 = model.add_node(0.0, 3.0, 0.0);
        let duplicate = model.add_node(4.001, 0.0, 0.0);
        let area = model.add_area(crate::areas::Area::quadrilateral(
            n1, duplicate, n3, n4, 1, 0.2,
        ));
        let (removed, _) = merge_joints(&mut model, &HashSet::new(), 0.01);
        assert_eq!(removed, 1);
        let stored = model.areas.get(&area).unwrap();
        assert!(stored.node_ids.contains(&n2));
        assert!(!stored.node_ids.contains(&duplicate));
        assert_eq!(model.areas.len(), 1);
        assert!((stored.area(&model) - 12.0).abs() < 1e-9);
    }

    #[test]
    fn align_sets_the_selected_axis() {
        let mut model = model_with_frame();
        let nodes = all_nodes(&model);
        assert_eq!(align_nodes(&mut model, &nodes, AlignAxis::X, 2.5), 2);
        assert!(model.nodes.values().all(|node| (node.x - 2.5).abs() < 1e-9));
        assert_eq!(align_nodes(&mut model, &nodes, AlignAxis::Z, 3.0), 2);
        assert!(model.nodes.values().all(|node| (node.z - 3.0).abs() < 1e-9));
    }

    #[test]
    fn trim_shortens_a_free_end_to_a_boundary() {
        let mut model = model_with_frame();
        let moving = model
            .nodes
            .iter()
            .find(|(_, node)| (node.x - 4.0).abs() < 1e-9)
            .map(|(&id, _)| id)
            .unwrap();
        let b1 = model.add_node(2.0, -2.0, 0.0);
        let b2 = model.add_node(2.0, 2.0, 0.0);
        model.add_element(Element::frame(2, b1, b2, 1, 1));

        let selected: HashSet<usize> = [1].into_iter().collect();
        let report = trim_extend_to_boundaries(&mut model, &selected, 0.01);
        assert_eq!(report.ends_moved, 1);
        assert!((model.nodes.get(&moving).unwrap().x - 2.0).abs() < 1e-9);
        assert!(model.nodes.get(&moving).unwrap().y.abs() < 1e-9);
    }

    #[test]
    fn extend_reaches_a_boundary_beyond_the_free_end() {
        let mut model = model_with_frame();
        let moving = model
            .nodes
            .iter()
            .find(|(_, node)| (node.x - 4.0).abs() < 1e-9)
            .map(|(&id, _)| id)
            .unwrap();
        let b1 = model.add_node(6.0, -2.0, 0.0);
        let b2 = model.add_node(6.0, 2.0, 0.0);
        model.add_element(Element::frame(2, b1, b2, 1, 1));

        let selected: HashSet<usize> = [1].into_iter().collect();
        let report = trim_extend_to_boundaries(&mut model, &selected, 0.01);
        assert_eq!(report.ends_moved, 1);
        assert!((model.nodes.get(&moving).unwrap().x - 6.0).abs() < 1e-9);
    }

    #[test]
    fn trim_leaves_connected_ends_alone() {
        let mut model = model_with_frame();
        let n1 = model
            .nodes
            .iter()
            .find(|(_, node)| node.x.abs() < 1e-9)
            .map(|(&id, _)| id)
            .unwrap();
        let n2 = model
            .nodes
            .iter()
            .find(|(_, node)| (node.x - 4.0).abs() < 1e-9)
            .map(|(&id, _)| id)
            .unwrap();
        let n3 = model.add_node(8.0, 0.0, 0.0);
        model.add_element(Element::frame(2, n2, n3, 1, 1));
        let b1 = model.add_node(6.0, -2.0, 0.0);
        let b2 = model.add_node(6.0, 2.0, 0.0);
        model.add_element(Element::frame(3, b1, b2, 1, 1));

        let selected: HashSet<usize> = [1].into_iter().collect();
        let report = trim_extend_to_boundaries(&mut model, &selected, 0.01);
        assert_eq!(report.ends_moved, 0);
        assert_eq!(report.ends_skipped, 2);
        assert!((model.nodes.get(&n1).unwrap().x).abs() < 1e-9);
        assert!((model.nodes.get(&n2).unwrap().x - 8.0).abs() > 1e-9);
    }

    #[test]
    fn replicate_creates_independent_copies() {
        let mut model = model_with_frame();
        let nodes = all_nodes(&model);
        let elements = all_elements(&model);
        let result = replicate_linear(
            &mut model,
            &nodes,
            &elements,
            &HashSet::new(),
            [0.0, 0.0, 3.5],
            2,
        );
        assert_eq!(result.new_nodes().len(), 4);
        assert_eq!(result.new_elements().len(), 2);
        assert_eq!(model.nodes.len(), 6);
        assert_eq!(model.elements.len(), 3);
    }
}
