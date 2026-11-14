//! Drawing canvases: the story-aware plan view and the orbitable 3D viewport.

use crate::app::{AreaShape, OpenStructApp, Tool};
use crate::theme;
use eframe::egui;
use nalgebra::DVector;
use openstruct::areas::AreaKind;
use openstruct::elements::Element;
use openstruct::loads::LoadDirection;
use openstruct::fem::{deflected_points, frame_rotation, offset_transformation};
use openstruct::model::Node;
use openstruct::shapes;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug)]
pub struct View {
    /// World coordinate (metres) shown at the view centre.
    pub center: (f64, f64),
    /// Pixels per metre.
    pub scale: f32,
}

impl View {
    pub fn to_screen(&self, rect: egui::Rect, x: f64, y: f64) -> egui::Pos2 {
        let center = rect.center();
        egui::pos2(center.x + ((x - self.center.0) as f32) * self.scale, center.y - ((y - self.center.1) as f32) * self.scale)
    }

    pub fn to_world(&self, rect: egui::Rect, point: egui::Pos2) -> (f64, f64) {
        let center = rect.center();
        (self.center.0 + ((point.x - center.x) / self.scale) as f64, self.center.1 - ((point.y - center.y) / self.scale) as f64)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CylindricalGrid {
    pub center: (f64, f64),
    /// Circumferential lines, one radius each (labels R1, R2, …).
    pub radii: Vec<f64>,
    /// Radial lines, in degrees (labels A1, A2, …).
    pub angles_deg: Vec<f64>,
    /// Optional user labels; blank or missing entries fall back to R1…Rn.
    #[serde(default)]
    pub ring_labels: Vec<String>,
    /// Optional user labels; blank or missing entries fall back to A1…An.
    #[serde(default)]
    pub radial_labels: Vec<String>,
}

impl CylindricalGrid {
    pub fn new(center: (f64, f64), ring_spacing: f64, rings: usize, start_angle_deg: f64, angle_step_deg: f64, angle_count: usize) -> Self {
        let spacing = ring_spacing.max(0.1);
        let radii = (1..=rings.max(1)).map(|index| index as f64 * spacing).collect();
        let angles_deg = (0..angle_count.max(1)).map(|index| start_angle_deg + index as f64 * angle_step_deg).collect();
        Self { center, radii, angles_deg, ring_labels: Vec::new(), radial_labels: Vec::new() }
    }

    pub fn outer_radius(&self) -> f64 {
        self.radii.iter().copied().fold(0.0, f64::max)
    }

    pub fn ring_label(&self, index: usize) -> String {
        self.ring_labels.get(index).filter(|label| !label.trim().is_empty()).cloned().unwrap_or_else(|| format!("R{}", index + 1))
    }

    pub fn radial_label(&self, index: usize) -> String {
        self.radial_labels.get(index).filter(|label| !label.trim().is_empty()).cloned().unwrap_or_else(|| format!("A{}", index + 1))
    }
}

/// Normal direction of a reference plane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RefPlaneAxis {
    X,
    Y,
    Z,
}

impl RefPlaneAxis {
    pub fn label(self) -> &'static str {
        match self {
            RefPlaneAxis::X => "X",
            RefPlaneAxis::Y => "Y",
            RefPlaneAxis::Z => "Z",
        }
    }

    /// World axes (0=X, 1=Y, 2=Z) of the two in-plane directions.
    pub fn plane_axes(self) -> (usize, usize) {
        match self {
            RefPlaneAxis::X => (1, 2),
            RefPlaneAxis::Y => (0, 2),
            RefPlaneAxis::Z => (0, 1),
        }
    }
}

/// A drawing plane: the infinite plane normal to `axis` at `offset` metres.
/// All drawing, picking and snapping happens in the plane's local (u, v)
/// coordinates, which correspond to the two world axes in `plane_axes()`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorkPlane {
    pub axis: RefPlaneAxis,
    pub offset: f64,
}

impl WorkPlane {
    pub fn new(axis: RefPlaneAxis, offset: f64) -> Self {
        Self { axis, offset }
    }

    pub fn is_horizontal(self) -> bool {
        self.axis == RefPlaneAxis::Z
    }

    pub fn contains(self, point: [f64; 3], tolerance: f64) -> bool {
        let component = match self.axis {
            RefPlaneAxis::X => point[0],
            RefPlaneAxis::Y => point[1],
            RefPlaneAxis::Z => point[2],
        };
        (component - self.offset).abs() < tolerance
    }

    /// World point to plane-local (u, v).
    pub fn to_local(self, point: [f64; 3]) -> (f64, f64) {
        let (u_axis, v_axis) = self.axis.plane_axes();
        (point[u_axis], point[v_axis])
    }

    /// Plane-local (u, v) to a world point on the plane.
    pub fn to_world(self, u: f64, v: f64) -> [f64; 3] {
        let (u_axis, v_axis) = self.axis.plane_axes();
        let mut point = [0.0; 3];
        point[u_axis] = u;
        point[v_axis] = v;
        point[match self.axis {
            RefPlaneAxis::X => 0,
            RefPlaneAxis::Y => 1,
            RefPlaneAxis::Z => 2,
        }] = self.offset;
        point
    }

    /// Labels of the local horizontal/vertical axes, used in the status bar.
    pub fn axis_labels(self) -> (&'static str, &'static str) {
        match self.axis {
            RefPlaneAxis::X => ("Y", "Z"),
            RefPlaneAxis::Y => ("X", "Z"),
            RefPlaneAxis::Z => ("X", "Y"),
        }
    }
}

/// A named drawing aid: the infinite plane normal to `axis` at `offset` (metres).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RefPlane {
    pub name: String,
    pub axis: RefPlaneAxis,
    pub offset: f64,
}

impl RefPlane {
    pub fn new(name: impl Into<String>, axis: RefPlaneAxis, offset: f64) -> Self {
        Self { name: name.into(), axis, offset }
    }

    pub fn describe(&self) -> String {
        format!("{} = {:.3} m", self.axis.label(), self.offset)
    }

    /// Rectangle outline of the plane, clipped to the given model extents.
    pub fn corners(&self, x0: f64, x1: f64, y0: f64, y1: f64, z0: f64, z1: f64) -> [[f64; 3]; 4] {
        let o = self.offset;
        match self.axis {
            RefPlaneAxis::X => [[o, y0, z0], [o, y1, z0], [o, y1, z1], [o, y0, z1]],
            RefPlaneAxis::Y => [[x0, o, z0], [x1, o, z0], [x1, o, z1], [x0, o, z1]],
            RefPlaneAxis::Z => [[x0, y0, o], [x1, y0, o], [x1, y1, o], [x0, y1, o]],
        }
    }
}

/// Grid system name used for a grid that has never been named.
pub fn default_grid_name() -> String {
    "G1".to_string()
}

/// Label bubble radius in metres: the 1250 mm default.
pub fn default_bubble_size() -> f64 {
    1.25
}

/// Where a grid line draws its label bubble.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BubbleLoc {
    /// Both ends, the default plan presentation.
    #[default]
    Both,
    /// The first end: top for a line at constant X, left for a line at constant Y.
    Start,
    /// The second end: bottom for a line at constant X, right for a line at constant Y.
    End,
}

impl BubbleLoc {
    pub fn label(self) -> &'static str {
        match self {
            Self::Both => "Both",
            Self::Start => "Start",
            Self::End => "End",
        }
    }
}

