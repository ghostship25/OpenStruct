//! Headless checks of the New Model builder.
//!
//! Each test configures the New Model settings exactly as the dialog would and
//! calls the real `build_new_model`, so the templates are checked without a
//! window: joint and frame counts, story elevations, and the design codes that
//! reach the application state.

use crate::app::{NewModelTemplate, OpenStructApp};
use openstruct::design::codes::DesignCode;

const NX: usize = 2;
const NY: usize = 3;
const DX: f64 = 6.0;
const DY: f64 = 4.0;
const STORIES: usize = 4;
const BASE_HEIGHT: f64 = 4.5;
const TYPICAL_HEIGHT: f64 = 3.2;

/// Story elevations: the bottom storey uses its own height, the rest the typical.
const STORY_ELEVATIONS: [f64; 4] = [0.0, 4.5, 7.7, 10.9];
/// Level elevations, including the top level above the last story.
const LEVEL_ELEVATIONS: [f64; 5] = [0.0, 4.5, 7.7, 10.9, 14.1];

const STEEL_CODE: DesignCode = DesignCode::Eurocode;
const CONCRETE_CODE: DesignCode = DesignCode::Indian;

fn build(template: NewModelTemplate) -> OpenStructApp {
    let mut app = OpenStructApp::fresh();
    app.nm_nx = NX;
    app.nm_ny = NY;
    app.nm_dx = DX;
    app.nm_dy = DY;
    app.nm_stories = STORIES;
    app.nm_base_h = BASE_HEIGHT;
    app.nm_typical_h = TYPICAL_HEIGHT;
    app.nm_template = template;
    app.nm_steel_code = STEEL_CODE;
    app.nm_concrete_code = CONCRETE_CODE;
    app.build_new_model();
    app
}

fn assert_close(actual: &[f64], expected: &[f64]) {
    assert_eq!(actual.len(), expected.len(), "got {actual:?}, expected {expected:?}");
    for (value, wanted) in actual.iter().zip(expected) {
        assert!((value - wanted).abs() < 1e-9, "got {actual:?}, expected {expected:?}");
    }
}

fn story_elevations(app: &OpenStructApp) -> Vec<f64> {
    app.model.stories.iter().map(|story| story.elevation).collect()
}

/// Joints sitting at each of the level elevations.
fn joints_per_level(app: &OpenStructApp) -> Vec<usize> {
    LEVEL_ELEVATIONS
        .iter()
        .map(|z| app.model.nodes.values().filter(|node| (node.z - z).abs() < 1e-9).count())
        .collect()
}

fn assert_codes_reached(app: &OpenStructApp) {
    assert_eq!(app.design_code, STEEL_CODE, "the steel code chosen in New Model did not reach the app");
    assert_eq!(
        app.concrete_design_code, CONCRETE_CODE,
        "the concrete code chosen in New Model did not reach the app"
    );
}

#[test]
fn blank_template_creates_the_grid_and_stories_only() {
    let app = build(NewModelTemplate::Blank);

    assert_eq!(app.model.stories.len(), STORIES);
    assert_close(&story_elevations(&app), &STORY_ELEVATIONS);
    assert_eq!(app.model.nodes.len(), 0, "the blank template must not place joints");
    assert_eq!(app.model.elements.len(), 0, "the blank template must not place frames");

    assert_eq!(app.grid.xs.len(), NX + 1);
    assert_eq!(app.grid.ys.len(), NY + 1);
    assert!((app.grid.xs[1] - app.grid.xs[0] - DX).abs() < 1e-9, "grid spacing along X is not the New Model value");
    assert!((app.grid.ys[1] - app.grid.ys[0] - DY).abs() < 1e-9, "grid spacing along Y is not the New Model value");

    assert_codes_reached(&app);
}

#[test]
fn joints_template_places_a_joint_at_every_intersection_on_every_level() {
    let app = build(NewModelTemplate::Joints);

    assert_eq!(app.model.stories.len(), STORIES);
    assert_close(&story_elevations(&app), &STORY_ELEVATIONS);

    let per_level = (NX + 1) * (NY + 1);
    assert_eq!(per_level, 12, "2 × 3 bays must give 12 intersections");
    assert_eq!(
        app.model.nodes.len(),
        per_level * LEVEL_ELEVATIONS.len(),
        "a joint is expected at every intersection of all five levels"
    );
    assert_eq!(joints_per_level(&app), vec![per_level; LEVEL_ELEVATIONS.len()]);
    assert_eq!(app.model.elements.len(), 0, "the joints template must not place frames");
    assert!(
        app.model.nodes.values().all(|node| !node.restrained.iter().any(|flag| *flag)),
        "nothing is fixed in the joints template"
    );

    assert_codes_reached(&app);
}

#[test]
fn building_template_adds_columns_above_every_joint_and_beams_on_supported_levels() {
    let app = build(NewModelTemplate::Building);

    assert_eq!(app.model.stories.len(), STORIES);
    assert_close(&story_elevations(&app), &STORY_ELEVATIONS);

    let side_x = NX + 1;
    let side_y = NY + 1;
    let per_level = side_x * side_y;
    assert_eq!(app.model.nodes.len(), per_level * LEVEL_ELEVATIONS.len());
    assert_eq!(joints_per_level(&app), vec![per_level; LEVEL_ELEVATIONS.len()]);

    let columns = STORIES * per_level;
    let beams_per_level = side_x * (side_y - 1) + (side_x - 1) * side_y;
    let beams = STORIES * beams_per_level;
    assert_eq!(
        app.model.elements.len(),
        columns + beams,
        "expected {columns} columns and {beams} beams"
    );

    let fixed = app
        .model
        .nodes
        .values()
        .filter(|node| node.z.abs() < 1e-9 && node.restrained == [true; 6])
        .count();
    assert_eq!(fixed, per_level, "every base joint of the building frame must be fixed");

    assert_codes_reached(&app);
}
