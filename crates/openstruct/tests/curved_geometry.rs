use openstruct::areas::{Area, AreaCurve, AreaKind};
use openstruct::edit::{mirror_selection, replicate_linear, MirrorAxis};
use openstruct::elements::Element;
use openstruct::model::Model;
use openstruct::split::split_at_intersections;
use std::collections::HashSet;

#[test]
fn curved_circle_splits_at_true_ellipse_intersections() {
    let mut model = Model::new("curve-split");
    let curve = AreaCurve::ellipse([2.0, 2.0, 0.0], 2.0, 2.0);
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
        assert!(!area.curve.as_ref().unwrap().closed);
        assert_eq!(area.node_ids.len(), 2);
        for id in &area.node_ids {
            assert!((model.nodes[id].x - 2.0).abs() < 1e-9);
        }
    }
}

#[test]
fn curved_area_follows_replicate_and_mirror() {
    let mut model = Model::new("curve-edit");
    let a = model.add_node(4.0, 0.0, 0.0);
    let b = model.add_node(0.0, 1.0, 0.0);
    let mut area = Area::new(vec![a, b], AreaKind::Slab, 1, 0.2);
    area.curve = Some(AreaCurve::ellipse([2.0, 0.0, 0.0], 2.0, 1.0));
    let id = model.add_area(area);
    let areas: HashSet<usize> = [id].into_iter().collect();

    let replicated = replicate_linear(
        &mut model,
        &HashSet::new(),
        &HashSet::new(),
        &areas,
        [0.0, 0.0, 3.0],
        1,
    );
    let copy = &model.areas[&replicated.area_pairs[0].1];
    assert_eq!(copy.curve.as_ref().unwrap().center, [2.0, 0.0, 3.0]);

    let mirrored = mirror_selection(
        &mut model,
        &HashSet::new(),
        &HashSet::new(),
        &areas,
        MirrorAxis::X,
        0.0,
    );
    let copy = &model.areas[&mirrored.area_pairs[0].1];
    assert!((copy.curve.as_ref().unwrap().center[0] + 2.0).abs() < 1e-9);
    assert!((copy.area(&model) - std::f64::consts::PI * 2.0).abs() < 1e-9);
}