/// Screen positions where a line draws its label bubble(s).
fn bubble_ends(loc: BubbleLoc, start: egui::Pos2, end: egui::Pos2) -> Vec<egui::Pos2> {
    match loc {
        BubbleLoc::Both => vec![start, end],
        BubbleLoc::Start => vec![start],
        BubbleLoc::End => vec![end],
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct GridSpec {
    /// Grid-local ordinates of the lines at constant X, in metres.
    pub xs: Vec<f64>,
    /// Grid-local ordinates of the lines at constant Y, in metres.
    pub ys: Vec<f64>,
    #[serde(default)]
    pub cylindrical: Option<CylindricalGrid>,
    /// Optional labels for lines at constant X; blank entries use 1, 2, 3…
    #[serde(default)]
    pub x_labels: Vec<String>,
    /// Optional labels for lines at constant Y; blank entries use A, B, C…
    #[serde(default)]
    pub y_labels: Vec<String>,
    /// Grid system name, as the Grid System Data dialog shows it.
    #[serde(default = "default_grid_name")]
    pub name: String,
    /// Global coordinates of the grid origin, in metres.
    #[serde(default)]
    pub origin: (f64, f64),
    /// Rotation of the grid about its origin, degrees counter-clockwise.
    #[serde(default)]
    pub rotation_deg: f64,
    /// Radius of the grid label bubbles, in metres.
    #[serde(default = "default_bubble_size")]
    pub bubble_size: f64,
    /// True when the Grid System Data dialog lists spacings instead of ordinates.
    #[serde(default)]
    pub display_as_spacing: bool,
    /// Per-line display flags for lines at constant X; missing entries are visible.
    #[serde(default)]
    pub x_visible: Vec<bool>,
    /// Per-line display flags for lines at constant Y; missing entries are visible.
    #[serde(default)]
    pub y_visible: Vec<bool>,
    /// Per-line bubble locations for lines at constant X; missing entries are Both.
    #[serde(default)]
    pub x_bubble: Vec<BubbleLoc>,
    /// Per-line bubble locations for lines at constant Y; missing entries are Both.
    #[serde(default)]
    pub y_bubble: Vec<BubbleLoc>,
}

impl GridSpec {
    pub fn new(spacing: f64, nx: usize, ny: usize) -> Self {
        Self::from_ordinates((0..=nx).map(|i| i as f64 * spacing).collect(), (0..=ny).map(|i| i as f64 * spacing).collect())
    }

    /// A rectangular grid from explicit local ordinates, with the new-grid defaults.
    pub fn from_ordinates(xs: Vec<f64>, ys: Vec<f64>) -> Self {
        Self {
            xs,
            ys,
            cylindrical: None,
            x_labels: Vec::new(),
            y_labels: Vec::new(),
            name: default_grid_name(),
            origin: (0.0, 0.0),
            rotation_deg: 0.0,
            bubble_size: default_bubble_size(),
            display_as_spacing: false,
            x_visible: Vec::new(),
            y_visible: Vec::new(),
            x_bubble: Vec::new(),
            y_bubble: Vec::new(),
        }
    }

    pub fn cylindrical(center: (f64, f64), ring_spacing: f64, rings: usize, start_angle_deg: f64, angle_step_deg: f64, angle_count: usize) -> Self {
        Self {
            xs: Vec::new(),
            ys: Vec::new(),
            cylindrical: Some(CylindricalGrid::new(center, ring_spacing, rings, start_angle_deg, angle_step_deg, angle_count)),
            x_labels: Vec::new(),
            y_labels: Vec::new(),
            name: default_grid_name(),
            origin: (0.0, 0.0),
            rotation_deg: 0.0,
            bubble_size: default_bubble_size(),
            display_as_spacing: false,
            x_visible: Vec::new(),
            y_visible: Vec::new(),
            x_bubble: Vec::new(),
            y_bubble: Vec::new(),
        }
    }

    /// True when the line at constant X with this index is displayed.
    pub fn x_line_visible(&self, i: usize) -> bool {
        self.x_visible.get(i).copied().unwrap_or(true)
    }

    /// True when the line at constant Y with this index is displayed.
    pub fn y_line_visible(&self, i: usize) -> bool {
        self.y_visible.get(i).copied().unwrap_or(true)
    }

    /// Bubble location for the line at constant X with this index.
    pub fn x_bubble_at(&self, i: usize) -> BubbleLoc {
        self.x_bubble.get(i).copied().unwrap_or_default()
    }

    /// Bubble location for the line at constant Y with this index.
    pub fn y_bubble_at(&self, i: usize) -> BubbleLoc {
        self.y_bubble.get(i).copied().unwrap_or_default()
    }

    /// True when the grid is turned away from the global axes.
    pub fn is_rotated(&self) -> bool {
        self.rotation_deg.abs() > 1.0e-9
    }

    /// Grid-local coordinates to global coordinates.
    pub fn to_world(&self, local: (f64, f64)) -> (f64, f64) {
        let (sin, cos) = self.rotation_deg.to_radians().sin_cos();
        (self.origin.0 + local.0 * cos - local.1 * sin, self.origin.1 + local.0 * sin + local.1 * cos)
    }

    /// Global coordinates to grid-local coordinates.
    pub fn to_local(&self, world: (f64, f64)) -> (f64, f64) {
        let (sin, cos) = self.rotation_deg.to_radians().sin_cos();
        let (dx, dy) = (world.0 - self.origin.0, world.1 - self.origin.1);
        (dx * cos + dy * sin, -dx * sin + dy * cos)
    }

    /// Extent of the grid lines in grid-local coordinates: (min X, max X, min Y, max Y).
    pub fn local_extent(&self) -> (f64, f64, f64, f64) {
        let mut x0 = self.xs.iter().copied().fold(f64::INFINITY, f64::min);
        let mut x1 = self.xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let mut y0 = self.ys.iter().copied().fold(f64::INFINITY, f64::min);
        let mut y1 = self.ys.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        if let Some(cylindrical) = &self.cylindrical {
            let radius = cylindrical.outer_radius();
            x0 = x0.min(cylindrical.center.0 - radius);
            x1 = x1.max(cylindrical.center.0 + radius);
            y0 = y0.min(cylindrical.center.1 - radius);
            y1 = y1.max(cylindrical.center.1 + radius);
        }
        if !x0.is_finite() || !x1.is_finite() || !y0.is_finite() || !y1.is_finite() {
            return (0.0, 0.0, 0.0, 0.0);
        }
        (x0, x1, y0, y1)
    }

    /// The grid's four corners in global coordinates, counter-clockwise from (min X, min Y).
    pub fn corners(&self) -> [(f64, f64); 4] {
        let (x0, x1, y0, y1) = self.local_extent();
        [(x0, y0), (x1, y0), (x1, y1), (x0, y1)].map(|corner| self.to_world(corner))
    }

    /// Global bounding box of the grid, after origin and rotation.
    pub fn bounds(&self) -> (f64, f64, f64, f64) {
        let corners = self.corners();
        let x0 = corners.iter().map(|corner| corner.0).fold(f64::INFINITY, f64::min);
        let x1 = corners.iter().map(|corner| corner.0).fold(f64::NEG_INFINITY, f64::max);
        let y0 = corners.iter().map(|corner| corner.1).fold(f64::INFINITY, f64::min);
        let y1 = corners.iter().map(|corner| corner.1).fold(f64::NEG_INFINITY, f64::max);
        if !x0.is_finite() || !x1.is_finite() || !y0.is_finite() || !y1.is_finite() {
            return (0.0, 0.0, 0.0, 0.0);
        }
        (x0, x1, y0, y1)
    }

    /// Plan segments for every grid line, split at each crossing.
    pub fn segments(&self) -> Vec<((f64, f64), (f64, f64))> {
        self.segments_filtered(true, true, true, true)
    }

    /// Segments for the selected grid-line families only.
    /// `x_lines` are lines at constant X, `y_lines` lines at constant Y;
    /// for cylindrical grids `rings` are circumferential chords and `radials`
    /// are the spokes from the centre outwards.
    pub fn segments_filtered(&self, x_lines: bool, y_lines: bool, rings: bool, radials: bool) -> Vec<((f64, f64), (f64, f64))> {
        let mut segments = Vec::new();
        if let Some(cylindrical) = &self.cylindrical {
            if radials {
                let mut radii = vec![0.0];
                radii.extend_from_slice(&cylindrical.radii);
                for angle in &cylindrical.angles_deg {
                    let (sin, cos) = angle.to_radians().sin_cos();
                    for pair in radii.windows(2) {
                        segments.push((
                            (cylindrical.center.0 + pair[0] * cos, cylindrical.center.1 + pair[0] * sin),
                            (cylindrical.center.0 + pair[1] * cos, cylindrical.center.1 + pair[1] * sin),
                        ));
                    }
                }
            }
            if rings && cylindrical.angles_deg.len() > 1 {
                for radius in &cylindrical.radii {
                    for index in 0..cylindrical.angles_deg.len() {
                        let first = cylindrical.angles_deg[index].to_radians();
                        let second = cylindrical.angles_deg[(index + 1) % cylindrical.angles_deg.len()].to_radians();
                        segments.push((
                            (cylindrical.center.0 + radius * first.cos(), cylindrical.center.1 + radius * first.sin()),
                            (cylindrical.center.0 + radius * second.cos(), cylindrical.center.1 + radius * second.sin()),
                        ));
                    }
                }
            }
        } else {
            if y_lines {
                for &y in &self.ys {
                    for pair in self.xs.windows(2) {
                        segments.push(((pair[0], y), (pair[1], y)));
                    }
                }
            }
            if x_lines {
                for &x in &self.xs {
                    for pair in self.ys.windows(2) {
                        segments.push(((x, pair[0]), (x, pair[1])));
                    }
                }
            }
        }
        // Segments are built in grid-local coordinates and placed through the
        // grid origin and rotation.
        segments.into_iter().map(|(a, b)| (self.to_world(a), self.to_world(b))).collect()
    }

    /// Grid lines running in Y are labelled with numbers (numeric) unless customised.
    pub fn label_x(&self, i: usize) -> String {
        self.x_labels.get(i).filter(|label| !label.trim().is_empty()).cloned().unwrap_or_else(|| format!("{}", i + 1))
    }

    /// Grid lines running in X are labelled A, B, C… unless customised.
    pub fn label_y(&self, i: usize) -> String {
        self.y_labels.get(i).filter(|label| !label.trim().is_empty()).cloned().unwrap_or_else(|| default_axis_label(i))
    }
}

/// Spreadsheet-style label for a line index: A, B, …, Z, AA, AB…
pub fn default_axis_label(index: usize) -> String {
    let mut label = String::new();
    let mut value = index as u32;
    loop {
        label.insert(0, (b'A' + (value % 26) as u8) as char);
        if value < 26 {
            break;
        }
        value = value / 26 - 1;
    }
    label
}

/// Orbit camera for the 3D viewport. Matches free-orbit behaviour.
#[derive(Clone, Copy, Debug)]
pub struct Camera3D {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f64,
    pub target: [f64; 3],
    pub initialized: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ViewPreset {
    Iso,
    Front,
    Side,
    Plan,
}

impl Default for Camera3D {
    fn default() -> Self {
        Self { yaw: -1.0, pitch: 0.55, distance: 42.0, target: [10.0, 7.5, 3.0], initialized: false }
    }
}

impl Camera3D {
    pub fn apply_preset(&mut self, preset: ViewPreset) {
        match preset {
            ViewPreset::Iso => {
                self.yaw = -1.0;
                self.pitch = 0.55;
            }
            ViewPreset::Front => {
                self.yaw = -std::f32::consts::FRAC_PI_2;
                self.pitch = 0.03;
            }
            ViewPreset::Side => {
                self.yaw = 0.0;
                self.pitch = 0.03;
            }
            ViewPreset::Plan => {
                self.yaw = -std::f32::consts::FRAC_PI_2;
                self.pitch = std::f32::consts::FRAC_PI_2 - 0.001;
            }
        }
    }

    pub fn eye(&self) -> [f64; 3] {
        let cp = self.pitch.cos() as f64;
        let sp = self.pitch.sin() as f64;
        let cy = self.yaw.cos() as f64;
        let sy = self.yaw.sin() as f64;
        [self.target[0] + self.distance * cp * cy, self.target[1] + self.distance * cp * sy, self.target[2] + self.distance * sp]
    }

    /// Camera right/up/forward unit vectors.
    pub fn basis(&self) -> ([f64; 3], [f64; 3], [f64; 3]) {
        let eye = self.eye();
        let forward = normalize([self.target[0] - eye[0], self.target[1] - eye[1], self.target[2] - eye[2]]);
        let world_up = if forward[2].abs() > 0.99 { [0.0, 1.0, 0.0] } else { [0.0, 0.0, 1.0] };
        let right = normalize(cross(forward, world_up));
        let up = cross(right, forward);
        (right, up, forward)
    }
}

struct Projector {
    eye: [f64; 3],
    right: [f64; 3],
    up: [f64; 3],
    forward: [f64; 3],
    focal: f64,
    center: egui::Pos2,
}

impl Projector {
    fn new(camera: &Camera3D, rect: egui::Rect) -> Self {
        let (right, up, forward) = camera.basis();
        Self { eye: camera.eye(), right, up, forward, focal: rect.height().max(1.0) as f64 * 1.15, center: rect.center() }
    }

    fn project(&self, p: [f64; 3]) -> Option<(egui::Pos2, f64)> {
        let rel = [p[0] - self.eye[0], p[1] - self.eye[1], p[2] - self.eye[2]];
        let cz = dot(rel, self.forward);
        if cz <= 0.05 {
            return None;
        }
        let cx = dot(rel, self.right);
        let cy = dot(rel, self.up);
        Some((egui::pos2(self.center.x + (cx / cz * self.focal) as f32, self.center.y - (cy / cz * self.focal) as f32), cz))
    }

    fn screen_dir(&self, direction: [f64; 3]) -> egui::Vec2 {
        let x = dot(direction, self.right) as f32;
        let y = dot(direction, self.up) as f32;
        egui::vec2(x, -y).normalized()
    }
}

impl OpenStructApp {
    // =====================================================================
    // Plan view
    // =====================================================================

    pub fn draw_canvas(&mut self, ui: &mut egui::Ui) {
        let available = ui.available_size();
        let (rect, response) = ui.allocate_exact_size(available, egui::Sense::click_and_drag());
        self.last_canvas_rect = Some(rect);
        let painter = ui.painter().with_clip_rect(rect);
        let pal = theme::palette(self.dark);
        painter.rect_filled(rect, egui::CornerRadius::same(0), pal.canvas);
        // The level shown in plan is washed with its Story Manager colour.
        let level_color = self.active_story_color();
        painter.rect_filled(rect, egui::CornerRadius::same(0), with_alpha(level_color, 20));

        if self.pending_fit.take().is_some() {
            self.fit_view(rect);
        }

        let hover = ui.input(|i| i.pointer.hover_pos()).filter(|point| rect.contains(*point));
        if let Some(point) = hover {
            self.mouse_world = self.view.to_world(rect, point);
        }

        // Zoom (wheel, towards the cursor).
        let scroll_y = ui.input(|i| i.smooth_scroll_delta.y);
        if response.hovered() && scroll_y.abs() > 0.0 {
            if let Some(point) = hover {
                let before = self.view.to_world(rect, point);
                let factor = (scroll_y / 240.0).exp();
                self.view.scale = (self.view.scale * factor).clamp(3.0, 8000.0);
                let after = self.view.to_world(rect, point);
                self.view.center.0 += before.0 - after.0;
                self.view.center.1 += before.1 - after.1;
            }
        }

        // Pan (middle drag).
        if response.dragged_by(egui::PointerButton::Middle) {
            let delta = response.drag_delta();
            self.view.center.0 -= (delta.x / self.view.scale) as f64;
            self.view.center.1 += (delta.y / self.view.scale) as f64;
        }

        // Rubber-band window selection on the active story.
        let primary_down = ui.input(|i| i.pointer.primary_down());
        if self.tool == Tool::Select && response.drag_started() && primary_down {
            self.drag_select_start = hover;
        }
        if self.tool == Tool::Select && response.drag_stopped() {
            let end = hover.or_else(|| ui.input(|i| i.pointer.interact_pos()));
            if let (Some(start), Some(end)) = (self.drag_select_start, end) {
                let (x0, y0) = self.view.to_world(rect, start);
                let (x1, y1) = self.view.to_world(rect, end);
                let (lx, hx) = (x0.min(x1), x0.max(x1));
                let (ly, hy) = (y0.min(y1), y0.max(y1));
                if (hx - lx) > 4.0 / self.view.scale as f64 && (hy - ly) > 4.0 / self.view.scale as f64 {
                    let inside = |x: f64, y: f64| x >= lx && x <= hx && y >= ly && y <= hy;
                    let mut nodes = HashSet::new();
                    let mut elements = HashSet::new();
                    let mut areas = HashSet::new();
                    for (id, node) in &self.model.nodes {
                        if let Some((u, v)) = self.node_uv(node) {
                            if inside(u, v) {
                                nodes.insert(*id);
                            }
                        }
                    }
                    for (id, element) in &self.model.elements {
                        if self.element_on_active_story(element) {
                            if let (Some(ni), Some(nj)) = (self.model.nodes.get(&element.node_i), self.model.nodes.get(&element.node_j)) {
                                if let (Some((ui, vi)), Some((uj, vj))) = (self.node_uv(ni), self.node_uv(nj)) {
                                    if inside(ui, vi) && inside(uj, vj) {
                                        elements.insert(*id);
                                    }
                                }
                            }
                        }
                    }
                    for (id, area) in &self.model.areas {
                        let base: Vec<usize> = if area.kind == AreaKind::Wall {
                            area.node_ids
                                .iter()
                                .copied()
                                .filter(|node_id| self.model.nodes.get(node_id).is_some_and(|node| self.node_on_active_story(node)))
                                .collect()
                        } else if self.area_on_active_story(area) {
                            area.node_ids.clone()
                        } else {
                            continue;
                        };
                        if !base.is_empty()
                            && base.iter().all(|node_id| self.model.nodes.get(node_id).and_then(|node| self.node_uv(node)).is_some_and(|(u, v)| inside(u, v)))
                        {
                            areas.insert(*id);
                        }
                    }
                    self.selected_nodes = nodes;
                    self.selected_elements = elements;
                    self.selected_areas = areas;
                    self.status_text = format!(
                        "Window selected {} joint(s), {} frame(s), {} area(s).",
                        self.selected_nodes.len(),
                        self.selected_elements.len(),
                        self.selected_areas.len()
                    );
                }
            }
            self.drag_select_start = None;
        }

        // Click interactions.
        let double_clicked = response.double_clicked();
        if double_clicked && self.tool == Tool::DrawArea && self.area_shape == AreaShape::Polygon {
            self.finish_polygon_area();
        }
        if response.clicked() {
            if let Some(point) = hover {
                let shift = ui.input(|i| i.modifiers.shift);
                let (wx, wy) = self.view.to_world(rect, point);
                let (snapped_x, snapped_y) = self.snap_to_grid(wx, wy, true);
                match self.tool {
                    Tool::Select => {
                        let hit_node = self.node_at(rect, point);
                        let hit_element = if hit_node.is_none() { self.element_at(rect, point) } else { None };
                        let hit_area = if hit_node.is_none() && hit_element.is_none() { self.area_at(rect, point) } else { None };
                        if let Some(id) = hit_node {
                            self.selected_elements.clear();
                            self.selected_areas.clear();
                            if !shift {
                                self.selected_nodes.clear();
                            }
                            self.selected_nodes.insert(id);
                            self.status_text = format!("Selected joint N{id} on {}.", self.work_plane_name());
                        } else if let Some(id) = hit_element {
                            self.selected_nodes.clear();
                            self.selected_areas.clear();
                            if !shift {
                                self.selected_elements.clear();
                            }
                            self.selected_elements.insert(id);
                            self.status_text = format!("Selected frame F{id} on {}.", self.work_plane_name());
                        } else if let Some(id) = hit_area {
                            self.selected_nodes.clear();
                            self.selected_elements.clear();
                            if !shift {
                                self.selected_areas.clear();
                            }
                            self.selected_areas.insert(id);
                            self.status_text = format!("Selected area A{id} on {}.", self.work_plane_name());
                        } else if !shift {
                            self.selected_nodes.clear();
                            self.selected_elements.clear();
                            self.selected_areas.clear();
                        }
                    }
                    Tool::DrawNode => {
                        self.push_undo();
                        let id = self.ensure_node_on_plane(snapped_x, snapped_y);
                        self.selected_nodes.clear();
                        self.selected_elements.clear();
                        self.selected_nodes.insert(id);
                        self.status_text = format!("Joint N{id} created at ({snapped_x:.3}, {snapped_y:.3}) m on {}.", self.work_plane_name());
                    }
                    Tool::DrawArea => {
                        if !self.work_plane_horizontal() {
                            self.status_text = "Areas require a horizontal work plane; select a story or a Z plane.".to_string();
                        } else if !(self.area_shape == AreaShape::Polygon && double_clicked) {
                            self.push_area_point((snapped_x, snapped_y));
                        }
                    }
                    Tool::DrawWall => {
                        let base = self.ensure_node_on_plane(snapped_x, snapped_y);
                        if let Some(previous) = self.draw_wall_start {
                            if previous != base {
                                self.add_wall_panel(previous, base);
                                self.draw_wall_start = Some(base);
                            } else {
                                self.draw_wall_start = None;
                                self.status_text = "Wall chain closed.".to_string();
                            }
                        } else {
                            self.selected_nodes.clear();
                            self.selected_elements.clear();
                            self.selected_areas.clear();
                            self.selected_nodes.insert(base);
                            self.draw_wall_start = Some(base);
                            self.status_text = format!("Wall chain started at N{base} on {}.", self.work_plane_name());
                        }
                    }
                    Tool::DrawFrame => {
                        let node_id = self.ensure_node_on_plane(snapped_x, snapped_y);
                        if let Some(start) = self.draw_frame_start {
                            if start != node_id {
                                let duplicate = self.model.elements.values().any(|element| {
                                    (element.node_i == start && element.node_j == node_id) || (element.node_i == node_id && element.node_j == start)
                                });
                                if !duplicate {
                                    self.push_undo();
                                    let material = self.default_material_id();
                                    let section = self.default_section_id();
                                    let next_id = self.model.element_counter + 1;
                                    let mut element = Element::frame(next_id, start, node_id, material, section);
                                    element.cardinal_point = self.cardinal_point;
                                    self.model.add_element(element);
                                    self.selected_nodes.clear();
                                    self.selected_elements.clear();
                                    self.selected_elements.insert(next_id);
                                    if self.quick_draw {
                                        self.quick_draw = false;
                                        self.tool = Tool::Select;
                                        self.draw_frame_start = None;
                                        self.status_text = format!("Quick Draw: frame F{next_id} created (N{start} → N{node_id}); returned to Select.");
                                    } else {
                                        self.draw_frame_start = Some(node_id);
                                        self.status_text = format!("Frame F{next_id} created (N{start} → N{node_id}) on {}.", self.work_plane_name());
                                    }
                                } else {
                                    self.draw_frame_start = Some(node_id);
                                    self.status_text = "Frame already exists; chain continues.".to_string();
                                }
                            } else {
                                self.draw_frame_start = None;
                                self.status_text = "Frame chain closed.".to_string();
                            }
                        } else {
                            self.selected_nodes.clear();
                            self.selected_elements.clear();
                            self.selected_nodes.insert(node_id);
                            self.draw_frame_start = Some(node_id);
                            self.status_text = format!("Frame chain started at N{node_id} on {}.", self.work_plane_name());
                        }
                    }
                }
            }
        }
        if response.secondary_clicked() {
            if self.tool == Tool::DrawFrame && self.draw_frame_start.take().is_some() {
                self.status_text = "Frame drawing chain cancelled.".to_string();
            }
            if self.tool == Tool::DrawWall && self.draw_wall_start.take().is_some() {
                self.status_text = "Wall chain ended.".to_string();
            }
            if self.tool == Tool::DrawArea {
                if self.area_shape == AreaShape::Polygon && self.area_points.len() >= 3 {
                    self.finish_polygon_area();
                } else if !self.area_points.is_empty() {
                    self.area_points.clear();
                    self.status_text = "Area drawing cancelled.".to_string();
                }
            }
        }

        if self.tool != Tool::Select {
            self.cursor_preview = hover.map(|point| {
                let (wx, wy) = self.view.to_world(rect, point);
                self.snap_to_grid(wx, wy, true)
            });
        } else {
            self.cursor_preview = None;
        }

        // Painting.
        let view = self.view;

        if self.show_grid && !self.work_plane_horizontal() {
            self.draw_elevation_guide_grid(&painter, rect);
        }

        if self.show_grid && self.work_plane_horizontal() {
            if let Some(grid) = self.grid.cylindrical.clone() {
                self.draw_cylindrical_grid(&painter, rect, &grid);
            }
        }

        if self.show_grid && self.work_plane_horizontal() && self.grid.cylindrical.is_none() {
            let stroke = egui::Stroke::new(1.0, with_alpha(level_color, 210));
            let bubble = (self.grid.bubble_size as f32 * view.scale).clamp(6.0, 40.0);
            if self.grid.is_rotated() {
                // A turned grid is drawn as its own rectangle of lines, with the
                // bubbles on both ends of each line.
                let (lx0, lx1, ly0, ly1) = self.grid.local_extent();
                for (index, &gx) in self.grid.xs.iter().enumerate() {
                    if !self.grid.x_line_visible(index) {
                        continue;
                    }
                    let (ax, ay) = self.grid.to_world((gx, ly0));
                    let (bx, by) = self.grid.to_world((gx, ly1));
                    let a = view.to_screen(rect, ax, ay);
                    let b = view.to_screen(rect, bx, by);
                    painter.line_segment([a, b], stroke);
                    let label = self.grid.label_x(index);
                    for end in bubble_ends(self.grid.x_bubble_at(index), a, b) {
                        painter.circle_filled(end, bubble, pal.panel_alt);
                        painter.circle_stroke(end, bubble, stroke);
                        painter.text(end, egui::Align2::CENTER_CENTER, label.as_str(), egui::FontId::proportional(9.0), pal.text_dim);
                    }
                }
                for (index, &gy) in self.grid.ys.iter().enumerate() {
                    if !self.grid.y_line_visible(index) {
                        continue;
                    }
                    let (ax, ay) = self.grid.to_world((lx0, gy));
                    let (bx, by) = self.grid.to_world((lx1, gy));
                    let a = view.to_screen(rect, ax, ay);
                    let b = view.to_screen(rect, bx, by);
                    painter.line_segment([a, b], stroke);
                    let label = self.grid.label_y(index);
                    for end in bubble_ends(self.grid.y_bubble_at(index), a, b) {
                        painter.circle_filled(end, bubble, pal.panel_alt);
                        painter.circle_stroke(end, bubble, stroke);
                        painter.text(end, egui::Align2::CENTER_CENTER, label.as_str(), egui::FontId::proportional(9.0), pal.text_dim);
                    }
                }
            } else {
                let (wx0, wy1) = view.to_world(rect, rect.left_top());
                let (wx1, wy0) = view.to_world(rect, rect.right_bottom());
                for (index, &gx) in self.grid.xs.iter().enumerate() {
                    if !self.grid.x_line_visible(index) {
                        continue;
                    }
                    let gx = gx + self.grid.origin.0;
                    if gx < wx0 || gx > wx1 {
                        continue;
                    }
                    let a = view.to_screen(rect, gx, wy1);
                    let b = view.to_screen(rect, gx, wy0);
                    painter.line_segment([a, b], stroke);
                    let label = self.grid.label_x(index);
                    let top = egui::pos2(a.x, rect.top() + 12.0);
                    let bottom = egui::pos2(a.x, rect.bottom() - 12.0);
                    for at in bubble_ends(self.grid.x_bubble_at(index), top, bottom) {
                        painter.circle_filled(at, bubble, pal.panel_alt);
                        painter.circle_stroke(at, bubble, stroke);
                        painter.text(at, egui::Align2::CENTER_CENTER, label.as_str(), egui::FontId::proportional(9.0), pal.text_dim);
                    }
                }
                for (index, &gy) in self.grid.ys.iter().enumerate() {
                    if !self.grid.y_line_visible(index) {
                        continue;
                    }
                    let gy = gy + self.grid.origin.1;
                    if gy < wy0 || gy > wy1 {
                        continue;
                    }
                    let a = view.to_screen(rect, wx0, gy);
                    let b = view.to_screen(rect, wx1, gy);
                    painter.line_segment([a, b], stroke);
                    let label = self.grid.label_y(index);
                    let start = egui::pos2(rect.left() + 12.0, a.y);
                    let end = egui::pos2(rect.right() - 12.0, a.y);
                    for at in bubble_ends(self.grid.y_bubble_at(index), start, end) {
                        painter.circle_filled(at, bubble, pal.panel_alt);
                        painter.circle_stroke(at, bubble, stroke);
                        painter.text(at, egui::Align2::CENTER_CENTER, label.as_str(), egui::FontId::proportional(9.0), pal.text_dim);
                    }
                }
            }
        }

        if self.show_dimensions && self.show_grid && self.work_plane_horizontal() && self.grid.cylindrical.is_none() && !self.grid.is_rotated() {
            self.draw_plan_dimensions(&painter, rect);
        }

        if self.show_ref_planes {
            self.draw_ref_planes_plan(&painter, rect);
        }

        // Slab areas on the work plane, behind frames and joints.
        if self.show_areas && self.work_plane_horizontal() {
            for (id, area) in &self.model.areas {
                if area.kind == AreaKind::Wall || !self.area_on_active_story(area) {
                    continue;
                }
                let points: Vec<egui::Pos2> = area
                    .polygon(&self.model)
                    .iter()
                    .map(|point| view.to_screen(rect, point[0], point[1]))
                    .collect();
                if points.len() < 3 {
                    continue;
                }
                let selected = self.selected_areas.contains(id);
                let fill = if selected { with_alpha(pal.selection, 55) } else { with_alpha(level_color, 30) };
                let stroke_color = if selected { pal.selection } else { with_alpha(level_color, 150) };
                painter.add(egui::Shape::convex_polygon(points, fill, egui::Stroke::new(if selected { 2.0 } else { 1.0 }, stroke_color)));
                if self.show_labels {
                    if let Some(centroid) = area.centroid(&self.model) {
                    painter.text(
                        view.to_screen(rect, centroid[0], centroid[1]),
                        egui::Align2::CENTER_CENTER,
                        format!("A{id}"),
                        egui::FontId::proportional(9.0),
                        pal.text_dim,
                    );
                    }
                }
            }
        }

        // Walls on the work plane: base polylines in plan, full panels in elevation.
        if self.show_areas {
        for (id, area) in &self.model.areas {
            if area.kind != AreaKind::Wall {
                continue;
            }
            let selected = self.selected_areas.contains(id);
            if self.work_plane_horizontal() {
                let base: Vec<egui::Pos2> =
                    area.node_ids.iter().filter_map(|node_id| self.model.nodes.get(node_id).and_then(|node| self.node_screen(&view, rect, node))).collect();
                if base.len() < 2 {
                    continue;
                }
                let stroke = egui::Stroke::new(if selected { 4.0 } else { 3.0 }, if selected { pal.selection } else { pal.frame });
                for pair in base.windows(2) {
                    painter.line_segment([pair[0], pair[1]], stroke);
                }
                if self.show_labels {
                    let midpoint = egui::pos2((base[0].x + base[1].x) / 2.0, (base[0].y + base[1].y) / 2.0);
                    painter.text(midpoint, egui::Align2::CENTER_BOTTOM, format!("W{id}"), egui::FontId::proportional(9.0), pal.text_dim);
                }
            } else {
                let panel: Vec<egui::Pos2> =
                    area.node_ids.iter().filter_map(|node_id| self.model.nodes.get(node_id).and_then(|node| self.node_screen(&view, rect, node))).collect();
                if panel.len() < 3 {
                    continue;
                }
                let fill = if selected { with_alpha(pal.selection, 45) } else { with_alpha(pal.frame, 18) };
                let stroke = egui::Stroke::new(if selected { 3.0 } else { 1.5 }, if selected { pal.selection } else { pal.frame });
                painter.add(egui::Shape::convex_polygon(panel.clone(), fill, stroke));
                if self.show_labels {
                    let label = panel.iter().fold(egui::Vec2::ZERO, |sum, point| sum + point.to_vec2()) / panel.len() as f32;
                    painter.text(label.to_pos2(), egui::Align2::CENTER_CENTER, format!("W{id}"), egui::FontId::proportional(9.0), pal.text_dim);
                }
            }
        }
        }

        // Frames on the work plane.
        if self.show_frames {
        for (id, element) in &self.model.elements {
            if !self.element_on_active_story(element) {
                continue;
            }
            if let (Some(ni), Some(nj)) = (self.model.nodes.get(&element.node_i), self.model.nodes.get(&element.node_j)) {
                let offset = self.frame_cardinal_offset(element, ni, nj);
                let a = self.offset_screen(&view, rect, ni, offset);
                let b = self.offset_screen(&view, rect, nj, offset);
                let selected = self.selected_elements.contains(id);
                let stroke = if selected { egui::Stroke::new(4.0, pal.selection) } else { egui::Stroke::new(2.5, pal.frame) };
                painter.line_segment([a, b], stroke);
                if element.cardinal_point.offsets() != (0.0, 0.0) && selected {
                    painter.circle_filled(a, 2.0, pal.selection);
                }
            }
        }
        }

        // Deformed shape on the active story.
        let scale = self.deformation_scale(rect);
        let results = self.results.take();
        if self.show_deformed {
            if let Some(results) = &results {
                let node_index: HashMap<usize, usize> = results.node_ids.iter().enumerate().map(|(i, &id)| (id, i)).collect();
                for &eid in &results.element_ids {
                    let Some(element) = self.model.elements.get(&eid) else {
                        continue;
                    };
                    if !self.element_on_active_story(element) {
                        continue;
                    }
                    let (Some(ni), Some(nj)) = (self.model.nodes.get(&element.node_i), self.model.nodes.get(&element.node_j)) else {
                        continue;
                    };
                    let (Some(&ii), Some(&jj)) = (node_index.get(&element.node_i), node_index.get(&element.node_j)) else {
                        continue;
                    };
                    let length = ((nj.x - ni.x).powi(2) + (nj.y - ni.y).powi(2) + (nj.z - ni.z).powi(2)).sqrt();
                    if length < 1e-9 {
                        continue;
                    }
                    let offset = self.frame_cardinal_offset(element, ni, nj);
                    let start = [ni.x + offset[0], ni.y + offset[1], ni.z + offset[2]];
                    let end = [nj.x + offset[0], nj.y + offset[1], nj.z + offset[2]];
                    let rotation = frame_rotation((end[0] - start[0]) / length, (end[1] - start[1]) / length, (end[2] - start[2]) / length);
                    let transform = offset_transformation(&rotation, offset, offset);
                    let mut displacement = DVector::<f64>::zeros(12);
                    for k in 0..6 {
                        displacement[k] = results.displacements[6 * ii + k];
                        displacement[k + 6] = results.displacements[6 * jj + k];
                    }
                    let local = transform * displacement;
                    let samples = if self.model.elements.len() > 2000 { 8 } else { 24 };
                    let points = deflected_points(start, end, &local, &rotation, samples, scale);
                    let plane = self.active_work_plane();
                    let screen: Vec<egui::Pos2> = points
                        .iter()
                        .map(|p| {
                            let (u, v) = plane.to_local(*p);
                            view.to_screen(rect, u, v)
                        })
                        .collect();
                    painter.add(egui::Shape::line(screen, egui::Stroke::new(2.0, pal.deformation)));
                }
            }
        }

        if self.show_loads && self.work_plane_horizontal() {
            self.load_arrows(&painter, rect);
        }

        // Nodes on the work plane.
        let node_count = self.model.nodes.len();
        if self.show_joints {
        for (id, node) in &self.model.nodes {
            let Some(point) = self.node_screen(&view, rect, node) else {
                continue;
            };
            let selected = self.selected_nodes.contains(id);
            if selected {
                painter.circle_stroke(point, 8.0, egui::Stroke::new(2.0, pal.selection));
            }
            painter.circle_filled(point, 4.0, pal.joint);
            if self.show_labels && (node_count <= 80 || selected) {
                painter.text(point + egui::vec2(7.0, -6.0), egui::Align2::LEFT_CENTER, format!("N{id}"), egui::FontId::proportional(9.0), pal.text_dim);
            }
            if self.show_supports && node.restrained[..3].iter().any(|&r| r) {
                let a = point + egui::vec2(-6.0, 9.0);
                let b = point + egui::vec2(6.0, 9.0);
                painter.line_segment([point, a], egui::Stroke::new(1.4, pal.ok));
                painter.line_segment([point, b], egui::Stroke::new(1.4, pal.ok));
                painter.line_segment([a, b], egui::Stroke::new(1.6, pal.ok));
            }
        }
        }

        // Draw-tool previews.
        if self.tool != Tool::Select {
            if let Some((x, y)) = self.cursor_preview {
                let point = view.to_screen(rect, x, y);
                painter.circle_stroke(point, 6.0, egui::Stroke::new(1.5, pal.text));
                if let Some(start) = self.draw_frame_start {
                    if let Some(a) = self.model.nodes.get(&start).and_then(|node| self.node_screen(&view, rect, node)) {
                        painter.extend(egui::Shape::dashed_line(&[a, point], egui::Stroke::new(1.6, pal.accent), 6.0, 4.0));
                    }
                }
                if let Some(base) = self.draw_wall_start {
                    if let Some(a) = self.model.nodes.get(&base).and_then(|node| self.node_screen(&view, rect, node)) {
                        painter.extend(egui::Shape::dashed_line(&[a, point], egui::Stroke::new(2.0, pal.accent), 6.0, 4.0));
                        painter.text(
                            point + egui::vec2(7.0, -6.0),
                            egui::Align2::LEFT_BOTTOM,
                            format!("h {:.2} m", self.wall_height),
                            egui::FontId::proportional(9.0),
                            pal.text_dim,
                        );
                    }
                }
                if self.tool == Tool::DrawArea && self.work_plane_horizontal() && !self.area_points.is_empty() {
                    let stroke = egui::Stroke::new(1.4, pal.accent);
                    let anchor = self.area_points[0];
                    let anchor_screen = view.to_screen(rect, anchor.0, anchor.1);
                    match self.area_shape {
                        AreaShape::Rectangle => {
                            painter.rect_stroke(egui::Rect::from_two_pos(anchor_screen, point), egui::CornerRadius::same(0), stroke, egui::StrokeKind::Inside);
                        }
                        AreaShape::Polygon => {
                            let mut screen: Vec<egui::Pos2> = self.area_points.iter().map(|(x, y)| view.to_screen(rect, *x, *y)).collect();
                            screen.push(point);
                            painter.extend(egui::Shape::dashed_line(&screen, stroke, 6.0, 4.0));
                        }
                        AreaShape::Circle | AreaShape::Ellipse => {
                            let (wx, wy) = self.cursor_preview.unwrap_or((x, y));
                            let (rx, ry) = if self.area_shape == AreaShape::Circle {
                                let radius = ((wx - anchor.0).powi(2) + (wy - anchor.1).powi(2)).sqrt();
                                (radius, radius)
                            } else {
                                ((wx - anchor.0).abs(), (wy - anchor.1).abs())
                            };
                            let mut screen: Vec<egui::Pos2> = shapes::ellipse(anchor, rx, ry, 32).iter().map(|(x, y)| view.to_screen(rect, *x, *y)).collect();
                            if let Some(first) = screen.first().copied() {
                                screen.push(first);
                            }
                            painter.extend(egui::Shape::dashed_line(&screen, stroke, 6.0, 4.0));
                        }
                        AreaShape::Arc => {
                            if self.area_points.len() == 1 {
                                let radius = ((x - anchor.0).powi(2) + (y - anchor.1).powi(2)).sqrt();
                                let mut screen: Vec<egui::Pos2> =
                                    shapes::ellipse(anchor, radius, radius, 32).iter().map(|(cx, cy)| view.to_screen(rect, *cx, *cy)).collect();
                                if let Some(first) = screen.first().copied() {
                                    screen.push(first);
                                }
                                painter.extend(egui::Shape::dashed_line(&screen, stroke, 6.0, 4.0));
                            } else {
                                let start = self.area_points[1];
                                let radius = ((start.0 - anchor.0).powi(2) + (start.1 - anchor.1).powi(2)).sqrt();
                                let start_angle = (start.1 - anchor.1).atan2(start.0 - anchor.0);
                                let end_angle = (y - anchor.1).atan2(x - anchor.0);
                                let sweep = (end_angle - start_angle).rem_euclid(std::f64::consts::TAU).max(1e-3);
                                let screen: Vec<egui::Pos2> = shapes::arc(anchor, radius, radius, start_angle, start_angle + sweep, 48)
                                    .iter()
                                    .map(|(cx, cy)| view.to_screen(rect, *cx, *cy))
                                    .collect();
                                painter.extend(egui::Shape::dashed_line(&screen, stroke, 6.0, 4.0));
                            }
                        }
                    }
                }
            }
        }

        if let (Some(start), Some(current)) = (self.drag_select_start, hover) {
            painter.rect_filled(egui::Rect::from_two_pos(start, current), egui::CornerRadius::same(0), pal.accent_soft);
        }

        let hint = match self.tool {
            Tool::Select => "Select: click joints/frames/areas, drag a window. Middle-drag pans, wheel zooms.",
            Tool::DrawNode => "Draw Joints: click to add a joint on the active story (snaps to grid).",
            Tool::DrawFrame => "Draw Frames: click to start, click to end; right-click/Esc breaks the chain.",
            Tool::DrawArea => "Draw Areas: pick a shape in the toolbar, then click on the active story (double-click closes a polygon).",
            Tool::DrawWall => "Draw Walls: click base joints to chain vertical panels; right-click ends the chain.",
        };
        painter.text(rect.left_top() + egui::vec2(10.0, 8.0), egui::Align2::LEFT_TOP, hint, egui::FontId::proportional(11.0), pal.text_dim);
        let plane = self.active_work_plane();
        let work_label = if plane.is_horizontal() {
            format!("Plan View · {} @ {:.2} m", self.work_plane_name(), plane.offset)
        } else {
            let (u, v) = plane.axis_labels();
            format!("Elevation · {} ({} = {:.2} m) · {u}/{v}", self.work_plane_name(), plane.axis.label(), plane.offset)
        };
        painter.text(rect.right_top() + egui::vec2(-10.0, 8.0), egui::Align2::RIGHT_TOP, work_label, egui::FontId::proportional(11.0), pal.text_dim);

        self.results = results;
    }

    // =====================================================================
    // 3D view
    // =====================================================================

    pub fn draw_view3d(&mut self, ui: &mut egui::Ui) {
        let available = ui.available_size();
        let (rect, response) = ui.allocate_exact_size(available, egui::Sense::click_and_drag());
        let painter = ui.painter().with_clip_rect(rect);
        let pal = theme::palette(self.dark);
        painter.rect_filled(rect, egui::CornerRadius::same(0), pal.canvas);

        if self.pending_fit.take().is_some() || !self.camera.initialized {
            self.fit_view_3d(rect);
        }

        let hover = ui.input(|i| i.pointer.hover_pos()).filter(|point| rect.contains(*point));
        if let Some(point) = hover {
            self.mouse_world = (point.x as f64, point.y as f64);
        }
        let dt_ms = ui.input(|i| i.stable_dt).clamp(0.001, 0.25) * 1000.0;
        self.frame_ms = if self.frame_ms <= 0.0 { dt_ms } else { self.frame_ms * 0.9 + dt_ms * 0.1 };

        // Orbit (primary drag), pan (shift+primary or middle drag), zoom (wheel).
        let shift = ui.input(|i| i.modifiers.shift);
        if response.dragged_by(egui::PointerButton::Primary) && !shift {
            let delta = response.drag_delta();
            self.camera.yaw += delta.x * 0.008;
            self.camera.pitch = (self.camera.pitch - delta.y * 0.008).clamp(-1.5, 1.5);
        }
        if (response.dragged_by(egui::PointerButton::Primary) && shift) || response.dragged_by(egui::PointerButton::Middle) {
            let delta = response.drag_delta();
            let (right, up, _) = self.camera.basis();
            let k = self.camera.distance / rect.height().max(1.0) as f64 * 1.2;
            for axis in 0..3 {
                self.camera.target[axis] += -right[axis] * delta.x as f64 * k + up[axis] * delta.y as f64 * k;
            }
        }
        let scroll_y = ui.input(|i| i.smooth_scroll_delta.y);
        if response.hovered() && scroll_y.abs() > 0.0 {
            self.camera.distance = (self.camera.distance * (-scroll_y as f64 / 300.0).exp()).clamp(0.5, 5000.0);
        }

        if response.clicked() {
            if let Some(point) = hover {
                self.pick_3d(rect, point, ui.input(|i| i.modifiers.shift));
            }
        }

        // Optional wgpu shaded sections, underneath the painter wireframe.
        if self.wgpu_ready && self.shade_3d {
            let story = if self.model.elements.len() > 4_000 && !self.clip_3d {
                Some(self.active_elevation())
            } else {
                None
            };
            let vertices = crate::wgpu3d::section_vertices(&self.model, story, self.camera.eye());
            if !vertices.is_empty() {
                let clip = self.clip_3d.then(|| self.active_elevation());
                let uniforms = crate::wgpu3d::camera_uniforms(&self.camera, rect.aspect_ratio(), clip, pal.frame);
                painter.add(eframe::egui_wgpu::Callback::new_paint_callback(
                    rect,
                    crate::wgpu3d::SectionsCallback { vertices, uniforms },
                ));
            }
        }

        // ----- painting -----------------------------------------------------
        let projector = Projector::new(&self.camera, rect);
        let (x0, x1, y0, y1) = self.grid.bounds();
        let active_story = self.active_story;

        // Grids and story outlines.
        for (index, story) in self.model.stories.iter().enumerate() {
            let z = story.elevation;
            let is_active = index == active_story;
            // Each level keeps the colour set in the Story Manager.
            let level_color = self.story_color(index);
            let color = if is_active { level_color } else { with_alpha(level_color, 150) };
            let label_color = if is_active { level_color } else { with_alpha(level_color, 190) };
            if let Some(cylindrical) = &self.grid.cylindrical {
                let outer = cylindrical.outer_radius();
                let circle_color = color;
                let circle_width = if is_active && self.show_grid { 1.6 } else { 1.0 };
                for (ring_index, radius) in cylindrical.radii.iter().enumerate() {
                    let mut points = Vec::new();
                    for step in 0..=64 {
                        let angle = std::f64::consts::TAU * step as f64 / 64.0;
                        let (px, py) = self
                            .grid
                            .to_world((cylindrical.center.0 + radius * angle.cos(), cylindrical.center.1 + radius * angle.sin()));
                        if let Some((point, _)) = projector.project([px, py, z]) {
                            points.push(point);
                        }
                    }
                    if points.len() > 1 {
                        painter.add(egui::Shape::line(points, egui::Stroke::new(circle_width, circle_color)));
                    }
                    if is_active {
                        let (rx, ry) = self.grid.to_world((cylindrical.center.0 + radius, cylindrical.center.1));
                        if let Some((label, _)) = projector.project([rx, ry, z]) {
                            painter.text(
                                label + egui::vec2(4.0, -4.0),
                                egui::Align2::LEFT_BOTTOM,
                                cylindrical.ring_label(ring_index),
                                egui::FontId::proportional(10.0),
                                pal.text_dim,
                            );
                        }
                    }
                }
                if is_active && self.show_grid {
                    for (angle_index, angle) in cylindrical.angles_deg.iter().enumerate() {
                        let radians = angle.to_radians();
                        let (cx, cy) = self.grid.to_world((cylindrical.center.0, cylindrical.center.1));
                        let (rx, ry) = self.grid.to_world((cylindrical.center.0 + outer * radians.cos(), cylindrical.center.1 + outer * radians.sin()));
                        let centre = [cx, cy, z];
                        let rim = [rx, ry, z];
                        if let (Some((a, _)), Some((b, _))) = (projector.project(centre), projector.project(rim)) {
                            painter.line_segment([a, b], egui::Stroke::new(1.0, color));
                            painter.text(
                                b + egui::vec2(4.0, -4.0),
                                egui::Align2::LEFT_BOTTOM,
                                cylindrical.radial_label(angle_index),
                                egui::FontId::proportional(10.0),
                                pal.text_dim,
                            );
                        }
                    }
                }
                let (ox, oy) = self.grid.to_world((cylindrical.center.0 + outer, cylindrical.center.1));
                if let Some((label_pos, _)) = projector.project([ox, oy, z]) {
                    painter.text(
                        label_pos + egui::vec2(-8.0, -2.0),
                        egui::Align2::RIGHT_BOTTOM,
                        format!("{}  {:.2}", story.name, z),
                        egui::FontId::proportional(10.0),
                        label_color,
                    );
                }
            } else {
                if is_active && self.show_grid {
                    let (lx0, lx1, ly0, ly1) = self.grid.local_extent();
                    for (index, &gx) in self.grid.xs.iter().enumerate() {
                        if !self.grid.x_line_visible(index) {
                            continue;
                        }
                        let (ax, ay) = self.grid.to_world((gx, ly0));
                        let (bx, by) = self.grid.to_world((gx, ly1));
                        if let (Some((a, _)), Some((b, _))) = (projector.project([ax, ay, z]), projector.project([bx, by, z])) {
                            painter.line_segment([a, b], egui::Stroke::new(1.0, color));
                        }
                    }
                    for (index, &gy) in self.grid.ys.iter().enumerate() {
                        if !self.grid.y_line_visible(index) {
                            continue;
                        }
                        let (ax, ay) = self.grid.to_world((lx0, gy));
                        let (bx, by) = self.grid.to_world((lx1, gy));
                        if let (Some((a, _)), Some((b, _))) = (projector.project([ax, ay, z]), projector.project([bx, by, z])) {
                            painter.line_segment([a, b], egui::Stroke::new(1.0, color));
                        }
                    }
                }
                let plan = self.grid.corners();
                let corners = [[plan[0].0, plan[0].1, z], [plan[1].0, plan[1].1, z], [plan[2].0, plan[2].1, z], [plan[3].0, plan[3].1, z]];
                for pair in [[0, 1], [1, 2], [2, 3], [3, 0]] {
                    if let (Some((a, _)), Some((b, _))) = (projector.project(corners[pair[0]]), projector.project(corners[pair[1]])) {
                        painter.line_segment([a, b], egui::Stroke::new(if is_active { 1.6 } else { 1.0 }, color));
                    }
                }
                if let Some((label_pos, _)) = projector.project([x0, y0, z]) {
                    painter.text(
                        label_pos + egui::vec2(-8.0, -2.0),
                        egui::Align2::RIGHT_BOTTOM,
                        format!("{}  {:.2}", story.name, z),
                        egui::FontId::proportional(10.0),
                        label_color,
                    );
                }
            }
        }

        // Vertical story lines at the grid boundary.
        let top_elevation = self.model.stories.iter().map(|story| story.elevation).fold(0.0_f64, f64::max);
        if let Some(cylindrical) = &self.grid.cylindrical {
            let outer = cylindrical.outer_radius();
            for (dx, dy) in [(1.0, 0.0), (0.0, 1.0), (-1.0, 0.0), (0.0, -1.0)] {
                let point = [cylindrical.center.0 + outer * dx, cylindrical.center.1 + outer * dy, 0.0];
                if let (Some((a, _)), Some((b, _))) = (projector.project(point), projector.project([point[0], point[1], top_elevation])) {
                    painter.line_segment([a, b], egui::Stroke::new(1.0, with_alpha(pal.grid, 60)));
                }
            }
        } else {
            for corner in [self.grid.corners()[0], self.grid.corners()[2]] {
                if let (Some((a, _)), Some((b, _))) = (projector.project([corner.0, corner.1, 0.0]), projector.project([corner.0, corner.1, top_elevation]))
                {
                    painter.line_segment([a, b], egui::Stroke::new(1.0, with_alpha(pal.grid, 60)));
                }
            }
        }

        let bottom_elevation = self.model.stories.iter().map(|story| story.elevation).fold(0.0_f64, f64::min);
        if self.show_ref_planes {
            self.draw_ref_planes_3d(&painter, &projector, x0, x1, y0, y1, bottom_elevation, top_elevation.max(bottom_elevation + 3.0));
        }

        // Load symbols: distributed frame loads and area pressures.
        let load_scale = self.load_symbol_scale.clamp(0.3, 3.0);
        let load_font = egui::FontId::proportional(10.0 * load_scale.min(1.6));
        let load_color = egui::Color32::from_rgb(0xe6, 0xa0, 0x3c);
        if self.show_loads && self.show_frame_loads {
        for load in &self.model.frame_loads {
            let Some(element) = self.model.elements.get(&load.element_id) else {
                continue;
            };
            let (Some(start_node), Some(end_node)) = (self.model.nodes.get(&element.node_i), self.model.nodes.get(&element.node_j)) else {
                continue;
            };
            let (Some(a), Some(b)) = (
                projector.project([start_node.x, start_node.y, start_node.z]),
                projector.project([end_node.x, end_node.y, end_node.z]),
            ) else {
                continue;
            };
            let (a, b) = (a.0, b.0);
            if (b - a).length() < 1.0 {
                continue;
            }
            let along = (b - a).normalized();
            let normal = egui::vec2(-along.y, along.x);
            let tail = a + normal * (20.0 * load_scale);
            let head = b + normal * (20.0 * load_scale);
            painter.line_segment([tail, head], egui::Stroke::new(1.2, load_color));
            for step in 0..=4 {
                let base = a + (b - a) * (step as f32 / 4.0);
                painter.line_segment([base, base + normal * (20.0 * load_scale)], egui::Stroke::new(1.0, with_alpha(load_color, 160)));
            }
            let magnitude = (load.start[0].powi(2) + load.start[1].powi(2) + load.start[2].powi(2)).sqrt();
            if magnitude > 0.0 {
                let direction = match load.direction {
                    LoadDirection::Global => projector.screen_dir(load.start),
                    LoadDirection::Local => normal,
                };
                let base = a + (b - a) * 0.5 + normal * (20.0 * load_scale);
                let tip = base + direction * (14.0 * load_scale);
                painter.line_segment([base, tip], egui::Stroke::new(1.6, load_color));
                let side = egui::vec2(-direction.y, direction.x);
                painter.line_segment([tip, tip - direction * (6.0 * load_scale) + side * (4.0 * load_scale)], egui::Stroke::new(1.6, load_color));
                painter.line_segment([tip, tip - direction * (6.0 * load_scale) - side * (4.0 * load_scale)], egui::Stroke::new(1.6, load_color));
                painter.text(
                    tip + direction * (8.0 * load_scale),
                    egui::Align2::CENTER_CENTER,
                    format!("{:.2} {}", self.units.dist_to_display(magnitude), self.units.dist_label()),
                    load_font.clone(),
                    load_color,
                );
            }
        }
        }
        if self.show_loads && self.show_area_loads {
        for load in &self.model.area_loads {
            let Some(area) = self.model.areas.get(&load.area_id) else {
                continue;
            };
            let Some(center) = area.centroid(&self.model) else {
                continue;
            };
            let Some((point, _)) = projector.project(center) else {
                continue;
            };
            painter.circle_stroke(point, 7.0 * load_scale, egui::Stroke::new(1.4, load_color));
            painter.line_segment(
                [point + egui::vec2(0.0, -7.0 * load_scale), point + egui::vec2(0.0, -14.0 * load_scale)],
                egui::Stroke::new(1.2, load_color),
            );
            painter.text(
                point + egui::vec2(4.0 * load_scale, -16.0 * load_scale),
                egui::Align2::LEFT_BOTTOM,
                format!("{:.2} {}", self.units.pressure_to_display(load.pressure), self.units.pressure_label()),
                load_font.clone(),
                load_color,
            );
        }
        }

        // Areas, painted back-to-front below the frame wireframe.
        let mut quads: Vec<(f64, Vec<egui::Pos2>, bool, egui::Color32)> = Vec::new();
        if self.show_areas {
        for (id, area) in &self.model.areas {
            let mut points = Vec::new();
            let mut depth = 0.0;
            let mut complete = true;
            let polygon = area.polygon(&self.model);
            // Areas are painted in the colour of the level they sit on.
            let level_color = polygon
                .first()
                .and_then(|point| self.model.story_index_at(point[2], 1e-6))
                .map(|index| self.story_color(index))
                .unwrap_or(pal.frame);
            for point in polygon {
                match projector.project(point) {
                    Some((screen, d)) => {
                        points.push(screen);
                        depth += d;
                    }
                    None => {
                        complete = false;
                        break;
                    }
                }
            }
            if complete && points.len() >= 3 {
                quads.push((depth / points.len() as f64, points, self.selected_areas.contains(id), level_color));
            }
        }
        }
        quads.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        for (_, points, selected, level_color) in quads {
            let fill = if selected { with_alpha(pal.selection, 70) } else { with_alpha(level_color, 38) };
            let stroke_color = if selected { pal.selection } else { with_alpha(level_color, 150) };
            painter.add(egui::Shape::convex_polygon(points, fill, egui::Stroke::new(if selected { 2.0 } else { 1.0 }, stroke_color)));
        }

        // Frames, depth-sorted.
        let mut segments: Vec<(f64, egui::Pos2, egui::Pos2, bool, bool)> = Vec::new();
        if self.show_frames {
        for (id, element) in &self.model.elements {
            let (Some(ni), Some(nj)) = (self.model.nodes.get(&element.node_i), self.model.nodes.get(&element.node_j)) else {
                continue;
            };
            let offset = self.frame_cardinal_offset(element, ni, nj);
            if let (Some((a, da)), Some((b, db))) = (
                projector.project([ni.x + offset[0], ni.y + offset[1], ni.z + offset[2]]),
                projector.project([nj.x + offset[0], nj.y + offset[1], nj.z + offset[2]]),
            ) {
                let active = self.element_on_active_story(element);
                segments.push(((da + db) * 0.5, a, b, self.selected_elements.contains(id), active));
            }
        }
        }
        segments.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        for (_, a, b, selected, active) in segments {
            let stroke = if selected {
                egui::Stroke::new(4.0, pal.selection)
            } else if active {
                egui::Stroke::new(2.4, pal.frame)
            } else {
                egui::Stroke::new(1.8, with_alpha(pal.frame, 120))
            };
            painter.line_segment([a, b], stroke);
        }

        // Deformed shape (scaled), all stories.
        if self.show_deformed {
            if let Some(results) = &self.results {
                let scale = if self.defl_auto { self.auto_deformation_scale_3d(rect) } else { self.defl_ampl };
                let node_index: HashMap<usize, usize> = results.node_ids.iter().enumerate().map(|(i, &id)| (id, i)).collect();
                for &eid in &results.element_ids {
                    let Some(element) = self.model.elements.get(&eid) else {
                        continue;
                    };
                    let (Some(ni), Some(nj)) = (self.model.nodes.get(&element.node_i), self.model.nodes.get(&element.node_j)) else {
                        continue;
                    };
                    let (Some(&ii), Some(&jj)) = (node_index.get(&element.node_i), node_index.get(&element.node_j)) else {
                        continue;
                    };
                    let length = ((nj.x - ni.x).powi(2) + (nj.y - ni.y).powi(2) + (nj.z - ni.z).powi(2)).sqrt();
                    if length < 1e-9 {
                        continue;
                    }
                    let offset = self.frame_cardinal_offset(element, ni, nj);
                    let start = [ni.x + offset[0], ni.y + offset[1], ni.z + offset[2]];
                    let end = [nj.x + offset[0], nj.y + offset[1], nj.z + offset[2]];
                    let rotation = frame_rotation((end[0] - start[0]) / length, (end[1] - start[1]) / length, (end[2] - start[2]) / length);
                    let transform = offset_transformation(&rotation, offset, offset);
                    let mut displacement = DVector::<f64>::zeros(12);
                    for k in 0..6 {
                        displacement[k] = results.displacements[6 * ii + k];
                        displacement[k + 6] = results.displacements[6 * jj + k];
                    }
                    let local = transform * displacement;
                    let samples = if self.model.elements.len() > 2000 { 8 } else { 24 };
                    let points = deflected_points(start, end, &local, &rotation, samples, scale);
                    let mut screen = Vec::new();
                    for p in &points {
                        if let Some((sp, _)) = projector.project(*p) {
                            screen.push(sp);
                        }
                    }
                    if screen.len() > 1 {
                        painter.add(egui::Shape::line(screen, egui::Stroke::new(2.0, pal.deformation)));
                    }
                }
            }
        }

        // Joints, supports and labels.
        let node_count = self.model.nodes.len();
        if self.show_joints {
        for (id, node) in &self.model.nodes {
            let Some((point, _)) = projector.project([node.x, node.y, node.z]) else {
                continue;
            };
            let selected = self.selected_nodes.contains(id);
            let active = self.node_on_active_story(node);
            if selected {
                painter.circle_stroke(point, 8.0, egui::Stroke::new(2.0, pal.selection));
            }
            painter.circle_filled(point, if active { 3.5 } else { 2.5 }, if active { pal.joint } else { with_alpha(pal.joint, 140) });
            if self.show_labels && (node_count <= 60 || selected) {
                painter.text(point + egui::vec2(7.0, -6.0), egui::Align2::LEFT_CENTER, format!("N{id}"), egui::FontId::proportional(9.0), pal.text_dim);
            }
            if self.show_supports && node.restrained[..3].iter().any(|&r| r) {
                painter.line_segment([point, point + egui::vec2(0.0, 10.0)], egui::Stroke::new(1.4, pal.ok));
                painter.line_segment([point + egui::vec2(-7.0, 10.0), point + egui::vec2(7.0, 10.0)], egui::Stroke::new(1.6, pal.ok));
            }
        }
        }

        // Axis triad.
        let origin = rect.left_bottom() + egui::vec2(42.0, -34.0);
        for (direction, label, color) in [
            ([1.0, 0.0, 0.0], "X", egui::Color32::from_rgb(0xe0, 0x6c, 0x6c)),
            ([0.0, 1.0, 0.0], "Y", egui::Color32::from_rgb(0x6c, 0xc0, 0x7a)),
            ([0.0, 0.0, 1.0], "Z", egui::Color32::from_rgb(0x6c, 0x9c, 0xe0)),
        ] {
            let dir = projector.screen_dir(direction);
            painter.line_segment([origin, origin + dir * 26.0], egui::Stroke::new(2.0, color));
            painter.text(origin + dir * 34.0, egui::Align2::CENTER_CENTER, label, egui::FontId::proportional(10.0), color);
        }

        painter.text(
            rect.left_top() + egui::vec2(10.0, 8.0),
            egui::Align2::LEFT_TOP,
            "Left-drag orbit · Shift-drag or middle-drag pan · Wheel zoom · Click to select",
            egui::FontId::proportional(11.0),
            pal.text_dim,
        );
        let backend = if self.wgpu_ready && self.shade_3d {
            if self.clip_3d { " · wgpu · clipped" } else { " · wgpu" }
        } else {
            ""
        };
        painter.text(
            rect.right_top() + egui::vec2(-10.0, 8.0),
            egui::Align2::RIGHT_TOP,
            format!(
                "3D View · {} stories · active: {} · {:.1} ms{}",
                self.model.stories.len(),
                self.active_story_name(),
                self.frame_ms,
                backend
            ),
            egui::FontId::proportional(11.0),
            pal.text_dim,
        );
    }

    pub fn deformation_scale(&self, rect: egui::Rect) -> f64 {
        let Some(results) = &self.results else {
            return 1.0;
        };
        if self.defl_auto {
            let mut max_displacement = 0.0_f64;
            for index in 0..results.node_ids.len() {
                let base = 6 * index;
                if base + 2 >= results.displacements.len() {
                    break;
                }
                let magnitude =
                    (results.displacements[base].powi(2) + results.displacements[base + 1].powi(2) + results.displacements[base + 2].powi(2)).sqrt();
                max_displacement = max_displacement.max(magnitude);
            }
            if max_displacement > 0.0 { (0.25 * rect.height() as f64 / self.view.scale as f64) / max_displacement } else { 1.0 }
        } else {
            self.defl_ampl
        }
    }

    fn auto_deformation_scale_3d(&self, _rect: egui::Rect) -> f64 {
        let Some(results) = &self.results else {
            return 1.0;
        };
        let mut max_displacement = 0.0_f64;
        for index in 0..results.node_ids.len() {
            let base = 6 * index;
            if base + 2 >= results.displacements.len() {
                break;
            }
            let magnitude = (results.displacements[base].powi(2) + results.displacements[base + 1].powi(2) + results.displacements[base + 2].powi(2)).sqrt();
            max_displacement = max_displacement.max(magnitude);
        }
        if max_displacement > 0.0 { (0.2 * self.camera.distance) / max_displacement } else { 1.0 }
    }

    pub fn fit_view(&mut self, rect: egui::Rect) {
        let plane = self.active_work_plane();
        let mut us: Vec<f64> = Vec::new();
        let mut vs: Vec<f64> = Vec::new();
        let mut any = false;
        for node in self.model.nodes.values() {
            if let Some((u, v)) = self.node_uv(node) {
                us.push(u);
                vs.push(v);
                any = true;
            }
        }
        let (grid_x0, grid_x1, grid_y0, grid_y1) = self.grid.bounds();
        if plane.is_horizontal() {
            us.extend([grid_x0, grid_x1]);
            vs.extend([grid_y0, grid_y1]);
        } else {
            match plane.axis {
                RefPlaneAxis::X => us.extend(self.grid.ys.iter().copied()),
                RefPlaneAxis::Y => us.extend(self.grid.xs.iter().copied()),
                RefPlaneAxis::Z => {}
            }
            vs.extend(self.model.stories.iter().map(|story| story.elevation));
            if vs.is_empty() {
                vs.extend([0.0, 3.0]);
            }
            if us.is_empty() {
                us.extend([0.0, 5.0]);
            }
        }
        if !any && us.is_empty() {
            if let Some(node) = self.model.nodes.values().next() {
                us.push(node.x);
                vs.push(node.y);
            }
        }
        if us.is_empty() || vs.is_empty() {
            return;
        }
        let (min_u, max_u) = (us.iter().cloned().fold(f64::INFINITY, f64::min), us.iter().cloned().fold(f64::NEG_INFINITY, f64::max));
        let (min_v, max_v) = (vs.iter().cloned().fold(f64::INFINITY, f64::min), vs.iter().cloned().fold(f64::NEG_INFINITY, f64::max));
        self.view.center = ((min_u + max_u) / 2.0, (min_v + max_v) / 2.0);
        let du = (max_u - min_u).max(1.0);
        let dv = (max_v - min_v).max(1.0);
        self.view.scale = (rect.width().min(rect.height()) / ((du.max(dv) * 1.3 + 2.0) as f32)).clamp(4.0, 8000.0);
    }

    pub fn fit_view_3d(&mut self, rect: egui::Rect) {
        let mut points: Vec<[f64; 3]> = Vec::new();
        for node in self.model.nodes.values() {
            points.push([node.x, node.y, node.z]);
        }
        let (x0, x1, y0, y1) = self.grid.bounds();
        for story in &self.model.stories {
            for corner in [[x0, y0], [x1, y0], [x1, y1], [x0, y1]] {
                points.push([corner[0], corner[1], story.elevation]);
            }
        }
        if points.is_empty() {
            return;
        }
        let mut min = [f64::INFINITY; 3];
        let mut max = [f64::NEG_INFINITY; 3];
        for p in &points {
            for axis in 0..3 {
                min[axis] = min[axis].min(p[axis]);
                max[axis] = max[axis].max(p[axis]);
            }
        }
        self.camera.target = [(min[0] + max[0]) / 2.0, (min[1] + max[1]) / 2.0, (min[2] + max[2]) / 2.0];
        let extent = (max[0] - min[0]).max(max[1] - min[1]).max(max[2] - min[2]).max(4.0);
        self.camera.distance = (extent * 1.9).clamp(4.0, 5000.0);
        self.camera.initialized = true;
        let _ = rect;
    }

    /// Exercise the real 3D picking path against projected benchmark joints.
    /// A probe is clicked 1 px away from the projected joint, inside the 12 px
    /// node hit radius, and succeeds only when that joint is selected.
    pub fn benchmark_picking(&mut self, probes: usize) -> (usize, usize, f64) {
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 720.0));
        if !self.camera.initialized {
            self.fit_view_3d(rect);
        }
        let projector = Projector::new(&self.camera, rect);
        let ids: Vec<usize> = self.model.nodes.keys().copied().take(probes).collect();
        let started = std::time::Instant::now();
        let mut hits = 0;
        let mut attempts = 0;
        for id in ids {
            let Some(node) = self.model.nodes.get(&id) else {
                continue;
            };
            let Some((point, _)) = projector.project([node.x, node.y, node.z]) else {
                continue;
            };
            if !rect.contains(point) {
                continue;
            }
            attempts += 1;
            let target = egui::pos2(point.x + 1.0, point.y + 1.0);
            self.pick_3d(rect, target, false);
            if self.selected_nodes.contains(&id) {
                hits += 1;
            }
        }
        (hits, attempts, started.elapsed().as_secs_f64() * 1000.0)
    }

    fn snap_to_grid(&self, x: f64, y: f64, snap: bool) -> (f64, f64) {
        if !snap {
            return (round_s(x), round_s(y));
        }
        let tolerance = 12.0 / self.view.scale as f64;
        if !self.work_plane_horizontal() {
            return self.snap_on_elevation(x, y, tolerance);
        }
        let mut best = None;
        let mut best_distance = f64::INFINITY;
        // Grid snapping happens in grid-local coordinates, so a turned grid snaps
        // to its own intersections rather than to the global axes.
        let (local_x, local_y) = self.grid.to_local((x, y));
        for &gx in &self.grid.xs {
            for &gy in &self.grid.ys {
                let distance = ((gx - local_x).powi(2) + (gy - local_y).powi(2)).sqrt();
                if distance < tolerance && distance < best_distance {
                    best_distance = distance;
                    best = Some(self.grid.to_world((gx, gy)));
                }
            }
        }
        for plane in &self.ref_planes {
            match plane.axis {
                RefPlaneAxis::X => {
                    let distance = (plane.offset - x).abs();
                    if distance < tolerance && distance < best_distance {
                        best_distance = distance;
                        best = Some((plane.offset, y));
                    }
                }
                RefPlaneAxis::Y => {
                    let distance = (plane.offset - y).abs();
                    if distance < tolerance && distance < best_distance {
                        best_distance = distance;
                        best = Some((x, plane.offset));
                    }
                }
                RefPlaneAxis::Z => {}
            }
        }
        if let Some(cylindrical) = &self.grid.cylindrical {
            for &radius in &cylindrical.radii {
                for &angle in &cylindrical.angles_deg {
                    let radians = angle.to_radians();
                    let local = (cylindrical.center.0 + radius * radians.cos(), cylindrical.center.1 + radius * radians.sin());
                    let point = self.grid.to_world(local);
                    let distance = ((point.0 - x).powi(2) + (point.1 - y).powi(2)).sqrt();
                    if distance < tolerance && distance < best_distance {
                        best_distance = distance;
                        best = Some(point);
                    }
                }
            }
        }
        best.unwrap_or((round_s(x), round_s(y)))
    }

    /// Snapping in an elevation view: joints on the plane, perpendicular grid
    /// lines, story levels and reference-plane intersections.
    fn snap_on_elevation(&self, u: f64, v: f64, tolerance: f64) -> (f64, f64) {
        let plane = self.active_work_plane();
        let mut best_joint = None;
        let mut best_joint_distance = f64::INFINITY;
        for node in self.model.nodes.values() {
            let Some((nu, nv)) = self.node_uv(node) else {
                continue;
            };
            let distance = ((nu - u).powi(2) + (nv - v).powi(2)).sqrt();
            if distance < tolerance && distance < best_joint_distance {
                best_joint_distance = distance;
                best_joint = Some((nu, nv));
            }
        }
        if let Some(point) = best_joint {
            return point;
        }

        let mut best_u = None;
        let mut best_u_distance = f64::INFINITY;
        let mut best_v = None;
        let mut best_v_distance = f64::INFINITY;
        let lines: &[f64] = match plane.axis {
            RefPlaneAxis::X => &self.grid.ys,
            RefPlaneAxis::Y => &self.grid.xs,
            RefPlaneAxis::Z => &[],
        };
        for &line in lines {
            let distance = (line - u).abs();
            if distance < tolerance && distance < best_u_distance {
                best_u_distance = distance;
                best_u = Some(line);
            }
        }
        for reference in &self.ref_planes {
            let vertical = match (plane.axis, reference.axis) {
                (RefPlaneAxis::X, RefPlaneAxis::Y) => true,
                (RefPlaneAxis::Y, RefPlaneAxis::X) => true,
                _ => false,
            };
            if vertical {
                let distance = (reference.offset - u).abs();
                if distance < tolerance && distance < best_u_distance {
                    best_u_distance = distance;
                    best_u = Some(reference.offset);
                }
            } else if reference.axis == RefPlaneAxis::Z {
                let distance = (reference.offset - v).abs();
                if distance < tolerance && distance < best_v_distance {
                    best_v_distance = distance;
                    best_v = Some(reference.offset);
                }
            }
        }
        for story in &self.model.stories {
            let distance = (story.elevation - v).abs();
            if distance < tolerance && distance < best_v_distance {
                best_v_distance = distance;
                best_v = Some(story.elevation);
            }
        }
        (best_u.unwrap_or_else(|| round_s(u)), best_v.unwrap_or_else(|| round_s(v)))
    }

    fn element_on_active_story(&self, element: &Element) -> bool {
        match (self.model.nodes.get(&element.node_i), self.model.nodes.get(&element.node_j)) {
            (Some(a), Some(b)) => self.node_on_active_story(a) && self.node_on_active_story(b),
            _ => false,
        }
    }

    /// Plane-local coordinates of a joint that lies on the active work plane.
    fn node_uv(&self, node: &Node) -> Option<(f64, f64)> {
        let plane = self.active_work_plane();
        if plane.contains([node.x, node.y, node.z], 1e-6) { Some(plane.to_local([node.x, node.y, node.z])) } else { None }
    }

    fn node_screen(&self, view: &View, rect: egui::Rect, node: &Node) -> Option<egui::Pos2> {
        self.node_uv(node).map(|(u, v)| view.to_screen(rect, u, v))
    }

    /// Section-edge offset of a frame from its centreline, in world metres.
    fn frame_cardinal_offset(&self, element: &Element, a: &Node, b: &Node) -> [f64; 3] {
        let Some(section) = self.model.sections.get(&element.section_id) else {
            return [0.0; 3];
        };
        openstruct::elements::cardinal_offset(
            element.cardinal_point,
            section.width,
            section.depth,
            [a.x, a.y, a.z],
            [b.x, b.y, b.z],
        )
    }

    fn offset_screen(&self, view: &View, rect: egui::Rect, node: &Node, offset: [f64; 3]) -> egui::Pos2 {
        let plane = self.active_work_plane();
        let (u, v) = plane.to_local([node.x + offset[0], node.y + offset[1], node.z + offset[2]]);
        view.to_screen(rect, u, v)
    }

    fn node_at(&self, rect: egui::Rect, point: egui::Pos2) -> Option<usize> {
        let view = self.view;
        let mut best: Option<(f32, usize)> = None;
        for (id, node) in &self.model.nodes {
            let Some(screen) = self.node_screen(&view, rect, node) else {
                continue;
            };
            let distance = screen.distance(point);
            if distance < 10.0 && best.map_or(true, |(best_distance, _)| distance < best_distance) {
                best = Some((distance, *id));
            }
        }
        best.map(|(_, id)| id)
    }

    fn element_at(&self, rect: egui::Rect, point: egui::Pos2) -> Option<usize> {
        let view = self.view;
        let mut best: Option<(f32, usize)> = None;
        for (id, element) in &self.model.elements {
            if !self.element_on_active_story(element) {
                continue;
            }
            let (Some(ni), Some(nj)) = (self.model.nodes.get(&element.node_i), self.model.nodes.get(&element.node_j)) else {
                continue;
            };
            let offset = self.frame_cardinal_offset(element, ni, nj);
            let a = self.offset_screen(&view, rect, ni, offset);
            let b = self.offset_screen(&view, rect, nj, offset);
            let distance = seg_dist(point, a, b);
            if distance < 8.0 && best.map_or(true, |(best_distance, _)| distance < best_distance) {
                best = Some((distance, *id));
            }
        }
        best.map(|(_, id)| id)
    }

    fn area_at(&self, rect: egui::Rect, point: egui::Pos2) -> Option<usize> {
        let view = self.view;
        for (id, area) in &self.model.areas {
            if area.kind == AreaKind::Wall {
                let base: Vec<egui::Pos2> =
                    area.node_ids.iter().filter_map(|node_id| self.model.nodes.get(node_id).and_then(|node| self.node_screen(&view, rect, node))).collect();
                if !self.work_plane_horizontal() {
                    if base.len() >= 3 && point_in_polygon(point, &base) {
                        return Some(*id);
                    }
                }
                for pair in base.windows(2) {
                    if seg_dist(point, pair[0], pair[1]) < 8.0 {
                        return Some(*id);
                    }
                }
                continue;
            }
            if !self.area_on_active_story(area) {
                continue;
            }
            let plane = self.active_work_plane();
            let points: Vec<egui::Pos2> = area
                .polygon(&self.model)
                .into_iter()
                .map(|point| {
                    let (u, v) = plane.to_local(point);
                    view.to_screen(rect, u, v)
                })
                .collect();
            if points.len() >= 3 {
                let on_edge = points
                    .iter()
                    .zip(points.iter().cycle().skip(1))
                    .take(points.len())
                    .any(|(a, b)| seg_dist(point, *a, *b) < 8.0);
                if on_edge || point_in_polygon(point, &points) {
                    return Some(*id);
                }
            }
        }
        None
    }

    fn pick_3d(&mut self, rect: egui::Rect, point: egui::Pos2, shift: bool) {
        let projector = Projector::new(&self.camera, rect);
        let mut best_node: Option<(f32, usize)> = None;
        for (id, node) in &self.model.nodes {
            if let Some((screen, _)) = projector.project([node.x, node.y, node.z]) {
                let distance = screen.distance(point);
                if distance < 12.0 && best_node.map_or(true, |(best, _)| distance < best) {
                    best_node = Some((distance, *id));
                }
            }
        }
        if let Some((_, id)) = best_node {
            if !shift {
                self.selected_nodes.clear();
                self.selected_elements.clear();
                self.selected_areas.clear();
            }
            self.selected_nodes.insert(id);
            self.status_text = format!("Selected joint N{id} ({}) in 3D.", self.story_name_at(self.model.nodes[&id].z));
            return;
        }

        let mut best_element: Option<(f32, usize)> = None;
        for (id, element) in &self.model.elements {
            let (Some(ni), Some(nj)) = (self.model.nodes.get(&element.node_i), self.model.nodes.get(&element.node_j)) else {
                continue;
            };
            let offset = self.frame_cardinal_offset(element, ni, nj);
            if let (Some((a, _)), Some((b, _))) = (
                projector.project([ni.x + offset[0], ni.y + offset[1], ni.z + offset[2]]),
                projector.project([nj.x + offset[0], nj.y + offset[1], nj.z + offset[2]]),
            ) {
                let distance = seg_dist(point, a, b);
                if distance < 8.0 && best_element.map_or(true, |(best, _)| distance < best) {
                    best_element = Some((distance, *id));
                }
            }
        }
        if let Some((_, id)) = best_element {
            if !shift {
                self.selected_nodes.clear();
                self.selected_elements.clear();
                self.selected_areas.clear();
            }
            self.selected_elements.insert(id);
            self.status_text = format!("Selected frame F{id} in 3D.");
            return;
        }

        let mut best_area: Option<(f64, usize)> = None;
        for (id, area) in &self.model.areas {
            let mut points = Vec::new();
            let mut depth = 0.0;
            let mut complete = true;
            for node_id in &area.node_ids {
                let Some(node) = self.model.nodes.get(node_id) else {
                    complete = false;
                    break;
                };
                match projector.project([node.x, node.y, node.z]) {
                    Some((screen, d)) => {
                        points.push(screen);
                        depth += d;
                    }
                    None => {
                        complete = false;
                        break;
                    }
                }
            }
            if complete && points.len() >= 3 && point_in_polygon(point, &points) {
                let average = depth / points.len() as f64;
                if best_area.map_or(true, |(best, _)| average < best) {
                    best_area = Some((average, *id));
                }
            }
        }
        if let Some((_, id)) = best_area {
            if !shift {
                self.selected_nodes.clear();
                self.selected_elements.clear();
                self.selected_areas.clear();
            }
            self.selected_areas.insert(id);
            self.status_text = format!("Selected area A{id} in 3D.");
        } else if !shift {
            self.selected_nodes.clear();
            self.selected_elements.clear();
            self.selected_areas.clear();
            self.status_text = "Selection cleared.".to_string();
        }
    }

    /// Grid spacing dimensions along the bottom and left edges of the plan.
    fn draw_plan_dimensions(&self, painter: &egui::Painter, rect: egui::Rect) {
        let pal = theme::palette(self.dark);
        let view = self.view;
        let (u0, v1) = view.to_world(rect, rect.left_top());
        let (u1, v0) = view.to_world(rect, rect.right_bottom());
        let stroke = egui::Stroke::new(1.0, with_alpha(pal.text_dim, 180));
        let text = egui::FontId::proportional(9.0);
        let baseline = rect.bottom() - 32.0;
        let xs: Vec<f64> = self.grid.xs.iter().enumerate().filter(|(index, _)| self.grid.x_line_visible(*index)).map(|(_, x)| *x).collect();
        for pair in xs.windows(2) {
            if pair[1] < u0.min(u1) || pair[0] > u0.max(u1) {
                continue;
            }
            let (a, b) = (view.to_screen(rect, pair[0], v0), view.to_screen(rect, pair[1], v0));
            let (ax, bx) = (a.x, b.x);
            painter.line_segment([egui::pos2(ax, baseline), egui::pos2(bx, baseline)], stroke);
            painter.line_segment([egui::pos2(ax, baseline - 4.0), egui::pos2(ax, baseline + 4.0)], stroke);
            painter.line_segment([egui::pos2(bx, baseline - 4.0), egui::pos2(bx, baseline + 4.0)], stroke);
            painter.text(
                egui::pos2((ax + bx) / 2.0, baseline - 3.0),
                egui::Align2::CENTER_BOTTOM,
                format!("{:.2}", (pair[1] - pair[0]).abs()),
                text.clone(),
                pal.text_dim,
            );
        }
        let left = rect.left() + 30.0;
        let ys: Vec<f64> = self.grid.ys.iter().enumerate().filter(|(index, _)| self.grid.y_line_visible(*index)).map(|(_, y)| *y).collect();
        for pair in ys.windows(2) {
            if pair[1] < v0.min(v1) || pair[0] > v0.max(v1) {
                continue;
            }
            let (a, b) = (view.to_screen(rect, u0, pair[0]), view.to_screen(rect, u0, pair[1]));
            let (ay, by) = (a.y, b.y);
            painter.line_segment([egui::pos2(left, ay), egui::pos2(left, by)], stroke);
            painter.line_segment([egui::pos2(left - 4.0, ay), egui::pos2(left + 4.0, ay)], stroke);
            painter.line_segment([egui::pos2(left - 4.0, by), egui::pos2(left + 4.0, by)], stroke);
            painter.text(
                egui::pos2(left - 4.0, (ay + by) / 2.0),
                egui::Align2::RIGHT_CENTER,
                format!("{:.2}", (pair[1] - pair[0]).abs()),
                text.clone(),
                pal.text_dim,
            );
        }
    }

    /// Horizontal story levels and perpendicular grid lines shown in an elevation view.
    fn draw_elevation_guide_grid(&self, painter: &egui::Painter, rect: egui::Rect) {
        let pal = theme::palette(self.dark);
        let view = self.view;
        let plane = self.active_work_plane();
        let (u0, v1) = view.to_world(rect, rect.left_top());
        let (u1, v0) = view.to_world(rect, rect.right_bottom());
        for story in &self.model.stories {
            if story.elevation < v0 || story.elevation > v1 {
                continue;
            }
            let a = view.to_screen(rect, u0, story.elevation);
            let b = view.to_screen(rect, u1, story.elevation);
            painter.line_segment([a, b], egui::Stroke::new(1.0, pal.grid));
            painter.text(
                egui::pos2(rect.left() + 6.0, a.y - 2.0),
                egui::Align2::LEFT_BOTTOM,
                format!("{}  z = {:.2}", story.name, story.elevation),
                egui::FontId::proportional(9.0),
                pal.text_dim,
            );
        }
        // Story-height dimensions down the right edge.
        if self.show_dimensions {
        let mut levels: Vec<f64> = self.model.stories.iter().map(|story| story.elevation).collect();
        levels.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let dimension_x = rect.right() - 34.0;
        let stroke = egui::Stroke::new(1.0, with_alpha(pal.text_dim, 180));
        for pair in levels.windows(2) {
            if pair[1] < v0 || pair[0] > v1 {
                continue;
            }
            let (a, b) = (view.to_screen(rect, u0, pair[0]), view.to_screen(rect, u0, pair[1]));
            painter.line_segment([egui::pos2(dimension_x, a.y), egui::pos2(dimension_x, b.y)], stroke);
            painter.line_segment([egui::pos2(dimension_x - 4.0, a.y), egui::pos2(dimension_x + 4.0, a.y)], stroke);
            painter.line_segment([egui::pos2(dimension_x - 4.0, b.y), egui::pos2(dimension_x + 4.0, b.y)], stroke);
            painter.text(
                egui::pos2(dimension_x - 4.0, (a.y + b.y) / 2.0),
                egui::Align2::RIGHT_CENTER,
                format!("{:.2}", (pair[1] - pair[0]).abs()),
                egui::FontId::proportional(9.0),
                pal.text_dim,
            );
        }
        }
        let mut lines: Vec<(f64, String, BubbleLoc)> = Vec::new();
        match plane.axis {
            RefPlaneAxis::X => {
                for (index, &y) in self.grid.ys.iter().enumerate() {
                    if self.grid.y_line_visible(index) {
                        lines.push((y, self.grid.label_y(index), self.grid.y_bubble_at(index)));
                    }
                }
            }
            RefPlaneAxis::Y => {
                for (index, &x) in self.grid.xs.iter().enumerate() {
                    if self.grid.x_line_visible(index) {
                        lines.push((x, self.grid.label_x(index), self.grid.x_bubble_at(index)));
                    }
                }
            }
            RefPlaneAxis::Z => {}
        }
        for (u, label, loc) in lines {
            if u < u0.min(u1) || u > u0.max(u1) {
                continue;
            }
            let a = view.to_screen(rect, u, v0);
            let b = view.to_screen(rect, u, v1);
            painter.line_segment([a, b], egui::Stroke::new(1.0, pal.grid));
            let top = egui::pos2(a.x, rect.top() + 12.0);
            let bottom = egui::pos2(a.x, rect.bottom() - 12.0);
            for at in bubble_ends(loc, top, bottom) {
                painter.circle_filled(at, 9.0, pal.panel_alt);
                painter.circle_stroke(at, 9.0, egui::Stroke::new(1.0, pal.grid));
                painter.text(at, egui::Align2::CENTER_CENTER, label.clone(), egui::FontId::proportional(9.0), pal.text_dim);
            }
        }
    }

    fn draw_ref_planes_plan(&self, painter: &egui::Painter, rect: egui::Rect) {
        if self.ref_planes.is_empty() {
            return;
        }
        let pal = theme::palette(self.dark);
        let view = self.view;
        let (wx0, wy1) = view.to_world(rect, rect.left_top());
        let (wx1, wy0) = view.to_world(rect, rect.right_bottom());
        let stroke = egui::Stroke::new(1.0, with_alpha(pal.warn, 160));

        if !self.work_plane_horizontal() {
            let work = self.active_work_plane();
            for plane in &self.ref_planes {
                let (start, end, label_pos) = match plane.axis {
                    RefPlaneAxis::Z => {
                        if plane.offset < wy0 || plane.offset > wy1 {
                            continue;
                        }
                        let left = view.to_screen(rect, wx0, plane.offset);
                        (left, view.to_screen(rect, wx1, plane.offset), left + egui::vec2(6.0, -6.0))
                    }
                    _ if plane.axis == work.axis => continue,
                    _ => {
                        if plane.offset < wx0.min(wx1) || plane.offset > wx0.max(wx1) {
                            continue;
                        }
                        let top = view.to_screen(rect, plane.offset, wy1);
                        (top, view.to_screen(rect, plane.offset, wy0), top + egui::vec2(6.0, 10.0))
                    }
                };
                painter.extend(egui::Shape::dashed_line(&[start, end], stroke, 7.0, 5.0));
                painter.text(
                    label_pos,
                    egui::Align2::LEFT_TOP,
                    format!("{}  {}", plane.name, plane.describe()),
                    egui::FontId::proportional(10.0),
                    with_alpha(pal.warn, 210),
                );
            }
            return;
        }

        for plane in &self.ref_planes {
            let (start, end, label_pos) = match plane.axis {
                RefPlaneAxis::X => {
                    if plane.offset < wx0.min(wx1) || plane.offset > wx0.max(wx1) {
                        continue;
                    }
                    let top = view.to_screen(rect, plane.offset, wy1);
                    (top, view.to_screen(rect, plane.offset, wy0), top + egui::vec2(6.0, 10.0))
                }
                RefPlaneAxis::Y => {
                    if plane.offset < wy0.min(wy1) || plane.offset > wy0.max(wy1) {
                        continue;
                    }
                    let left = view.to_screen(rect, wx0, plane.offset);
                    (left, view.to_screen(rect, wx1, plane.offset), left + egui::vec2(6.0, -6.0))
                }
                RefPlaneAxis::Z => continue,
            };
            painter.extend(egui::Shape::dashed_line(&[start, end], stroke, 7.0, 5.0));
            painter.text(
                label_pos,
                egui::Align2::LEFT_TOP,
                format!("{}  {}", plane.name, plane.describe()),
                egui::FontId::proportional(10.0),
                with_alpha(pal.warn, 210),
            );
        }
    }

    fn draw_ref_planes_3d(&self, painter: &egui::Painter, projector: &Projector, x0: f64, x1: f64, y0: f64, y1: f64, z0: f64, z1: f64) {
        if self.ref_planes.is_empty() {
            return;
        }
        let pal = theme::palette(self.dark);
        let stroke = egui::Stroke::new(1.0, with_alpha(pal.warn, 120));
        for plane in &self.ref_planes {
            let corners = plane.corners(x0, x1, y0, y1, z0, z1);
            let projected: Option<Vec<egui::Pos2>> = corners.iter().map(|&point| projector.project(point).map(|(screen, _)| screen)).collect();
            let Some(points) = projected else {
                continue;
            };
            for index in 0..4 {
                painter.extend(egui::Shape::dashed_line(&[points[index], points[(index + 1) % 4]], stroke, 6.0, 5.0));
            }
            painter.text(
                points[0] + egui::vec2(4.0, -4.0),
                egui::Align2::LEFT_BOTTOM,
                format!("{}  {}", plane.name, plane.describe()),
                egui::FontId::proportional(10.0),
                with_alpha(pal.warn, 210),
            );
        }
    }

    fn draw_cylindrical_grid(&self, painter: &egui::Painter, rect: egui::Rect, grid: &CylindricalGrid) {
        let pal = theme::palette(self.dark);
        let view = self.view;
        let stroke = egui::Stroke::new(1.0, with_alpha(self.active_story_color(), 210));
        let bubble = (self.grid.bubble_size as f32 * view.scale).clamp(6.0, 40.0);
        let (cx, cy) = self.grid.to_world((grid.center.0, grid.center.1));
        let center = view.to_screen(rect, cx, cy);
        painter.circle_stroke(center, 3.0, egui::Stroke::new(1.0, pal.text_dim));
        let outer = grid.outer_radius();
        for (index, radius) in grid.radii.iter().enumerate() {
            let points: Vec<egui::Pos2> = (0..=96)
                .map(|step| {
                    let angle = std::f64::consts::TAU * step as f64 / 96.0;
                    let (px, py) = self
                        .grid
                        .to_world((grid.center.0 + radius * angle.cos(), grid.center.1 + radius * angle.sin()));
                    view.to_screen(rect, px, py)
                })
                .collect();
            painter.add(egui::Shape::line(points, stroke));
            let (lx, ly) = self.grid.to_world((grid.center.0 + radius, grid.center.1));
            let label = view.to_screen(rect, lx, ly);
            painter.circle_filled(label, bubble, pal.panel_alt);
            painter.circle_stroke(label, bubble, stroke);
            painter.text(label, egui::Align2::CENTER_CENTER, grid.ring_label(index), egui::FontId::proportional(9.0), pal.text_dim);
        }
        for (index, angle) in grid.angles_deg.iter().enumerate() {
            let radians = angle.to_radians();
            let (ex, ey) = self.grid.to_world((grid.center.0 + outer * radians.cos(), grid.center.1 + outer * radians.sin()));
            let end = view.to_screen(rect, ex, ey);
            painter.line_segment([center, end], stroke);
            painter.circle_filled(end, bubble, pal.panel_alt);
            painter.circle_stroke(end, bubble, stroke);
            painter.text(end, egui::Align2::CENTER_CENTER, grid.radial_label(index), egui::FontId::proportional(9.0), pal.text_dim);
        }
    }

    fn load_arrows(&self, painter: &egui::Painter, rect: egui::Rect) {
        let pal = theme::palette(self.dark);
        let view = self.view;
        for (&node_id, load) in &self.nodal_loads {
            if load.iter().all(|&value| value == 0.0) {
                continue;
            }
            let Some(node) = self.model.nodes.get(&node_id) else {
                continue;
            };
            if !self.node_on_active_story(node) {
                continue;
            }
            let point = view.to_screen(rect, node.x, node.y);
            let mut direction = egui::vec2(load[0] as f32 + load[1] as f32 * 0.35, load[1] as f32 + load[2] as f32 * 0.05);
            if direction.length() < 1e-3 {
                direction = egui::vec2(0.0, 1.0);
            }
            direction = direction.normalized();
            let tip = point + direction * 26.0;
            painter.line_segment([point, tip], egui::Stroke::new(2.0, pal.load));
            let perpendicular = egui::vec2(-direction.y, direction.x);
            painter.line_segment([tip - direction * 7.0 + perpendicular * 4.0, tip], egui::Stroke::new(2.0, pal.load));
            painter.line_segment([tip - direction * 7.0 - perpendicular * 4.0, tip], egui::Stroke::new(2.0, pal.load));
            painter.text(point - direction * 14.0, egui::Align2::CENTER_CENTER, format!("{:.0}", load[2]), egui::FontId::proportional(9.0), pal.load);
        }
    }
}

fn point_in_polygon(point: egui::Pos2, polygon: &[egui::Pos2]) -> bool {
    if polygon.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = polygon.len() - 1;
    for i in 0..polygon.len() {
        let a = polygon[i];
        let b = polygon[j];
        if ((a.y > point.y) != (b.y > point.y)) && (point.x < (b.x - a.x) * (point.y - a.y) / (b.y - a.y) + a.x) {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn seg_dist(point: egui::Pos2, a: egui::Pos2, b: egui::Pos2) -> f32 {
    let ab = b - a;
    let ap = point - a;
    if ab.length_sq() < 1e-9 {
        return ap.length();
    }
    let t = (ap.dot(ab) / ab.length_sq()).clamp(0.0, 1.0);
    (a + t * ab - point).length()
}

fn round_s(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn normalize(v: [f64; 3]) -> [f64; 3] {
    let length = dot(v, v).sqrt();
    if length < 1e-12 { [0.0, 0.0, 0.0] } else { [v[0] / length, v[1] / length, v[2] / length] }
}

fn with_alpha(color: egui::Color32, alpha: u8) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cylindrical_grid_builds_rings_angles_and_extent() {
        let grid = GridSpec::cylindrical((1.0, 2.0), 2.5, 3, 15.0, 45.0, 8);
        let cylindrical = grid.cylindrical.as_ref().unwrap();
        assert_eq!(cylindrical.radii, vec![2.5, 5.0, 7.5]);
        assert_eq!(cylindrical.angles_deg.len(), 8);
        assert!((cylindrical.angles_deg[1] - 60.0).abs() < 1e-9);
        assert!((cylindrical.outer_radius() - 7.5).abs() < 1e-9);
        let (x0, x1, y0, y1) = grid.bounds();
        assert!((x0 - (1.0 - 7.5)).abs() < 1e-9);
        assert!((x1 - (1.0 + 7.5)).abs() < 1e-9);
        assert!((y0 - (2.0 - 7.5)).abs() < 1e-9);
        assert!((y1 - (2.0 + 7.5)).abs() < 1e-9);
    }

    #[test]
    fn rectangular_grid_stays_rectangular() {
        let grid = GridSpec::new(6.0, 2, 1);
        assert!(grid.cylindrical.is_none());
        assert_eq!(grid.bounds(), (0.0, 12.0, 0.0, 6.0));
    }

    #[test]
    fn grid_origin_and_rotation_move_the_lines_and_round_trip() {
        let mut grid = GridSpec::new(5.0, 2, 1);
        assert!(!grid.is_rotated());

        grid.origin = (10.0, -4.0);
        grid.rotation_deg = 90.0;
        assert!(grid.is_rotated());

        let world = grid.to_world((5.0, 0.0));
        assert!((world.0 - 10.0).abs() < 1e-9, "the local X ordinate turns into global Y");
        assert!((world.1 + 4.0 - 5.0).abs() < 1e-9, "local X becomes global Y after a quarter turn");

        let local = grid.to_local(world);
        assert!((local.0 - 5.0).abs() < 1e-9 && local.1.abs() < 1e-9, "to_local must undo to_world");

        let segments = grid.segments();
        assert_eq!(segments.len(), 7, "rotation must not change the segment count");
        assert!(
            segments.iter().all(|(a, b)| (a.0 - b.0).abs() < 1e-9 || (a.1 - b.1).abs() < 1e-9),
            "a quarter turn keeps the lines parallel to the global axes"
        );

        let (x0, x1, y0, y1) = grid.bounds();
        assert!((x0 - 5.0).abs() < 1e-9 && (x1 - 10.0).abs() < 1e-9, "bounds X after the turn: {x0}..{x1}");
        assert!((y0 + 4.0).abs() < 1e-9 && (y1 - 6.0).abs() < 1e-9, "bounds Y after the turn: {y0}..{y1}");
    }

    #[test]
    fn grid_segments_split_at_every_crossing() {
        let rectangular = GridSpec::new(5.0, 2, 1);
        assert_eq!(rectangular.segments().len(), 7);

        let cylindrical = GridSpec::cylindrical((0.0, 0.0), 5.0, 2, 0.0, 90.0, 4);
        assert_eq!(cylindrical.segments().len(), 16);
    }

    #[test]
    fn grid_segments_can_be_filtered_by_line_family() {
        let rectangular = GridSpec::new(5.0, 2, 1);
        assert_eq!(rectangular.segments_filtered(true, false, false, false).len(), 3);
        assert_eq!(rectangular.segments_filtered(false, true, false, false).len(), 4);
        assert_eq!(rectangular.segments_filtered(false, false, false, false).len(), 0);

        let cylindrical = GridSpec::cylindrical((0.0, 0.0), 5.0, 2, 0.0, 90.0, 4);
        assert_eq!(cylindrical.segments_filtered(false, false, true, false).len(), 8);
        assert_eq!(cylindrical.segments_filtered(false, false, false, true).len(), 8);
    }

    #[test]
    fn grid_line_visibility_and_bubble_locations_default_to_shown_both_ends() {
        let mut grid = GridSpec::new(5.0, 2, 1);
        assert!(grid.x_line_visible(0) && grid.x_line_visible(2) && grid.x_line_visible(9));
        assert_eq!(grid.x_bubble_at(0), BubbleLoc::Both);

        grid.x_visible = vec![true, false, true];
        grid.y_bubble = vec![BubbleLoc::End, BubbleLoc::Start];
        assert!(!grid.x_line_visible(1));
        assert_eq!(grid.y_bubble_at(1), BubbleLoc::Start);
        assert_eq!(grid.y_bubble_at(7), BubbleLoc::Both, "missing entries keep the default");

        let json = serde_json::to_string(&grid).unwrap();
        let restored: GridSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.x_visible, grid.x_visible);
        assert_eq!(restored.y_bubble, grid.y_bubble);

        let legacy: GridSpec = serde_json::from_str(r#"{"xs":[0.0,5.0],"ys":[0.0,5.0]}"#).unwrap();
        assert!(legacy.x_line_visible(0));
        assert_eq!(legacy.y_bubble_at(0), BubbleLoc::Both);
    }

    #[test]
    fn custom_grid_labels_override_defaults_and_blank_entries_fall_back() {
        let mut grid = GridSpec::new(5.0, 2, 1);
        grid.x_labels = vec!["Grid 1".to_string(), "  ".to_string(), "Grid 3".to_string()];
        grid.y_labels = vec![String::new(), "North".to_string()];
        assert_eq!(grid.label_x(0), "Grid 1");
        assert_eq!(grid.label_x(1), "2");
        assert_eq!(grid.label_x(2), "Grid 3");
        assert_eq!(grid.label_y(0), "A");
        assert_eq!(grid.label_y(1), "North");

        let mut cylindrical = GridSpec::cylindrical((0.0, 0.0), 5.0, 2, 0.0, 90.0, 4);
        let grid = cylindrical.cylindrical.as_mut().unwrap();
        grid.ring_labels = vec!["Inner".to_string()];
        grid.radial_labels = vec![String::new(), "East".to_string()];
        assert_eq!(grid.ring_label(0), "Inner");
        assert_eq!(grid.ring_label(1), "R2");
        assert_eq!(grid.radial_label(0), "A1");
        assert_eq!(grid.radial_label(1), "East");
        assert_eq!(grid.radial_label(2), "A3");
        assert_eq!(default_axis_label(0), "A");
        assert_eq!(default_axis_label(25), "Z");
        assert_eq!(default_axis_label(26), "AA");
    }

    #[test]
    fn vertical_work_plane_maps_between_world_and_local() {
        let plane = WorkPlane::new(RefPlaneAxis::X, 2.0);
        assert_eq!(plane.to_world(3.0, 4.0), [2.0, 3.0, 4.0]);
        assert_eq!(plane.to_local([2.0, 3.0, 4.0]), (3.0, 4.0));
        assert!(plane.contains([2.0, -1.0, 9.0], 1e-9));
        assert!(!plane.contains([2.5, -1.0, 9.0], 1e-9));
        assert!(!plane.is_horizontal());
        assert_eq!(plane.axis_labels(), ("Y", "Z"));

        let y_plane = WorkPlane::new(RefPlaneAxis::Y, -1.5);
        assert_eq!(y_plane.to_world(6.0, 2.5), [6.0, -1.5, 2.5]);
        assert_eq!(y_plane.axis_labels(), ("X", "Z"));
    }

    #[test]
    fn ref_plane_outline_follows_its_axis() {
        let x_plane = RefPlane::new("Grid offset", RefPlaneAxis::X, 2.0);
        let corners = x_plane.corners(0.0, 10.0, 0.0, 8.0, 0.0, 3.0);
        assert_eq!(corners[0], [2.0, 0.0, 0.0]);
        assert_eq!(corners[2], [2.0, 8.0, 3.0]);
        assert_eq!(x_plane.describe(), "X = 2.000 m");

        let z_plane = RefPlane::new("Level guide", RefPlaneAxis::Z, 3.5);
        let corners = z_plane.corners(0.0, 10.0, 0.0, 8.0, 0.0, 3.0);
        assert_eq!(corners[3], [0.0, 8.0, 3.5]);
    }
}
