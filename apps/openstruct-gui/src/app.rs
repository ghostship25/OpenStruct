//! Application state, command dispatch, undo/redo, file I/O and the eframe shell.

use crate::canvas::{BubbleLoc, Camera3D, GridSpec, RefPlane, RefPlaneAxis, View, ViewPreset, WorkPlane};
use crate::commands::Cmd;
use crate::theme;
use eframe::egui;
use openstruct::analysis::AnalysisResults;
use openstruct::areas::{Area, AreaCurve, AreaKind};
use openstruct::edit::{self, AlignAxis, CopyResult, MirrorAxis};
use openstruct::elements::{CardinalPoint, Element};
use openstruct::loads::{AreaLoad, ComboType, DistributedLoad, LoadCase, LoadCombination, LoadDirection, LoadPattern, LoadType, NodalLoad, TemperatureLoad};
use openstruct::materials::Material;
use openstruct::model::{Diaphragm, Node, Story};
use openstruct::check::CheckReport;
use openstruct::design::concrete::ConcreteStrength;
use openstruct::sections::Section;
use openstruct::shapes;
use openstruct::split;
use openstruct::{Analysis, Model};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

pub const MAX_UNDO: usize = 50;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    Select,
    DrawNode,
    DrawFrame,
    DrawArea,
    DrawWall,
}

/// Plan shape used by the area (slab) tool.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AreaShape {
    Rectangle,
    Polygon,
    Circle,
    Ellipse,
    Arc,
}

impl AreaShape {
    pub const ALL: [AreaShape; 5] = [AreaShape::Rectangle, AreaShape::Polygon, AreaShape::Circle, AreaShape::Ellipse, AreaShape::Arc];

    pub fn label(self) -> &'static str {
        match self {
            AreaShape::Rectangle => "Rectangle",
            AreaShape::Polygon => "Polygon",
            AreaShape::Circle => "Circle",
            AreaShape::Ellipse => "Ellipse",
            AreaShape::Arc => "Arc",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ViewTab {
    Plan,
    View3D,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EditMode {
    Move,
    Copy,
    Mirror,
    Rotate,
    Replicate,
}

impl EditMode {
    pub fn title(self) -> &'static str {
        match self {
            EditMode::Move => "Move Selection",
            EditMode::Copy => "Copy Selection",
            EditMode::Mirror => "Mirror Selection",
            EditMode::Rotate => "Rotate Selection",
            EditMode::Replicate => "Replicate Selection",
        }
    }
}

/// Which object families are included by Select by Property.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SelectScope {
    All,
    Joints,
    Frames,
    Areas,
}

impl SelectScope {
    pub const ALL: [SelectScope; 4] = [SelectScope::All, SelectScope::Joints, SelectScope::Frames, SelectScope::Areas];

    pub fn label(self) -> &'static str {
        match self {
            SelectScope::All => "Joints, frames and areas",
            SelectScope::Joints => "Joints only",
            SelectScope::Frames => "Frames only",
            SelectScope::Areas => "Areas only",
        }
    }
}

/// A named selection set. Groups are document data, so they survive save/load
/// and undo just like geometry.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Group {
    pub name: String,
    #[serde(default)]
    pub nodes: Vec<usize>,
    #[serde(default)]
    pub elements: Vec<usize>,
    #[serde(default)]
    pub areas: Vec<usize>,
}

impl Group {
    pub fn from_selection(name: impl Into<String>, nodes: &HashSet<usize>, elements: &HashSet<usize>, areas: &HashSet<usize>) -> Self {
        let mut nodes: Vec<usize> = nodes.iter().copied().collect();
        let mut elements: Vec<usize> = elements.iter().copied().collect();
        let mut areas: Vec<usize> = areas.iter().copied().collect();
        nodes.sort_unstable();
        elements.sort_unstable();
        areas.sort_unstable();
        Self { name: name.into(), nodes, elements, areas }
    }

    pub fn count(&self) -> usize {
        self.nodes.len() + self.elements.len() + self.areas.len()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum BottomTab {
    Joints,
    Displacements,
    Reactions,
    Forces,
    FrameLoads,
    AreaLoads,
    TemperatureLoads,
    Sections,
    Combinations,
    Envelope,
}

impl BottomTab {
    pub const ALL: [BottomTab; 10] = [
        BottomTab::Joints,
        BottomTab::Displacements,
        BottomTab::Reactions,
        BottomTab::Forces,
        BottomTab::FrameLoads,
        BottomTab::AreaLoads,
        BottomTab::TemperatureLoads,
        BottomTab::Sections,
        BottomTab::Combinations,
        BottomTab::Envelope,
    ];

    pub fn title(self) -> &'static str {
        match self {
            BottomTab::Joints => "Joint Coordinates",
            BottomTab::Displacements => "Joint Displacements",
            BottomTab::Reactions => "Joint Reactions",
            BottomTab::Forces => "Frame Forces",
            BottomTab::FrameLoads => "Frame Loads",
            BottomTab::AreaLoads => "Area Loads",
            BottomTab::TemperatureLoads => "Temperature Loads",
            BottomTab::Sections => "Frame Sections",
            BottomTab::Combinations => "Combinations",
            BottomTab::Envelope => "Combined Envelope",
        }
    }
}

pub use openstruct::units::UnitSystem;

/// Shape family of a frame-section draft row.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SectionKind {
    Rectangular,
    Circular,
    ISection,
    Channel,
    Tee,
    Box,
    Pipe,
    Angle,
    /// Composite section drawn in the Section Designer.
    Custom,
}

impl SectionKind {
    pub const ALL: [SectionKind; 9] = [
        SectionKind::Rectangular,
        SectionKind::Circular,
        SectionKind::ISection,
        SectionKind::Channel,
        SectionKind::Tee,
        SectionKind::Box,
        SectionKind::Pipe,
        SectionKind::Angle,
        SectionKind::Custom,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SectionKind::Rectangular => "Rectangular",
            SectionKind::Circular => "Circular",
            SectionKind::ISection => "I / wide flange",
            SectionKind::Channel => "Channel",
            SectionKind::Tee => "Tee",
            SectionKind::Box => "Box",
            SectionKind::Pipe => "Pipe",
            SectionKind::Angle => "Angle",
            SectionKind::Custom => "Custom (drawn)",
        }
    }

    fn name_prefix(self) -> &'static str {
        match self {
            SectionKind::Rectangular => "RECT",
            SectionKind::Circular => "CIRC",
            SectionKind::ISection => "I",
            SectionKind::Channel => "CHAN",
            SectionKind::Tee => "TEE",
            SectionKind::Box => "BOX",
            SectionKind::Pipe => "PIPE",
            SectionKind::Angle => "L",
            SectionKind::Custom => "CUSTOM",
        }
    }

    /// The width is a free dimension rather than a mirrored diameter.
    pub fn uses_width(self) -> bool {
        matches!(
            self,
            SectionKind::Rectangular | SectionKind::ISection | SectionKind::Channel | SectionKind::Tee | SectionKind::Box | SectionKind::Angle
        )
    }

    /// The web/wall column is meaningful.
    pub fn uses_wall(self) -> bool {
        matches!(
            self,
            SectionKind::ISection | SectionKind::Channel | SectionKind::Tee | SectionKind::Box | SectionKind::Pipe | SectionKind::Angle
        )
    }

    /// The flange column is meaningful.
    pub fn uses_flange(self) -> bool {
        matches!(self, SectionKind::ISection | SectionKind::Channel | SectionKind::Tee)
    }
}

/// Editable frame-section row; `id == 0` means "assign a new id on apply".
#[derive(Clone)]
pub struct SectionDraft {
    pub id: usize,
    pub kind: SectionKind,
    pub name: String,
    pub width: f64,
    pub depth: f64,
    /// Web thickness, wall thickness or angle leg thickness, depending on the shape.
    pub web: f64,
    /// Flange thickness for I, channel and tee shapes.
    pub flange: f64,
    /// Drawn parts for a custom section.
    pub designer: Vec<openstruct::sections::DesignerPart>,
    /// Solver stiffness modifiers (1.0 = unmodified).
    pub modifiers: openstruct::sections::SectionModifiers,
}

/// Part type the section designer adds on the next drag.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DesignerPartKind {
    Rectangle,
    Circle,
    Polygon,
}

/// Everything that is saved to a `.ostrut` file and captured by undo/redo.
#[derive(Clone, Serialize, Deserialize)]
pub struct Doc {
    pub model: Model,
    pub nodal_loads: HashMap<usize, [f64; 3]>,
    pub grid: GridSpec,
    #[serde(default)]
    pub ref_planes: Vec<RefPlane>,
    /// Index of the story whose height is the typical story height.
    #[serde(default)]
    pub master_story: Option<usize>,
    #[serde(default)]
    pub groups: Vec<Group>,
}

impl Doc {
    fn from_model(model: Model) -> Self {
        Self {
            model,
            nodal_loads: HashMap::new(),
            grid: GridSpec::new(5.0, 4, 3),
            ref_planes: Vec::new(),
            master_story: None,
            groups: Vec::new(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    pub dark: bool,
    pub units: UnitSystem,
    pub show_grid: bool,
    pub show_loads: bool,
    pub show_explorer: bool,
    pub show_properties: bool,
    pub show_tables: bool,
    pub show_joints: bool,
    pub show_frames: bool,
    pub show_areas: bool,
    pub show_labels: bool,
    pub show_supports: bool,
    pub show_dimensions: bool,
    pub show_ref_planes: bool,
    pub show_frame_loads: bool,
    pub show_area_loads: bool,
    pub load_symbol_scale: f32,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            dark: true,
            units: UnitSystem::Metric,
            show_grid: true,
            show_loads: true,
            show_explorer: true,
            show_properties: true,
            show_tables: true,
            show_joints: true,
            show_frames: true,
            show_areas: true,
            show_labels: true,
            show_supports: true,
            show_dimensions: true,
            show_ref_planes: true,
            show_frame_loads: true,
            show_area_loads: true,
            load_symbol_scale: 1.0,
        }
    }
}

/// First-model templates offered by the New Model dialog.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NewModelTemplate {
    Blank,
    Joints,
    Building,
}

impl NewModelTemplate {
    pub const ALL: [Self; 3] = [Self::Blank, Self::Joints, Self::Building];

    pub fn label(&self) -> &'static str {
        match self {
            Self::Blank => "Blank",
            Self::Joints => "Joints at grid",
            Self::Building => "Building Frame",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Blank => "Grid and stories only; nothing is placed on the plan.",
            Self::Joints => "A joint at every grid intersection on every story, no members.",
            Self::Building => "Columns at every intersection, beams along both axes, fixed bases.",
        }
    }
}

pub struct OpenStructApp {
    pub ctx: egui::Context,
    pub model: Model,
    pub view: View,
    pub camera: Camera3D,
    pub grid: GridSpec,
    pub tool: Tool,
    /// Active story index into `model.stories`; drawing happens at its elevation.
    pub active_story: usize,
    /// Node id where a chained frame draw currently starts (None = idle).
    pub draw_frame_start: Option<usize>,
    /// Pending points for the area being drawn: anchor, polygon vertices, or arc centre/start/end.
    pub area_points: Vec<(f64, f64)>,
    pub area_shape: AreaShape,
    /// Tessellation vertices for circles, ellipses and arcs.
    pub area_segments: u32,
    /// Base node of the wall panel chain (None = idle).
    pub draw_wall_start: Option<usize>,
    pub wall_height: f64,
    pub cursor_preview: Option<(f64, f64)>,
    pub selected_nodes: HashSet<usize>,
    pub selected_elements: HashSet<usize>,
    pub selected_areas: HashSet<usize>,
    pub drag_select_start: Option<egui::Pos2>,
    pub results: Option<AnalysisResults>,
    pub nodal_loads: HashMap<usize, [f64; 3]>,
    pub load_value: [f64; 3],
    pub show_grid: bool,
    pub show_loads: bool,
    /// 3D load symbols for frame loads (under the `show_loads` master switch).
    pub show_frame_loads: bool,
    /// 3D load symbols for area pressures (under the `show_loads` master switch).
    pub show_area_loads: bool,
    /// Size multiplier for the 3D load symbols.
    pub load_symbol_scale: f32,
    pub show_deformed: bool,
    pub defl_auto: bool,
    pub defl_ampl: f64,
    pub bottom_tab: BottomTab,
    pub table_filter: String,
    /// Column and direction of the active table's sort; `None` keeps model order.
    pub table_sort: Option<(usize, bool)>,
    /// One filter string per visible table column.
    pub table_filters: Vec<String>,
    pub grid_dialog_open: bool,
    pub grid_spacing: f64,
    pub grid_nx: i32,
    pub grid_ny: i32,
    pub grid_cylindrical: bool,
    pub cyl_center_x: f64,
    pub cyl_center_y: f64,
    pub cyl_ring_spacing: f64,
    pub cyl_rings: i32,
    pub cyl_angle_step: f64,
    pub cyl_angle_count: i32,
    pub cyl_start_angle: f64,
    pub grid_x_labels: Vec<String>,
    pub grid_y_labels: Vec<String>,
    // -- Grid System Data dialog --------------------------------------------
    pub grid_name: String,
    pub grid_origin_x: f64,
    pub grid_origin_y: f64,
    pub grid_rotation_deg: f64,
    /// Bubble radius in millimetres, as displayed.
    pub grid_bubble_mm: f64,
    /// True when the grid tables list spacings rather than ordinates.
    pub grid_display_as_spacing: bool,
    /// Grid-local ordinates edited by the dialog.
    pub grid_draft_xs: Vec<f64>,
    pub grid_draft_ys: Vec<f64>,
    pub grid_draft_x_visible: Vec<bool>,
    pub grid_draft_y_visible: Vec<bool>,
    pub grid_draft_x_bubble: Vec<BubbleLoc>,
    pub grid_draft_y_bubble: Vec<BubbleLoc>,
    pub cyl_ring_labels: Vec<String>,
    pub cyl_radial_labels: Vec<String>,
    pub ref_planes: Vec<RefPlane>,
    pub ref_dialog_open: bool,
    pub ref_draft: Vec<RefPlane>,
    pub align_dialog_open: bool,
    pub align_axis: AlignAxis,
    pub align_value: f64,
    pub align_average: bool,
    pub grid_members_dialog_open: bool,
    pub grid_members_frames: bool,
    pub grid_members_walls: bool,
    pub grid_members_wall_height: f64,
    pub grid_members_x_lines: bool,
    pub grid_members_y_lines: bool,
    pub grid_members_rings: bool,
    pub grid_members_radials: bool,
    pub groups: Vec<Group>,
    pub group_dialog_open: bool,
    pub group_name: String,
    pub group_choice: usize,
    pub select_property_dialog_open: bool,
    pub select_scope: SelectScope,
    pub select_active_story_only: bool,
    pub select_material: usize,
    pub select_section: usize,
    pub area_section_dialog_open: bool,
    pub area_section_kind: AreaKind,
    pub area_section_thickness: f64,
    pub area_section_material: usize,
    pub display_options_open: bool,
    pub define_materials_dialog_open: bool,
    pub define_frame_sections_dialog_open: bool,
    pub define_load_patterns_dialog_open: bool,
    pub define_load_cases_dialog_open: bool,
    pub define_combos_dialog_open: bool,
    pub preferences_dialog_open: bool,
    pub load_pattern_draft: Vec<LoadPattern>,
    pub load_pattern_new_name: String,
    pub load_pattern_new_type: LoadType,
    pub frame_load_dialog_open: bool,
    /// Frame end releases / partial fixity assignment dialog.
    pub frame_release_dialog_open: bool,
    pub release_draft: [bool; 12],
    pub fixity_draft: [f64; 12],
    /// Joint support springs assignment dialog.
    pub joint_springs_dialog_open: bool,
    pub spring_draft: [f64; 6],
    /// Temperature / strain load assignment dialog.
    pub temperature_dialog_open: bool,
    pub temperature_delta: f64,
    pub temperature_strain: f64,    pub temperature_case: Option<usize>,

    /// Diaphragm assignment dialog.
    pub diaphragm_dialog_open: bool,
    pub diaphragm_release_draft: [bool; 12],

    // -- phase 4/5: analysis, results, design and reports -------------------
    pub export_e2k_dialog_open: bool,
    /// What the last E2K `.e2k` import read, shown in a report window.
    pub import_e2k_report: Option<openstruct::e2k::E2kImportReport>,
    pub e2k_report_dialog_open: bool,
    pub define_diaphragms_dialog_open: bool,
    pub diaphragm_draft: Vec<Diaphragm>,
    pub assign_diaphragm_dialog_open: bool,
    pub assign_diaphragm_choice: usize,
    pub analysis_options_dialog_open: bool,
    pub check_before_solve: bool,
    pub solve_combinations: bool,
    pub auto_deformed: bool,
    pub model_check_dialog_open: bool,
    pub model_check_report: Option<CheckReport>,
    pub results_setup_dialog_open: bool,
    pub story_results_dialog_open: bool,
    pub frame_diagrams_dialog_open: bool,
    pub diagram_component: usize,
    pub area_contours_dialog_open: bool,
    /// The code the steel and concrete design windows check to.
    pub design_code: openstruct::design::codes::DesignCode,
    /// Concrete design code, kept separate from the steel code with both.
    pub concrete_design_code: openstruct::design::codes::DesignCode,
    // -- New Model dialog ---------------------------------------------------
    pub new_model_open: bool,
    pub nm_units: UnitSystem,
    pub nm_region: usize,
    pub nm_steel_db: usize,
    pub nm_steel_code: openstruct::design::codes::DesignCode,
    pub nm_concrete_code: openstruct::design::codes::DesignCode,
    pub nm_nx: usize,
    pub nm_ny: usize,
    pub nm_dx: f64,
    pub nm_dy: f64,
    pub nm_stories: usize,
    pub nm_typical_h: f64,
    pub nm_base_h: f64,
    pub nm_template: NewModelTemplate,
    pub steel_design_dialog_open: bool,
    pub concrete_design_dialog_open: bool,
    pub concrete_strength: ConcreteStrength,
    pub composite_design_dialog_open: bool,
    pub composite_slab_width: f64,
    pub composite_slab_thickness: f64,
    pub shear_wall_design_dialog_open: bool,
    pub detailing_dialog_open: bool,
    pub reports_dialog_open: bool,
    pub report_title: String,
    pub report_include_design: bool,
    pub report_html: Option<String>,
    pub documentation_dialog_open: bool,
    pub verification_dialog_open: bool,
    pub verification_rows: Vec<(String, f64, f64, bool)>,

    pub frame_load_direction: LoadDirection,
    pub frame_load_axis: usize,
    pub frame_load_magnitude: String,
    pub frame_load_projected: bool,
    pub frame_load_case: Option<usize>,
    pub area_load_dialog_open: bool,
    pub area_load_direction: LoadDirection,
    pub area_load_pressure: String,
    pub area_load_case: Option<usize>,
    pub material_draft: Vec<(usize, Material)>,
    pub material_selected: usize,
    /// Index of the material open in the Material Property Data editor.
    pub material_editor: Option<usize>,
    /// Working copy edited by the Material Property Data dialog.
    pub material_edit: Option<Material>,
    /// True when density is entered as mass density (kg/m³), false for weight (kN/m³).
    pub material_mass_input: bool,
    pub material_show_design: bool,
    pub material_show_damping: bool,
    // -- Add New Material ---------------------------------------------------
    pub new_material_open: bool,
    pub new_material_region: usize,
    pub new_material_type: openstruct::materials::MaterialType,
    pub new_material_standard: usize,
    pub new_material_grade: usize,
    pub section_draft: Vec<SectionDraft>,
    pub section_new_kind: SectionKind,
    pub section_designer_open: bool,
    /// Draft row the designer writes into; `None` appends a new row.
    pub designer_target: Option<usize>,
    pub designer_parts: Vec<openstruct::sections::DesignerPart>,
    pub designer_new_kind: DesignerPartKind,
    pub designer_drag_start: Option<(f64, f64)>,
    /// Half-width of the designer canvas view, in metres.
    pub designer_view: f64,
    pub designer_selected: Option<usize>,
    /// Polygon vertices being drawn (still open until finished).
    pub designer_pending: Vec<(f64, f64)>,
    /// Hole vertices being drawn for the selected polygon part.
    pub designer_pending_hole: Vec<(f64, f64)>,
    pub designer_hole_mode: bool,
    /// Vertex handle being dragged in the designer: part, hole (None = outer ring), vertex.
    pub designer_drag_vertex: Option<(usize, Option<usize>, usize)>,
    /// Vertex selected for keyboard nudging.
    pub designer_vertex: Option<(usize, Option<usize>, usize)>,
    pub load_case_draft: Vec<LoadCase>,
    pub load_case_new_name: String,
    pub combo_draft: Vec<LoadCombination>,
    pub combo_new_name: String,
    pub combo_new_equation: String,
    pub combo_results: Vec<openstruct::analysis::CombinationResult>,
    /// Which result set the Displacements/Reactions/Forces tables show:
    /// `None` is the static first-case analysis, `Some(i)` is combination `i`.
    pub result_source: Option<usize>,
    /// Per-member design envelope across every evaluated combination.
    pub member_envelopes: Vec<openstruct::analysis::MemberEnvelope>,
    pub typed_coord_dialog_open: bool,
    pub typed_coord_u: f64,
    pub typed_coord_v: f64,
    pub typed_coord_u2: f64,
    pub typed_coord_v2: f64,
    pub quick_draw: bool,
    pub show_joints: bool,
    pub show_frames: bool,
    pub show_areas: bool,
    pub show_labels: bool,
    pub show_supports: bool,
    pub show_dimensions: bool,
    pub show_ref_planes: bool,
    /// Some(plane) when drawing uses a reference plane instead of the active story.
    pub work_plane: Option<WorkPlane>,
    pub story_dialog_open: bool,
    pub story_draft: Vec<Story>,
    pub master_story: Option<usize>,
    pub story_master_draft: Option<usize>,
    pub new_story_name: String,
    pub new_story_elevation: f64,
    /// Cardinal point assigned to newly drawn frames.
    pub cardinal_point: CardinalPoint,
    pub edit_mode: Option<EditMode>,
    pub edit_dx: f64,
    pub edit_dy: f64,
    pub edit_dz: f64,
    pub edit_copies: i32,
    pub edit_axis: MirrorAxis,
    pub edit_pivot: f64,
    pub edit_pivot_y: f64,
    pub edit_angle: f64,
    pub pending_fit: Option<()>,
    pub status_text: String,
    /// Smoothed 3D frame time in milliseconds (display-only performance readout).
    pub frame_ms: f32,
    /// True when the wgpu shaded-section callback was installed successfully.
    pub wgpu_ready: bool,
    pub shade_3d: bool,
    pub clip_3d: bool,
    pub mouse_world: (f64, f64),
    pub undo: Vec<Doc>,
    pub redo: Vec<Doc>,
    pub view_tab: ViewTab,
    pub dark: bool,
    pub units: UnitSystem,
    pub show_explorer: bool,
    pub show_properties: bool,
    pub show_tables: bool,
    pub palette_open: bool,
    pub palette_query: String,
    pub shortcuts_open: bool,
    pub about_open: bool,
    pub file_path: Option<PathBuf>,
    pub dirty: bool,
    /// Screen rectangle the plan canvas occupied on the last frame. Headless
    /// layout tests read it to prove the view does not move under the user.
    pub last_canvas_rect: Option<egui::Rect>,
    /// Rectangles of the two bottom panels, recorded for the same layout tests.
    pub last_status_rect: Option<egui::Rect>,
    pub last_tables_rect: Option<egui::Rect>,
}

impl OpenStructApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let prefs = cc.storage.and_then(|storage| eframe::get_value::<Prefs>(storage, "openstruct.prefs")).unwrap_or_default();
        theme::install(&cc.egui_ctx, prefs.dark);
        let mut app = Self::fresh();
        app.ctx = cc.egui_ctx.clone();
        app.dark = prefs.dark;
        app.units = prefs.units;
        app.show_grid = prefs.show_grid;
        app.show_loads = prefs.show_loads;
        app.show_explorer = prefs.show_explorer;
        app.show_properties = prefs.show_properties;
        app.show_tables = prefs.show_tables;
        app.show_joints = prefs.show_joints;
        app.show_frames = prefs.show_frames;
        app.show_areas = prefs.show_areas;
        app.show_labels = prefs.show_labels;
        app.show_supports = prefs.show_supports;
        app.show_dimensions = prefs.show_dimensions;
        app.show_ref_planes = prefs.show_ref_planes;
        app.show_frame_loads = prefs.show_frame_loads;
        app.show_area_loads = prefs.show_area_loads;
        app.load_symbol_scale = prefs.load_symbol_scale;
        app.wgpu_ready = crate::wgpu3d::install(cc);
        // Opt-in: the painter viewport stays the default until the wgpu pass has
        // been visually checked on the target backend.
        app.shade_3d = false;
        app
    }

    pub(crate) fn fresh() -> Self {
        let (model, grid) = blank_model();
        Self {
            ctx: egui::Context::default(),
            model,
            view: View { center: (10.0, 7.5), scale: 32.0 },
            camera: Camera3D::default(),
            grid,
            tool: Tool::Select,
            active_story: 0,
            draw_frame_start: None,
            area_points: Vec::new(),
            area_shape: AreaShape::Rectangle,
            area_segments: 32,
            draw_wall_start: None,
            wall_height: 3.5,
            cursor_preview: None,
            selected_nodes: HashSet::new(),
            selected_elements: HashSet::new(),
            selected_areas: HashSet::new(),
            drag_select_start: None,
            results: None,
            nodal_loads: HashMap::new(),
            load_value: [0.0, 0.0, -10.0],
            show_grid: true,
            show_loads: true,
            show_frame_loads: true,
            show_area_loads: true,
            load_symbol_scale: 1.0,
            show_deformed: false,
            defl_auto: true,
            defl_ampl: 1000.0,
            bottom_tab: BottomTab::Joints,
            table_filter: String::new(),
            table_sort: None,
            table_filters: Vec::new(),
            grid_dialog_open: false,
            grid_spacing: 5.0,
            grid_nx: 4,
            grid_ny: 3,
            grid_cylindrical: false,
            cyl_center_x: 0.0,
            cyl_center_y: 0.0,
            cyl_ring_spacing: 5.0,
            cyl_rings: 3,
            cyl_angle_step: 30.0,
            cyl_angle_count: 12,
            cyl_start_angle: 0.0,
            grid_x_labels: Vec::new(),
            grid_y_labels: Vec::new(),
            grid_name: crate::canvas::default_grid_name(),
            grid_origin_x: 0.0,
            grid_origin_y: 0.0,
            grid_rotation_deg: 0.0,
            grid_bubble_mm: crate::canvas::default_bubble_size() * 1000.0,
            grid_display_as_spacing: false,
            grid_draft_xs: Vec::new(),
            grid_draft_x_visible: Vec::new(),
            grid_draft_y_visible: Vec::new(),
            grid_draft_x_bubble: Vec::new(),
            grid_draft_y_bubble: Vec::new(),
            grid_draft_ys: Vec::new(),
            cyl_ring_labels: Vec::new(),
            cyl_radial_labels: Vec::new(),
            ref_planes: Vec::new(),
            ref_dialog_open: false,
            ref_draft: Vec::new(),
            align_dialog_open: false,
            align_axis: AlignAxis::X,
            align_value: 0.0,
            align_average: true,
            grid_members_dialog_open: false,
            grid_members_frames: true,
            grid_members_walls: false,
            grid_members_wall_height: 3.5,
            grid_members_x_lines: true,
            grid_members_y_lines: true,
            grid_members_rings: true,
            grid_members_radials: true,
            groups: Vec::new(),
            group_dialog_open: false,
            group_name: String::new(),
            group_choice: 0,
            select_property_dialog_open: false,
            select_scope: SelectScope::All,
            select_active_story_only: false,
            select_material: 0,
            select_section: 0,
            area_section_dialog_open: false,
            area_section_kind: AreaKind::Slab,
            area_section_thickness: 0.2,
            area_section_material: 1,
            display_options_open: false,
            define_materials_dialog_open: false,
            define_frame_sections_dialog_open: false,
            define_load_patterns_dialog_open: false,
            define_load_cases_dialog_open: false,
            define_combos_dialog_open: false,
            preferences_dialog_open: false,
            load_pattern_draft: Vec::new(),
            load_pattern_new_name: String::new(),
            load_pattern_new_type: LoadType::Dead,
            frame_load_dialog_open: false,
            frame_release_dialog_open: false,
            release_draft: [false; 12],
            fixity_draft: [0.0; 12],
            joint_springs_dialog_open: false,
            spring_draft: [0.0; 6],
            diaphragm_dialog_open: false,
            diaphragm_release_draft: [false; 12],
            export_e2k_dialog_open: false,
            import_e2k_report: None,
            e2k_report_dialog_open: false,
            define_diaphragms_dialog_open: false,
            diaphragm_draft: Vec::new(),
            assign_diaphragm_dialog_open: false,
            assign_diaphragm_choice: 1,
            analysis_options_dialog_open: false,
            check_before_solve: true,
            solve_combinations: true,
            auto_deformed: true,
            model_check_dialog_open: false,
            model_check_report: None,
            results_setup_dialog_open: false,
            story_results_dialog_open: false,
            frame_diagrams_dialog_open: false,
            diagram_component: 0,
            area_contours_dialog_open: false,
            design_code: openstruct::design::codes::DesignCode::AiscAci,
            concrete_design_code: openstruct::design::codes::DesignCode::AiscAci,
            new_model_open: false,
            nm_units: UnitSystem::Metric,
            nm_region: 0,
            nm_steel_db: 0,
            nm_steel_code: openstruct::design::codes::DesignCode::AiscAci,
            nm_concrete_code: openstruct::design::codes::DesignCode::AiscAci,
            nm_nx: 3,
            nm_ny: 3,
            nm_dx: 5.0,
            nm_dy: 5.0,
            nm_stories: 3,
            nm_typical_h: 3.5,
            nm_base_h: 3.5,
            nm_template: NewModelTemplate::Building,
            steel_design_dialog_open: false,
            concrete_design_dialog_open: false,
            concrete_strength: ConcreteStrength::C4000,
            composite_design_dialog_open: false,
            composite_slab_width: 1.5,
            composite_slab_thickness: 0.12,
            shear_wall_design_dialog_open: false,
            detailing_dialog_open: false,
            reports_dialog_open: false,
            report_title: "OpenStruct Structural Report".to_string(),
            report_include_design: true,
            report_html: None,
            documentation_dialog_open: false,
            verification_dialog_open: false,
            verification_rows: Vec::new(),
            temperature_dialog_open: false,
            temperature_delta: 20.0,
            temperature_strain: 0.0,
            temperature_case: None,
            frame_load_direction: LoadDirection::Global,
            frame_load_axis: 2,
            frame_load_magnitude: "10".to_string(),
            frame_load_projected: false,
            frame_load_case: None,
            area_load_dialog_open: false,
            area_load_direction: LoadDirection::Global,
            area_load_pressure: "2".to_string(),
            area_load_case: None,
            material_draft: Vec::new(),
            material_selected: 0,
            material_editor: None,
            material_edit: None,
            material_mass_input: true,
            material_show_design: false,
            material_show_damping: false,
            new_material_open: false,
            new_material_region: 0,
            new_material_type: openstruct::materials::MaterialType::Steel,
            new_material_standard: 0,
            new_material_grade: 0,
            section_draft: Vec::new(),
            section_new_kind: SectionKind::Rectangular,
            section_designer_open: false,
            designer_target: None,
            designer_parts: Vec::new(),
            designer_new_kind: DesignerPartKind::Rectangle,
            designer_drag_start: None,
            designer_view: 0.6,
            designer_selected: None,
            designer_pending: Vec::new(),
            designer_pending_hole: Vec::new(),
            designer_hole_mode: false,
            designer_drag_vertex: None,
            designer_vertex: None,
            load_case_draft: Vec::new(),
            load_case_new_name: String::new(),
            combo_draft: Vec::new(),
            combo_new_name: String::new(),
            combo_new_equation: String::new(),
            combo_results: Vec::new(),
            result_source: None,
            member_envelopes: Vec::new(),
            typed_coord_dialog_open: false,
            typed_coord_u: 0.0,
            typed_coord_v: 0.0,
            typed_coord_u2: 5.0,
            typed_coord_v2: 0.0,
            quick_draw: false,
            show_joints: true,
            show_frames: true,
            show_areas: true,
            show_labels: true,
            show_supports: true,
            show_dimensions: true,
            show_ref_planes: true,
            work_plane: None,
            story_dialog_open: false,
            story_draft: Vec::new(),
            master_story: None,
            story_master_draft: None,
            new_story_name: String::new(),
            new_story_elevation: 0.0,
            cardinal_point: CardinalPoint::Center,
            edit_mode: None,
            edit_dx: 0.0,
            edit_dy: 0.0,
            edit_dz: 0.0,
            edit_copies: 1,
            edit_axis: MirrorAxis::X,
            edit_pivot: 0.0,
            edit_pivot_y: 0.0,
            edit_angle: 90.0,
            pending_fit: None,
            status_text: "Ready — draw joints (2) and frames (3) on the active story, assign supports and loads, then Run (F5).".to_string(),
            frame_ms: 0.0,
            wgpu_ready: false,
            shade_3d: false,
            clip_3d: false,
            mouse_world: (0.0, 0.0),
            undo: Vec::new(),
            redo: Vec::new(),
            view_tab: ViewTab::Plan,
            dark: true,
            units: UnitSystem::Metric,
            show_explorer: true,
            show_properties: true,
            show_tables: true,
            palette_open: false,
            palette_query: String::new(),
            shortcuts_open: false,
            about_open: false,
            file_path: None,
            dirty: false,
            last_canvas_rect: None,
            last_status_rect: None,
            last_tables_rect: None,
        }
    }

    // -- stories -------------------------------------------------------------

    pub fn active_elevation(&self) -> f64 {
        self.model.stories.get(self.active_story).map(|story| story.elevation).unwrap_or(0.0)
    }

    pub fn active_story_name(&self) -> String {
        self.model.stories.get(self.active_story).map(|story| story.name.clone()).unwrap_or_else(|| "Story1".to_string())
    }

    /// True when a reference plane (of any orientation) is the current work plane.
    pub fn work_plane_active(&self) -> bool {
        self.work_plane.is_some_and(|work| self.ref_planes.iter().any(|plane| plane.axis == work.axis && (plane.offset - work.offset).abs() < 1e-9))
    }

    /// The plane used for drawing, picking and snapping: the selected reference
    /// plane, or the horizontal plane of the active story.
    pub fn active_work_plane(&self) -> WorkPlane {
        match self.work_plane.filter(|_| self.work_plane_active()) {
            Some(work) => work,
            None => WorkPlane::new(RefPlaneAxis::Z, self.active_elevation()),
        }
    }

    pub fn work_plane_horizontal(&self) -> bool {
        self.active_work_plane().is_horizontal()
    }

    /// Elevation used by elevation-only operations (slabs, grid members).
    pub fn work_elevation(&self) -> f64 {
        match self.active_work_plane() {
            WorkPlane { axis: RefPlaneAxis::Z, offset } => offset,
            _ => self.active_elevation(),
        }
    }

    pub fn work_plane_name(&self) -> String {
        if !self.work_plane_active() {
            return self.active_story_name();
        }
        let work = self.active_work_plane();
        self.ref_planes
            .iter()
            .find(|plane| plane.axis == work.axis && (plane.offset - work.offset).abs() < 1e-9)
            .map(|plane| plane.name.clone())
            .unwrap_or_else(|| format!("{} = {:.2} m", work.axis.label(), work.offset))
    }

    /// Short description used by the toolbar/status bar, e.g. "X = 2.50 m".
    pub fn work_plane_description(&self) -> String {
        let work = self.active_work_plane();
        format!("{} = {:.2} m", work.axis.label(), work.offset)
    }

    pub fn plane_world(&self, u: f64, v: f64) -> [f64; 3] {
        self.active_work_plane().to_world(u, v)
    }

    /// Reuse or create a joint at plane-local (u, v) on the active work plane.
    pub fn ensure_node_on_plane(&mut self, u: f64, v: f64) -> usize {
        let [x, y, z] = self.plane_world(u, v);
        self.ensure_node_at(x, y, z)
    }

    /// Display colour stored on the Story Manager row for a level.
    pub fn story_color(&self, index: usize) -> egui::Color32 {
        let [r, g, b] = self
            .model
            .stories
            .get(index)
            .map(|story| story.color)
            .unwrap_or_else(openstruct::model::default_story_color);
        egui::Color32::from_rgb(r, g, b)
    }

    /// Display colour of the active level, used to paint the plan view.
    pub fn active_story_color(&self) -> egui::Color32 {
        self.story_color(self.active_story)
    }

    pub fn story_name_at(&self, z: f64) -> String {
        match self.model.story_index_at(z, 1e-6) {
            Some(index) => self.model.stories[index].name.clone(),
            None => format!("z = {z:.2} m"),
        }
    }

    /// Joints are drawn and picked on the current work plane (any orientation).
    pub fn node_on_active_story(&self, node: &Node) -> bool {
        self.active_work_plane().contains([node.x, node.y, node.z], 1e-6)
    }

    /// An area belongs to the work plane when every perimeter joint does.
    pub fn area_on_active_story(&self, area: &Area) -> bool {
        !area.node_ids.is_empty() && area.node_ids.iter().all(|id| self.model.nodes.get(id).is_some_and(|node| self.node_on_active_story(node)))
    }

    /// Reuse a joint at this exact position, otherwise create one.
    pub fn ensure_node_at(&mut self, x: f64, y: f64, z: f64) -> usize {
        for (id, node) in &self.model.nodes {
            if (node.x - x).abs() < 1e-6 && (node.y - y).abs() < 1e-6 && (node.z - z).abs() < 1e-6 {
                return *id;
            }
        }
        self.model.add_node(x, y, z)
    }

    /// Handle one click of the area tool, creating the area once the shape is complete.
    pub fn push_area_point(&mut self, point: (f64, f64)) {
        match self.area_shape {
            AreaShape::Polygon => {
                self.area_points.push(point);
                self.status_text =
                    format!("Polygon vertex {} at ({:.3}, {:.3}); double-click or right-click to close.", self.area_points.len(), point.0, point.1);
            }
            AreaShape::Arc => {
                self.area_points.push(point);
                match self.area_points.len() {
                    1 => self.status_text = "Arc centre set; click one end of the curve.".to_string(),
                    2 => self.status_text = "Arc start set; click the other end of the curve.".to_string(),
                    _ => {
                        let points = std::mem::take(&mut self.area_points);
                        self.finish_arc(&points);
                    }
                }
            }
            AreaShape::Rectangle | AreaShape::Circle | AreaShape::Ellipse => {
                if self.area_points.is_empty() {
                    self.area_points.push(point);
                    self.status_text = match self.area_shape {
                        AreaShape::Rectangle => "First corner set; click the opposite corner.".to_string(),
                        AreaShape::Circle => "Centre set; click a point on the circle.".to_string(),
                        _ => "Centre set; click a corner of the bounding box.".to_string(),
                    };
                } else {
                    let anchor = self.area_points[0];
                    self.area_points.clear();
                    let z = self.work_elevation();
                    let (outline, curve) = match self.area_shape {
                        AreaShape::Rectangle => (shapes::rectangle(anchor, point), None),
                        AreaShape::Circle => {
                            let radius = ((point.0 - anchor.0).powi(2) + (point.1 - anchor.1).powi(2)).sqrt();
                            (
                                shapes::ellipse(anchor, radius, radius, self.area_segments as usize),
                                Some(AreaCurve::ellipse([anchor.0, anchor.1, z], radius, radius)),
                            )
                        }
                        _ => {
                            let rx = (point.0 - anchor.0).abs();
                            let ry = (point.1 - anchor.1).abs();
                            (
                                shapes::ellipse(anchor, rx, ry, self.area_segments as usize),
                                Some(AreaCurve::ellipse([anchor.0, anchor.1, z], rx, ry)),
                            )
                        }
                    };
                    self.add_slab_outline_curved(outline, curve);
                }
            }
        }
    }

    /// Close the current polygon with the vertices collected so far.
    pub fn finish_polygon_area(&mut self) -> Option<usize> {
        if self.area_points.len() < 3 {
            self.status_text = "A polygon needs at least three vertices.".to_string();
            return None;
        }
        let points = std::mem::take(&mut self.area_points);
        self.add_slab_outline(points)
    }

    fn finish_arc(&mut self, points: &[(f64, f64)]) {
        let (center, start, end) = (points[0], points[1], points[2]);
        let radius = ((start.0 - center.0).powi(2) + (start.1 - center.1).powi(2)).sqrt();
        if radius < 1e-6 {
            self.status_text = "Arc rejected: the start point is at the centre.".to_string();
            return;
        }
        let start_angle = (start.1 - center.1).atan2(start.0 - center.0);
        let end_angle = (end.1 - center.1).atan2(end.0 - center.0);
        let mut sweep = (end_angle - start_angle).rem_euclid(std::f64::consts::TAU);
        if sweep < 1e-3 {
            sweep = std::f64::consts::TAU;
        }
        let outline = shapes::arc(center, radius, radius, start_angle, start_angle + sweep, self.area_segments as usize);
        let z = self.work_elevation();
        self.add_slab_outline_curved(
            outline,
            Some(AreaCurve::arc([center.0, center.1, z], radius, radius, start_angle, sweep)),
        );
    }

    /// Add a slab from a closed plan outline, reusing coincident joints.
    pub fn add_slab_outline(&mut self, outline: Vec<(f64, f64)>) -> Option<usize> {
        self.add_slab_outline_curved(outline, None)
    }

    fn add_slab_outline_curved(&mut self, outline: Vec<(f64, f64)>, curve: Option<AreaCurve>) -> Option<usize> {
        let mut points: Vec<(f64, f64)> = Vec::with_capacity(outline.len());
        for point in outline {
            let duplicate = points.last().is_some_and(|last| (last.0 - point.0).abs() < 1e-6 && (last.1 - point.1).abs() < 1e-6);
            if !duplicate {
                points.push(point);
            }
        }
        if points.len() >= 2 {
            let first = points[0];
            let last = points[points.len() - 1];
            if (first.0 - last.0).abs() < 1e-6 && (first.1 - last.1).abs() < 1e-6 {
                points.pop();
            }
        }
        if points.len() < 3 {
            self.status_text = "Area rejected: the outline collapsed onto too few distinct points.".to_string();
            return None;
        }
        if !self.work_plane_horizontal() {
            self.status_text = "Areas require a horizontal work plane; select a story or a Z plane.".to_string();
            return None;
        }
        let z = self.work_elevation();
        self.push_undo();
        let mut ids: Vec<usize> = Vec::with_capacity(points.len());
        for (x, y) in &points {
            let node = self.ensure_node_at(*x, *y, z);
            if !ids.contains(&node) {
                ids.push(node);
            }
        }
        if ids.len() < 3 {
            self.status_text = "Area rejected: the outline shares too few joints.".to_string();
            return None;
        }
        let (mut min_x, mut max_x, mut min_y, mut max_y) = (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
        for (x, y) in &points {
            min_x = min_x.min(*x);
            max_x = max_x.max(*x);
            min_y = min_y.min(*y);
            max_y = max_y.max(*y);
        }
        let material = self.default_material_id();
        let mut area = Area::new(ids, AreaKind::Slab, material, 0.2);
        area.curve = curve;
        let id = self.model.add_area(area);
        self.selected_nodes.clear();
        self.selected_elements.clear();
        self.selected_areas.clear();
        self.selected_areas.insert(id);
        let vertices = self.model.areas.get(&id).map(|area| area.node_ids.len()).unwrap_or(0);
        self.status_text = format!(
            "Slab A{id} ({}) created on {} ({vertices} vertices, {:.2} × {:.2} m). Areas are display-only until the shell solver.",
            self.area_shape.label(),
            self.work_plane_name(),
            max_x - min_x,
            max_y - min_y
        );
        Some(id)
    }

    /// Add a vertical wall panel between two base joints on the active story.
    pub fn add_wall_panel(&mut self, base_a: usize, base_b: usize) -> Option<usize> {
        if base_a == base_b {
            return None;
        }
        let height = self.wall_height.max(0.01);
        let (ax, ay, z) = {
            let node = self.model.nodes.get(&base_a)?;
            (node.x, node.y, node.z)
        };
        let (bx, by) = {
            let node = self.model.nodes.get(&base_b)?;
            (node.x, node.y)
        };
        let duplicate = self.model.areas.values().any(|area| {
            area.kind == AreaKind::Wall
                && area.node_ids.len() == 4
                && area.node_ids.contains(&base_a)
                && area.node_ids.contains(&base_b)
                && area.thickness > 0.0
        });
        if duplicate {
            self.status_text = "A wall already uses those two joints; chain continues.".to_string();
            return None;
        }
        self.push_undo();
        let top_a = self.ensure_node_at(ax, ay, z + height);
        let top_b = self.ensure_node_at(bx, by, z + height);
        if top_a == top_b {
            self.status_text = "Wall rejected: the top joints coincide.".to_string();
            return None;
        }
        let material = self.default_material_id();
        let id = self.model.add_area(Area::new(vec![base_a, base_b, top_b, top_a], AreaKind::Wall, material, 0.2));
        self.selected_nodes.clear();
        self.selected_elements.clear();
        self.selected_areas.clear();
        self.selected_areas.insert(id);
        let length = ((bx - ax).powi(2) + (by - ay).powi(2)).sqrt();
        self.status_text =
            format!("Wall A{id} created on {} ({length:.2} m long × {height:.2} m high). Continue clicking to chain panels.", self.work_plane_name());
        Some(id)
    }

    /// Height of the master story to the level above it; the typical height.
    pub fn master_story_height(&self) -> f64 {
        let Some(index) = self.master_story else {
            return 3.5;
        };
        let Some(master) = self.model.stories.get(index) else {
            return 3.5;
        };
        let mut above: Option<f64> = None;
        for story in &self.model.stories {
            if story.elevation > master.elevation + 1e-9 && above.map_or(true, |value| story.elevation < value) {
                above = Some(story.elevation);
            }
        }
        above.map(|value| value - master.elevation).unwrap_or(3.5)
    }

    pub fn default_story_height(&self) -> f64 {
        let elevation = self.active_elevation();
        let mut above: Option<f64> = None;
        for story in &self.model.stories {
            if story.elevation > elevation + 1e-9 && above.map_or(true, |value| story.elevation < value) {
                above = Some(story.elevation);
            }
        }
        above.map(|value| value - elevation).unwrap_or_else(|| self.master_story_height())
    }

    pub fn set_view_preset(&mut self, preset: ViewPreset) {
        self.camera.apply_preset(preset);
        self.view_tab = ViewTab::View3D;
    }

    pub fn open_story_dialog(&mut self) {
        self.story_draft = self.model.stories.clone();
        if self.story_draft.is_empty() {
            self.story_draft.push(Story::new("Story1", 0.0));
        }
        self.story_master_draft = self.master_story;
        self.new_story_name = format!("Story{}", self.story_draft.len() + 1);
        let top = self.story_draft.iter().map(|story| story.elevation).fold(0.0_f64, f64::max);
        self.new_story_elevation = top + self.master_story_height();
        self.story_dialog_open = true;
    }

    pub fn apply_story_draft(&mut self) {
        let master_name = self
            .story_master_draft
            .and_then(|index| self.story_draft.get(index))
            .map(|story| story.name.clone());
        let before: Vec<String> = self
            .story_draft
            .iter()
            .filter(|story| !story.name.trim().is_empty())
            .map(|story| story.name.clone())
            .collect();
        let mut draft: Vec<Story> = self.story_draft.iter().filter(|story| !story.name.trim().is_empty()).cloned().collect();
        if draft.is_empty() {
            draft.push(Story::new("Story1", 0.0));
        }
        for index in 1..draft.len() {
            let base = draft[index].name.clone();
            let mut candidate = base.clone();
            let mut suffix = 1;
            while draft.iter().take(index).any(|story| story.name == candidate) {
                candidate = format!("{base}-{suffix}");
                suffix += 1;
            }
            draft[index].name = candidate;
        }
        // Similar-to references point at stories by name, so follow a rename and
        // drop references to stories that were deleted.
        let current_names: Vec<String> = before
            .iter()
            .enumerate()
            .map(|(position, name)| draft.get(position).map(|story| story.name.clone()).unwrap_or_else(|| name.clone()))
            .collect();
        for story in draft.iter_mut() {
            let Some(reference) = story.similar_to.clone() else {
                continue;
            };
            let renamed = before
                .iter()
                .position(|name| *name == reference)
                .map(|position| current_names[position].clone());
            story.similar_to = match renamed {
                Some(name) if name != story.name => Some(name),
                _ => None,
            };
        }
        let active_name = self.active_story_name();
        self.push_undo();
        self.model.stories = draft;
        self.master_story = master_name.and_then(|name| self.model.stories.iter().position(|story| story.name == name));
        self.story_master_draft = self.master_story;
        self.active_story = self.model.stories.iter().position(|story| story.name == active_name).unwrap_or(0).min(self.model.stories.len().saturating_sub(1));
        self.story_dialog_open = false;
        self.story_draft = self.model.stories.clone();
        self.pending_fit = Some(());
        self.status_text = format!("Stories updated ({} levels).", self.model.stories.len());
    }

    // -- edit dialogs --------------------------------------------------------

    pub fn open_edit_dialog(&mut self, mode: EditMode) {
        if self.selected_nodes.is_empty() && self.selected_elements.is_empty() && self.selected_areas.is_empty() {
            self.status_text = "Select joints, frames and/or areas first.".to_string();
            return;
        }
        self.edit_mode = Some(mode);
        self.edit_dx = 0.0;
        self.edit_dy = 0.0;
        self.edit_dz = 0.0;
        self.edit_copies = 1;
        self.edit_axis = MirrorAxis::X;
        self.edit_pivot = self.selection_center_x();
        self.edit_pivot_y = self.selection_center_y();
        self.edit_angle = 90.0;
        if matches!(mode, EditMode::Copy | EditMode::Replicate) {
            self.edit_dz = self.default_story_height();
        }
    }

    fn selection_center_y(&self) -> f64 {
        let mut sum = 0.0;
        let mut count = 0.0;
        for id in self.selection_node_ids() {
            if let Some(node) = self.model.nodes.get(&id) {
                sum += node.y;
                count += 1.0;
            }
        }
        if count > 0.0 { sum / count } else { 0.0 }
    }

    fn selection_center_x(&self) -> f64 {
        let mut sum = 0.0;
        let mut count = 0.0;
        for id in self.selection_node_ids() {
            if let Some(node) = self.model.nodes.get(&id) {
                sum += node.x;
                count += 1.0;
            }
        }
        if count > 0.0 { sum / count } else { 0.0 }
    }

    /// Selected joints plus every joint attached to a selected frame or area.
    pub fn selection_node_ids(&self) -> HashSet<usize> {
        let mut nodes = self.selected_nodes.clone();
        for &id in &self.selected_elements {
            if let Some(element) = self.model.elements.get(&id) {
                nodes.insert(element.node_i);
                nodes.insert(element.node_j);
            }
        }
        for &id in &self.selected_areas {
            if let Some(area) = self.model.areas.get(&id) {
                nodes.extend(area.node_ids.iter().copied());
            }
        }
        nodes
    }

    pub fn apply_edit_dialog(&mut self) {
        let Some(mode) = self.edit_mode.take() else {
            return;
        };
        if self.selected_nodes.is_empty() && self.selected_elements.is_empty() && self.selected_areas.is_empty() {
            self.status_text = "Nothing selected.".to_string();
            return;
        }
        self.push_undo();
        let delta = [self.edit_dx, self.edit_dy, self.edit_dz];
        match mode {
            EditMode::Move => {
                let targets = self.selection_node_ids();
                let moved = edit::move_nodes(&mut self.model, &targets, delta);
                self.sync_curves_translated(&targets, delta);
                self.status_text = format!("Moved {moved} joint(s).");
            }
            EditMode::Copy => {
                let result = edit::copy_selection(&mut self.model, &self.selected_nodes, &self.selected_elements, &self.selected_areas, |x, y, z| {
                    (x + delta[0], y + delta[1], z + delta[2])
                });
                for &(old_id, new_id) in &result.area_pairs {
                    let curve = self.model.areas.get(&old_id).and_then(|area| area.curve.clone());
                    if let Some(mut curve) = curve {
                        curve.translate(delta);
                        if let Some(area) = self.model.areas.get_mut(&new_id) {
                            area.curve = Some(curve);
                        }
                    }
                }
                self.apply_copy_result(result, "Copied");
            }
            EditMode::Replicate => {
                let copies = self.edit_copies.max(1) as usize;
                let result = edit::replicate_linear(&mut self.model, &self.selected_nodes, &self.selected_elements, &self.selected_areas, delta, copies);
                self.apply_copy_result(result, "Replicated");
            }
            EditMode::Mirror => {
                let result = edit::mirror_selection(
                    &mut self.model,
                    &self.selected_nodes,
                    &self.selected_elements,
                    &self.selected_areas,
                    self.edit_axis,
                    self.edit_pivot,
                );
                self.apply_copy_result(result, "Mirrored");
            }
            EditMode::Rotate => {
                let targets = self.selection_node_ids();
                let rotated = edit::rotate_nodes(&mut self.model, &targets, (self.edit_pivot, self.edit_pivot_y), self.edit_angle);
                self.sync_curves_rotated(&targets, (self.edit_pivot, self.edit_pivot_y), self.edit_angle.to_radians());
                self.status_text = format!("Rotated {rotated} joint(s) by {:.2}°.", self.edit_angle);
            }
        }
    }

    fn sync_curves_translated(&mut self, targets: &HashSet<usize>, delta: [f64; 3]) {
        for area in self.model.areas.values_mut() {
            let ids = area.node_ids.clone();
            if ids.len() < 2 || !ids.iter().all(|id| targets.contains(id)) {
                continue;
            }
            if let Some(curve) = area.curve.as_mut() {
                curve.translate(delta);
            }
        }
    }

    fn sync_curves_rotated(&mut self, targets: &HashSet<usize>, pivot: (f64, f64), radians: f64) {
        for area in self.model.areas.values_mut() {
            let ids = area.node_ids.clone();
            if ids.len() < 2 || !ids.iter().all(|id| targets.contains(id)) {
                continue;
            }
            if let Some(curve) = area.curve.as_mut() {
                curve.rotate_z(pivot, radians);
            }
        }
    }

    fn apply_copy_result(&mut self, result: CopyResult, verb: &str) {
        for &(old_id, new_id) in &result.node_pairs {
            if let Some(load) = self.nodal_loads.get(&old_id).copied() {
                self.nodal_loads.insert(new_id, load);
            }
        }
        let nodes = result.new_nodes().len();
        let elements = result.new_elements().len();
        let areas = result.new_areas().len();
        self.selected_nodes = result.new_nodes();
        self.selected_elements = result.new_elements();
        self.selected_areas = result.new_areas();
        self.status_text = format!("{verb} {nodes} joint(s), {elements} frame(s) and {areas} area(s).");
    }

    pub fn split_at_intersections(&mut self) {
        self.push_undo();
        let report = split::split_at_intersections(&mut self.model, 0.001);
        self.results = None;
        self.selected_nodes.clear();
        self.selected_elements.clear();
        self.selected_areas.clear();
        self.draw_frame_start = None;
        self.draw_wall_start = None;
        self.area_points.clear();
        self.status_text = if report.is_empty() {
            "Split at intersections: no crossings found.".to_string()
        } else {
            format!(
                "Split at intersections: {} joint(s) added, {} frame(s) and {} slab(s) split, {} area(s) gained edge joints.",
                report.joints_added, report.frames_split, report.slabs_split, report.areas_updated
            )
        };
    }

    pub fn merge_joints_now(&mut self) {
        self.push_undo();
        let subset = self.selected_nodes.clone();
        let (removed, map) = edit::merge_joints(&mut self.model, &subset, 0.001);
        if removed == 0 {
            self.status_text = "No coincident joints within 1 mm.".to_string();
            return;
        }
        let loads = std::mem::take(&mut self.nodal_loads);
        for (node_id, load) in loads {
            let target = *map.get(&node_id).unwrap_or(&node_id);
            self.nodal_loads.entry(target).or_insert(load);
        }
        self.selected_nodes = self.selected_nodes.iter().map(|id| *map.get(id).unwrap_or(id)).collect();
        self.status_text = format!("Merged {removed} coincident joint(s).");
    }

    pub fn open_align_dialog(&mut self) {
        let nodes = self.selection_node_ids();
        if nodes.is_empty() {
            self.status_text = "Select joints, frames and/or areas to align first.".to_string();
            return;
        }
        let index = self.align_axis.index();
        self.align_value = self.average_coordinate(&nodes, index);
        self.align_average = true;
        self.align_dialog_open = true;
    }

    pub fn average_coordinate(&self, nodes: &HashSet<usize>, index: usize) -> f64 {
        let mut sum = 0.0;
        let mut count = 0.0;
        for id in nodes {
            if let Some(node) = self.model.nodes.get(id) {
                sum += [node.x, node.y, node.z][index.min(2)];
                count += 1.0;
            }
        }
        if count > 0.0 { sum / count } else { 0.0 }
    }

    pub fn apply_align(&mut self) {
        let nodes = self.selection_node_ids();
        if nodes.is_empty() {
            self.status_text = "Select joints, frames and/or areas to align first.".to_string();
            self.align_dialog_open = false;
            return;
        }
        let target = if self.align_average { self.average_coordinate(&nodes, self.align_axis.index()) } else { self.align_value };
        self.push_undo();
        let aligned = edit::align_nodes(&mut self.model, &nodes, self.align_axis, target);
        // Alignment is not a rigid transform, so a preserved ellipse would no
        // longer describe the joints. Fall back to the joint boundary.
        for area in self.model.areas.values_mut() {
            let ids = area.node_ids.clone();
            if area.curve.is_some() && ids.iter().all(|id| nodes.contains(id)) {
                area.curve = None;
            }
        }
        self.results = None;
        self.status_text = format!("Aligned {aligned} joint(s) to {} = {:.3} m.", self.align_axis.label(), target);
        self.align_dialog_open = false;
    }

    pub fn trim_extend_now(&mut self) {
        if self.selected_elements.is_empty() {
            self.status_text = "Select the frames to trim or extend first; unselected frames act as boundaries.".to_string();
            return;
        }
        let selected = self.selected_elements.clone();
        let mut model = self.model.clone();
        let report = edit::trim_extend_to_boundaries(&mut model, &selected, 0.01);
        if report.ends_moved == 0 {
            self.status_text = format!("Trim / Extend: no free frame end reached an unselected frame ({} end(s) skipped).", report.ends_skipped);
            return;
        }
        self.push_undo();
        self.model = model;
        self.results = None;
        self.status_text = format!("Trim / Extend: {} frame end(s) moved to the nearest boundary; {} end(s) skipped.", report.ends_moved, report.ends_skipped);
    }

    fn frame_exists(&self, a: usize, b: usize) -> bool {
        self.model.elements.values().any(|element| (element.node_i == a && element.node_j == b) || (element.node_i == b && element.node_j == a))
    }

    /// Set the cardinal point on every selected frame as one undoable edit.
    pub fn assign_cardinal_to_selection(&mut self, point: CardinalPoint) {
        if self.selected_elements.is_empty() {
            self.status_text = "Select one or more frames first.".to_string();
            return;
        }
        self.push_undo();
        let mut count = 0;
        for id in &self.selected_elements {
            if let Some(element) = self.model.elements.get_mut(id) {
                element.cardinal_point = point;
                count += 1;
            }
        }
        self.status_text = format!("Cardinal point '{}' assigned to {count} selected frame(s).", point.label());
    }

    pub fn create_grid_members(&mut self) {
        if !self.grid_members_frames && !self.grid_members_walls {
            self.status_text = "Choose frames, walls or both before applying.".to_string();
            return;
        }
        if !self.work_plane_horizontal() {
            self.status_text = "Grid members are generated on a horizontal work plane; select a story or a Z plane.".to_string();
            return;
        }
        let selected_lines = if self.grid.cylindrical.is_some() {
            self.grid_members_rings || self.grid_members_radials
        } else {
            self.grid_members_x_lines || self.grid_members_y_lines
        };
        if !selected_lines {
            self.status_text = "Select at least one grid-line direction.".to_string();
            return;
        }
        let segments = self.grid.segments_filtered(self.grid_members_x_lines, self.grid_members_y_lines, self.grid_members_rings, self.grid_members_radials);
        if segments.is_empty() {
            self.status_text = "The selected grid lines have no segments.".to_string();
            return;
        }
        let z = self.work_elevation();
        let material = self.default_material_id();
        let section = if self.grid_members_frames { Some(self.default_section_id()) } else { None };
        let height = self.grid_members_wall_height.max(0.01);
        self.push_undo();
        let mut created_frames = 0;
        let mut created_walls = 0;
        for ((ax, ay), (bx, by)) in segments {
            let length = ((bx - ax).powi(2) + (by - ay).powi(2)).sqrt();
            if length < 1e-6 {
                continue;
            }
            let a = self.ensure_node_at(ax, ay, z);
            let b = self.ensure_node_at(bx, by, z);
            if a == b {
                continue;
            }
            if let Some(section) = section {
                if !self.frame_exists(a, b) {
                    let next_id = self.model.element_counter + 1;
                    let mut element = Element::frame(next_id, a, b, material, section);
                    element.cardinal_point = self.cardinal_point;
                    self.model.add_element(element);
                    created_frames += 1;
                }
            }
            if self.grid_members_walls {
                let exists = self.model.areas.values().any(|area| area.kind == AreaKind::Wall && area.node_ids.contains(&a) && area.node_ids.contains(&b));
                if !exists {
                    let top_a = self.ensure_node_at(ax, ay, z + height);
                    let top_b = self.ensure_node_at(bx, by, z + height);
                    if top_a != top_b {
                        self.model.add_area(Area::new(vec![a, b, top_b, top_a], AreaKind::Wall, material, 0.2));
                        created_walls += 1;
                    }
                }
            }
        }
        self.selected_nodes.clear();
        self.selected_elements.clear();
        self.selected_areas.clear();
        self.status_text =
            format!("Grid members on {}: {created_frames} frame(s) and {created_walls} wall(s) created; duplicates were kept out.", self.work_plane_name());
        self.grid_members_dialog_open = false;
    }

    // -- Phase 2 modelling workflows -----------------------------------------

    /// Create the acceptance-case building in one undoable step.
    /// Open the New Model dialog, seeded from the current grid and stories.
    pub fn open_new_model(&mut self) {
        self.nm_nx = self.grid.xs.len().saturating_sub(1).clamp(1, 40);
        self.nm_ny = self.grid.ys.len().saturating_sub(1).clamp(1, 40);
        self.nm_dx = self.grid.xs.windows(2).next().map(|pair| pair[1] - pair[0]).unwrap_or(5.0);
        self.nm_dy = self.grid.ys.windows(2).next().map(|pair| pair[1] - pair[0]).unwrap_or(5.0);
        self.nm_stories = self.model.stories.len().clamp(1, 80);
        self.nm_typical_h = self.default_story_height();
        self.nm_units = self.units;
        self.nm_steel_code = self.design_code;
        self.nm_concrete_code = self.concrete_design_code;
        self.new_model_open = true;
        self.status_text = "New Model: choose units, region, codes, grid, stories and a template.".to_string();
    }

    /// The 3-storey 3×3 building the old File menu item used to create, as a
    /// preset for the New Model dialog.
    pub fn nm_preset_3x3(&mut self) {
        self.nm_nx = 3;
        self.nm_ny = 3;
        self.nm_dx = 5.0;
        self.nm_dy = 5.0;
        self.nm_stories = 3;
        self.nm_typical_h = 3.5;
        self.nm_base_h = 3.5;
        self.nm_template = NewModelTemplate::Building;
    }

    /// Build the first model from the New Model settings: stories, grid,
    /// region-named materials, default member sizes and the chosen template.
    pub fn build_new_model(&mut self) {
        use openstruct::elements::Element;
        use openstruct::materials::Material;
        use openstruct::sections::{Section, SectionShape};

        self.push_undo();
        let nx = self.nm_nx.clamp(1, 40);
        let ny = self.nm_ny.clamp(1, 40);
        let dx = self.nm_dx.max(0.5);
        let dy = self.nm_dy.max(0.5);
        let stories = self.nm_stories.clamp(1, 80);
        let typical = self.nm_typical_h.max(1.0);
        let base_height = self.nm_base_h.max(1.0);

        // Level elevations: the bottom storey uses its own height.
        let mut level_z = vec![0.0_f64];
        let mut z = 0.0;
        for level in 0..stories {
            z += if level == 0 { base_height } else { typical };
            level_z.push(z);
        }

        let mut model = Model::new("Untitled");
        model.stories = (0..stories)
            .map(|level| Story::new(&format!("Story{}", level + 1), level_z[level]))
            .collect();

        // Materials are named the way the chosen region names them.
        let (steel_name, steel_fy) = match self.nm_region {
            1 => ("A992Fy50", 345.0e6),
            2 => ("S355", 355.0e6),
            _ => ("Fe345", 345.0e6),
        };
        let mut steel = Material::steel();
        steel.name = steel_name.to_string();
        steel.fy = steel_fy;
        let steel_id = model.add_material(steel);

        let (concrete_name, fck) = match self.nm_region {
            1 => ("4000Psi", 27.6e6),
            2 => ("C30/37", 30.0e6),
            _ => ("M30", 30.0e6),
        };
        let mut concrete = Material::concrete();
        concrete.name = concrete_name.to_string();
        concrete.fy = fck;
        model.add_material(concrete);

        // Default member sizes from the chosen steel section database.
        let (database, depth, width, web, flange) = match self.nm_steel_db {
            1 => ("W12X26", 0.31, 0.165, 0.0062, 0.0097),
            2 => ("IPE300", 0.30, 0.15, 0.0071, 0.0107),
            _ => ("ISMB300", 0.30, 0.14, 0.0077, 0.0124),
        };
        let column = Section::from_shape(database, SectionShape::ISection { depth, width, web, flange })
            .unwrap_or_else(|| Section::rectangular("COL 400x400", 0.4, 0.4));
        let column_id = model.add_section(column);
        let beam = Section::from_shape(
            &format!("{database} (beam)"),
            SectionShape::ISection { depth: depth * 0.67, width: width * 0.85, web: web * 0.8, flange: flange * 0.85 },
        )
        .unwrap_or_else(|| Section::rectangular("BM 300x500", 0.3, 0.5));
        let beam_id = model.add_section(beam);

        let side_x = nx + 1;
        let side_y = ny + 1;
        let mut joints: Vec<Vec<Vec<usize>>> = vec![vec![vec![0usize; side_y]; side_x]; level_z.len()];
        if self.nm_template != NewModelTemplate::Blank {
            for (level, elevation) in level_z.iter().enumerate() {
                for i in 0..side_x {
                    for j in 0..side_y {
                        joints[level][i][j] = model.add_node(i as f64 * dx, j as f64 * dy, *elevation);
                    }
                }
            }
        }

        if self.nm_template == NewModelTemplate::Building {
            for level in 0..stories {
                for i in 0..side_x {
                    for j in 0..side_y {
                        model.add_element(Element::frame(1, joints[level][i][j], joints[level + 1][i][j], steel_id, column_id));
                    }
                }
            }
            for level in 1..=stories {
                for i in 0..side_x {
                    for j in 0..side_y.saturating_sub(1) {
                        model.add_element(Element::frame(1, joints[level][i][j], joints[level][i][j + 1], steel_id, beam_id));
                    }
                }
                for i in 0..side_x.saturating_sub(1) {
                    for j in 0..side_y {
                        model.add_element(Element::frame(1, joints[level][i][j], joints[level][i + 1][j], steel_id, beam_id));
                    }
                }
            }
            for i in 0..side_x {
                for j in 0..side_y {
                    if let Some(node) = model.nodes.get_mut(&joints[0][i][j]) {
                        node.fix_all();
                    }
                }
            }
        }

        self.model = model;
        self.grid = GridSpec::from_ordinates(
            (0..side_x).map(|i| i as f64 * dx).collect(),
            (0..side_y).map(|j| j as f64 * dy).collect(),
        );
        self.grid_spacing = dx;
        self.grid_nx = nx as i32;
        self.grid_ny = ny as i32;
        self.grid_x_labels.clear();
        self.grid_y_labels.clear();
        self.units = self.nm_units;
        self.design_code = self.nm_steel_code;
        self.concrete_design_code = self.nm_concrete_code;
        self.results = None;
        self.combo_results.clear();
        self.member_envelopes.clear();
        self.ref_planes.clear();
        self.work_plane = None;
        self.nodal_loads.clear();
        self.groups.clear();
        self.selected_nodes.clear();
        self.selected_elements.clear();
        self.selected_areas.clear();
        self.draw_frame_start = None;
        self.area_points.clear();
        self.draw_wall_start = None;
        self.active_story = 0;
        self.master_story = None;
        self.story_master_draft = None;
        self.camera = Camera3D::default();
        self.show_deformed = false;
        self.view_tab = if self.nm_template == NewModelTemplate::Building { ViewTab::View3D } else { ViewTab::Plan };
        self.file_path = None;
        self.dirty = true;
        self.pending_fit = Some(());
        self.status_text = format!(
            "New model: {stories} stories, {nx} × {ny} bays at {dx:.2} m × {dy:.2} m; {} joints, {} frames. Steel {steel_name}, concrete {concrete_name}, {} / {}.",
            self.model.nodes.len(),
            self.model.elements.len(),
            self.design_code.short_name(),
            self.concrete_design_code.short_name()
        );
    }

    pub fn open_typed_coords(&mut self) {
        let (u, v) = self.mouse_world;
        self.typed_coord_u = (u * 1000.0).round() / 1000.0;
        self.typed_coord_v = (v * 1000.0).round() / 1000.0;
        self.typed_coord_u2 = self.typed_coord_u + 5.0;
        self.typed_coord_v2 = self.typed_coord_v;
        self.typed_coord_dialog_open = true;
        self.status_text = format!("Type plane-local {} / {} coordinates for the active work plane.", self.active_work_plane().axis_labels().0, self.active_work_plane().axis_labels().1);
    }

    pub fn create_typed_joint(&mut self) {
        let (u, v) = (self.typed_coord_u, self.typed_coord_v);
        if !u.is_finite() || !v.is_finite() {
            self.status_text = "Coordinates must be finite numbers.".to_string();
            return;
        }
        self.push_undo();
        let id = self.ensure_node_on_plane(u, v);
        self.selected_nodes.clear();
        self.selected_elements.clear();
        self.selected_areas.clear();
        self.selected_nodes.insert(id);
        self.status_text = format!("Joint N{id} created at typed ({u:.3}, {v:.3}) on {}.", self.work_plane_name());
    }

    pub fn create_typed_frame(&mut self) {
        let (u1, v1, u2, v2) = (self.typed_coord_u, self.typed_coord_v, self.typed_coord_u2, self.typed_coord_v2);
        if [u1, v1, u2, v2].iter().any(|value| !value.is_finite()) {
            self.status_text = "Frame coordinates must be finite numbers.".to_string();
            return;
        }
        if ((u2 - u1).powi(2) + (v2 - v1).powi(2)).sqrt() < 1e-6 {
            self.status_text = "Frame ends must be at different coordinates.".to_string();
            return;
        }
        self.push_undo();
        let a = self.ensure_node_on_plane(u1, v1);
        let b = self.ensure_node_on_plane(u2, v2);
        if self.frame_exists(a, b) {
            self.status_text = format!("A frame already connects N{a} and N{b}.");
            return;
        }
        let next_id = self.model.element_counter + 1;
        let material = self.default_material_id();
        let section = self.default_section_id();
        let mut element = Element::frame(next_id, a, b, material, section);
        element.cardinal_point = self.cardinal_point;
        self.model.add_element(element);
        self.selected_nodes.clear();
        self.selected_elements.clear();
        self.selected_areas.clear();
        self.selected_elements.insert(next_id);
        self.status_text = format!("Frame F{next_id} created from typed coordinates (N{a} → N{b}).");
    }

    pub fn open_area_sections(&mut self) {
        let Some(area) = self.selected_areas.iter().next().and_then(|id| self.model.areas.get(id)) else {
            self.status_text = "Select one or more areas before defining their section.".to_string();
            return;
        };
        self.area_section_kind = area.kind;
        self.area_section_thickness = area.thickness;
        self.area_section_material = area.material_id;
        self.area_section_dialog_open = true;
    }

    pub fn apply_area_sections(&mut self) {
        if self.selected_areas.is_empty() {
            self.status_text = "Select areas first.".to_string();
            self.area_section_dialog_open = false;
            return;
        }
        let thickness = self.area_section_thickness.clamp(0.001, 10.0);
        self.push_undo();
        let mut changed = 0;
        for id in self.selected_areas.clone() {
            if let Some(area) = self.model.areas.get_mut(&id) {
                area.kind = self.area_section_kind;
                area.thickness = thickness;
                area.material_id = self.area_section_material;
                changed += 1;
            }
        }
        self.status_text = format!("{} area section(s) set to {} / {:.3} m.", changed, self.area_section_kind.label(), thickness);
        self.area_section_dialog_open = false;
    }

    pub fn select_by_property(&mut self) {
        let story = if self.select_active_story_only { Some(self.active_elevation()) } else { None };
        let material = (self.select_material > 0).then_some(self.select_material);
        let section = (self.select_section > 0).then_some(self.select_section);
        let at_story = |z: f64| story.is_none_or(|elevation| (z - elevation).abs() < 1e-6);

        let mut nodes = HashSet::new();
        let mut elements = HashSet::new();
        let mut areas = HashSet::new();
        if matches!(self.select_scope, SelectScope::All | SelectScope::Joints) {
            for (&id, node) in &self.model.nodes {
                if at_story(node.z) {
                    nodes.insert(id);
                }
            }
        }
        if matches!(self.select_scope, SelectScope::All | SelectScope::Frames) {
            for (&id, element) in &self.model.elements {
                if material.is_some_and(|material| element.material_id != material) || section.is_some_and(|section| element.section_id != section) {
                    continue;
                }
                if let (Some(a), Some(b)) = (self.model.nodes.get(&element.node_i), self.model.nodes.get(&element.node_j)) {
                    if at_story(a.z) && at_story(b.z) {
                        elements.insert(id);
                    }
                }
            }
        }
        if matches!(self.select_scope, SelectScope::All | SelectScope::Areas) {
            for (&id, area) in &self.model.areas {
                if material.is_some_and(|material| area.material_id != material) {
                    continue;
                }
                if area.node_ids.iter().all(|node_id| self.model.nodes.get(node_id).is_some_and(|node| at_story(node.z))) {
                    areas.insert(id);
                }
            }
        }
        self.selected_nodes = nodes;
        self.selected_elements = elements;
        self.selected_areas = areas;
        self.status_text = format!(
            "Property selection: {} joint(s), {} frame(s), {} area(s).",
            self.selected_nodes.len(),
            self.selected_elements.len(),
            self.selected_areas.len()
        );
        self.select_property_dialog_open = false;
    }

    pub fn open_groups(&mut self) {
        if self.group_name.trim().is_empty() {
            self.group_name = format!("Group {}", self.groups.len() + 1);
        }
        self.group_dialog_open = true;
    }

    pub fn create_group_from_selection(&mut self) {
        if self.selected_nodes.is_empty() && self.selected_elements.is_empty() && self.selected_areas.is_empty() {
            self.status_text = "Select objects before creating a group.".to_string();
            return;
        }
        let name = if self.group_name.trim().is_empty() { format!("Group {}", self.groups.len() + 1) } else { self.group_name.trim().to_string() };
        self.push_undo();
        let group = Group::from_selection(name.clone(), &self.selected_nodes, &self.selected_elements, &self.selected_areas);
        if let Some(existing) = self.groups.iter_mut().find(|group| group.name == name) {
            *existing = group;
        } else {
            self.groups.push(group);
        }
        self.group_choice = self.groups.iter().position(|group| group.name == name).unwrap_or(0);
        self.status_text = format!("Group '{name}' stored with {} object(s).", self.groups[self.group_choice].count());
    }

    pub fn replace_group_from_selection(&mut self) {
        if self.groups.is_empty() || self.group_choice >= self.groups.len() {
            self.status_text = "Choose a group first.".to_string();
            return;
        }
        if self.selected_nodes.is_empty() && self.selected_elements.is_empty() && self.selected_areas.is_empty() {
            self.status_text = "Select objects before replacing a group.".to_string();
            return;
        }
        self.push_undo();
        let name = self.groups[self.group_choice].name.clone();
        self.groups[self.group_choice] = Group::from_selection(name, &self.selected_nodes, &self.selected_elements, &self.selected_areas);
        self.status_text = format!("Group replaced with {} object(s).", self.groups[self.group_choice].count());
    }

    pub fn select_group(&mut self) {
        let Some(group) = self.groups.get(self.group_choice) else {
            self.status_text = "Choose a group first.".to_string();
            return;
        };
        self.selected_nodes = group.nodes.iter().copied().filter(|id| self.model.nodes.contains_key(id)).collect();
        self.selected_elements = group.elements.iter().copied().filter(|id| self.model.elements.contains_key(id)).collect();
        self.selected_areas = group.areas.iter().copied().filter(|id| self.model.areas.contains_key(id)).collect();
        self.status_text = format!(
            "Selected group '{}': {} joint(s), {} frame(s), {} area(s).",
            group.name,
            self.selected_nodes.len(),
            self.selected_elements.len(),
            self.selected_areas.len()
        );
    }

    pub fn delete_group(&mut self) {
        if self.group_choice >= self.groups.len() {
            return;
        }
        self.push_undo();
        let removed = self.groups.remove(self.group_choice);
        self.group_choice = self.group_choice.min(self.groups.len().saturating_sub(1));
        self.status_text = format!("Group '{}' deleted.", removed.name);
    }

    // -- command dispatch ----------------------------------------------------

    pub fn run(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::New => self.reset_model(),
            Cmd::BuildingTemplate => self.open_new_model(),
            Cmd::Open => self.open_file(),
            Cmd::Save => self.save_file(false),
            Cmd::SaveAs => self.save_file(true),
            Cmd::Exit => self.ctx.send_viewport_cmd(egui::ViewportCommand::Close),

            Cmd::Undo => self.undo(),
            Cmd::Redo => self.redo(),
            Cmd::Delete => self.delete_selection(),
            Cmd::MoveSelection => self.open_edit_dialog(EditMode::Move),
            Cmd::CopySelection => self.open_edit_dialog(EditMode::Copy),
            Cmd::MirrorSelection => self.open_edit_dialog(EditMode::Mirror),
            Cmd::RotateSelection => self.open_edit_dialog(EditMode::Rotate),
            Cmd::AlignSelection => self.open_align_dialog(),
            Cmd::Replicate => self.open_edit_dialog(EditMode::Replicate),
            Cmd::MergeJoints => self.merge_joints_now(),
            Cmd::SplitFrames => self.split_at_intersections(),
            Cmd::TrimExtend => self.trim_extend_now(),
            Cmd::BenchmarkModel => self.load_benchmark_model(),
            Cmd::ToggleShaded3d => {
                if !self.wgpu_ready {
                    self.status_text = "The wgpu shaded-section pass is not available in this build/session.".to_string();
                } else {
                    self.shade_3d = !self.shade_3d;
                    self.status_text = if self.shade_3d { "wgpu shaded sections on." } else { "wgpu shaded sections off." }.to_string();
                }
            }
            Cmd::ToggleClip3d => {
                self.clip_3d = !self.clip_3d;
                self.status_text = if self.clip_3d {
                    format!("3D clipping on above {:.2} m.", self.active_elevation())
                } else {
                    "3D clipping off.".to_string()
                };
            }
            Cmd::CreateGridMembers => {
                if self.grid.segments().is_empty() {
                    self.status_text = "The current grid has no lines to build on.".to_string();
                } else if !self.work_plane_horizontal() {
                    self.status_text = "Grid members need a horizontal work plane; select a story or a Z plane.".to_string();
                } else {
                    self.grid_members_wall_height = self.default_story_height();
                    self.grid_members_dialog_open = true;
                }
            }
            Cmd::CommandPalette => {
                self.palette_open = true;
                self.palette_query.clear();
            }

            Cmd::ZoomFit => self.pending_fit = Some(()),
            Cmd::ZoomIn => self.view.scale = (self.view.scale * 1.25).clamp(3.0, 8000.0),
            Cmd::ZoomOut => self.view.scale = (self.view.scale / 1.25).clamp(3.0, 8000.0),
            Cmd::ViewPlan => self.view_tab = ViewTab::Plan,
            Cmd::View3D => self.view_tab = ViewTab::View3D,
            Cmd::ElevationView => {
                self.set_view_preset(ViewPreset::Front);
            }
            Cmd::ToggleGrid => self.show_grid = !self.show_grid,
            Cmd::ToggleDeformed => {
                self.show_deformed = !self.show_deformed;
                if self.show_deformed && self.results.is_none() {
                    self.status_text = "Run the analysis to see the deformed shape.".to_string();
                }
            }
            Cmd::DisplayLoadSymbols => self.show_loads = !self.show_loads,
            Cmd::ToggleExplorer => self.show_explorer = !self.show_explorer,
            Cmd::ToggleProperties => self.show_properties = !self.show_properties,
            Cmd::ShowTables => {
                self.show_tables = true;
                self.bottom_tab = BottomTab::Joints;
            }

            Cmd::GridSystem => {
                self.grid_dialog_open = true;
                if let Some(cylindrical) = &self.grid.cylindrical {
                    self.grid_cylindrical = true;
                    self.cyl_center_x = cylindrical.center.0;
                    self.cyl_center_y = cylindrical.center.1;
                    self.cyl_ring_spacing = match cylindrical.radii.as_slice() {
                        [first, second, ..] => second - first,
                        [only] => *only,
                        [] => 5.0,
                    };
                    self.cyl_rings = cylindrical.radii.len().max(1) as i32;
                    self.cyl_angle_step = match cylindrical.angles_deg.as_slice() {
                        [first, second, ..] => second - first,
                        _ => 30.0,
                    };
                    self.cyl_angle_count = cylindrical.angles_deg.len().max(1) as i32;
                    self.cyl_start_angle = cylindrical.angles_deg.first().copied().unwrap_or(0.0);
                } else {
                    self.grid_cylindrical = false;
                    self.grid_spacing = self.grid.xs.get(1).copied().unwrap_or(5.0);
                    self.grid_nx = self.grid.xs.len().saturating_sub(1) as i32;
                    self.grid_ny = self.grid.ys.len().saturating_sub(1) as i32;
                    self.grid_draft_xs = self.grid.xs.clone();
                    self.grid_draft_ys = self.grid.ys.clone();
                    self.grid_draft_x_visible = (0..self.grid.xs.len()).map(|index| self.grid.x_line_visible(index)).collect();
                    self.grid_draft_y_visible = (0..self.grid.ys.len()).map(|index| self.grid.y_line_visible(index)).collect();
                    self.grid_draft_x_bubble = (0..self.grid.xs.len()).map(|index| self.grid.x_bubble_at(index)).collect();
                    self.grid_draft_y_bubble = (0..self.grid.ys.len()).map(|index| self.grid.y_bubble_at(index)).collect();
                }
                self.grid_name = self.grid.name.clone();
                self.grid_origin_x = self.grid.origin.0;
                self.grid_origin_y = self.grid.origin.1;
                self.grid_rotation_deg = self.grid.rotation_deg;
                self.grid_bubble_mm = self.grid.bubble_size * 1000.0;
                self.grid_display_as_spacing = self.grid.display_as_spacing;
                self.grid_x_labels = (0..self.grid.xs.len()).map(|index| self.grid.label_x(index)).collect();
                self.grid_y_labels = (0..self.grid.ys.len()).map(|index| self.grid.label_y(index)).collect();
                self.cyl_ring_labels = match &self.grid.cylindrical {
                    Some(cylindrical) => (0..cylindrical.radii.len()).map(|index| cylindrical.ring_label(index)).collect(),
                    None => (0..self.cyl_rings.max(1) as usize).map(|index| format!("R{}", index + 1)).collect(),
                };
                self.cyl_radial_labels = match &self.grid.cylindrical {
                    Some(cylindrical) => (0..cylindrical.angles_deg.len()).map(|index| cylindrical.radial_label(index)).collect(),
                    None => (0..self.cyl_angle_count.max(1) as usize).map(|index| format!("A{}", index + 1)).collect(),
                };
            }
            Cmd::DefineRefPlanes => {
                self.ref_dialog_open = true;
                self.ref_draft = self.ref_planes.clone();
                if self.ref_draft.is_empty() {
                    self.ref_draft.push(RefPlane::new("Ref 1", RefPlaneAxis::X, 0.0));
                }
            }
            Cmd::DefineStories => self.open_story_dialog(),

            Cmd::ToolSelect => self.tool = Tool::Select,
            Cmd::DrawJoint => self.tool = Tool::DrawNode,
            Cmd::DrawFrame => {
                self.tool = Tool::DrawFrame;
                self.quick_draw = false;
            }
            Cmd::DrawArea => {
                self.tool = Tool::DrawArea;
                self.draw_frame_start = None;
                self.draw_wall_start = None;
                self.area_points.clear();
                self.status_text = format!(
                    "Draw Areas ({}): {}",
                    self.area_shape.label(),
                    match self.area_shape {
                        AreaShape::Rectangle => "click two opposite corners.",
                        AreaShape::Polygon => "click vertices, then double-click or right-click to close.",
                        AreaShape::Circle => "click the centre, then a point on the circle.",
                        AreaShape::Ellipse => "click the centre, then a corner of the bounding box.",
                        AreaShape::Arc => "click the centre, the start of the arc, then its end.",
                    }
                );
            }
            Cmd::DrawWall => {
                self.tool = Tool::DrawWall;
                self.draw_frame_start = None;
                self.area_points.clear();
                self.draw_wall_start = None;
                self.wall_height = self.default_story_height();
                self.status_text =
                    format!("Draw Walls: click a base joint, then the next; panels are {:.2} m high. Right-click ends the chain.", self.wall_height);
            }
            Cmd::TypeCoordinates => self.open_typed_coords(),
            Cmd::QuickDraw => {
                self.tool = Tool::DrawFrame;
                self.quick_draw = true;
                self.draw_frame_start = None;
                self.status_text = "Quick Draw: click two points for one frame; the tool returns to Select.".to_string();
            }
            Cmd::DefineGroups => self.open_groups(),
            Cmd::SelectByGroup => self.open_groups(),
            Cmd::SelectByProperty => self.select_property_dialog_open = true,
            Cmd::DefineAreaSections => self.open_area_sections(),
            Cmd::DefineMaterials => self.open_materials(),
            Cmd::DefineFrameSections => self.open_sections(),
            Cmd::DefineLoadPatterns => self.open_load_patterns(),
            Cmd::DefineLoadCases => self.open_load_cases(),
            Cmd::DefineCombos => self.open_combos(),
            Cmd::AssignFrameLoads => self.frame_load_dialog_open = true,
            Cmd::AssignAreaLoads => self.area_load_dialog_open = true,
            Cmd::AssignReleases => self.open_frame_releases(),
            Cmd::JointSprings => self.open_joint_springs(),
            Cmd::TemperatureLoads => self.open_temperature_loads(),
            Cmd::Preferences => self.preferences_dialog_open = true,
            Cmd::DisplayOptions => self.display_options_open = true,
            Cmd::CloseView => {
                self.view_tab = ViewTab::Plan;
                self.status_text = "Closed the 3D tab; returned to the plan view.".to_string();
            }

            Cmd::SelectAll => {
                self.selected_nodes = self.model.nodes.keys().copied().collect();
                self.selected_elements = self.model.elements.keys().copied().collect();
                self.selected_areas = self.model.areas.keys().copied().collect();
                self.status_text =
                    format!("Selected {} joints, {} frames and {} areas.", self.selected_nodes.len(), self.selected_elements.len(), self.selected_areas.len());
            }
            Cmd::DeselectAll => {
                self.selected_nodes.clear();
                self.selected_elements.clear();
                self.selected_areas.clear();
                self.status_text = "Selection cleared.".to_string();
            }
            Cmd::InvertSelection => {
                let all_nodes: HashSet<usize> = self.model.nodes.keys().copied().collect();
                let all_elements: HashSet<usize> = self.model.elements.keys().copied().collect();
                let all_areas: HashSet<usize> = self.model.areas.keys().copied().collect();
                self.selected_nodes = all_nodes.difference(&self.selected_nodes).copied().collect();
                self.selected_elements = all_elements.difference(&self.selected_elements).copied().collect();
                self.selected_areas = all_areas.difference(&self.selected_areas).copied().collect();
                self.status_text =
                    format!("Selected {} joints, {} frames and {} areas.", self.selected_nodes.len(), self.selected_elements.len(), self.selected_areas.len());
            }
            Cmd::SelectByStory => {
                let elevation = self.active_elevation();
                self.selected_nodes = self.model.nodes.iter().filter(|(_, node)| (node.z - elevation).abs() < 1e-6).map(|(&id, _)| id).collect();
                self.selected_elements = self
                    .model
                    .elements
                    .iter()
                    .filter(|(_, element)| {
                        let ni = self.model.nodes.get(&element.node_i);
                        let nj = self.model.nodes.get(&element.node_j);
                        matches!((ni, nj), (Some(a), Some(b)) if (a.z - elevation).abs() < 1e-6 && (b.z - elevation).abs() < 1e-6)
                    })
                    .map(|(&id, _)| id)
                    .collect();
                self.selected_areas = self
                    .model
                    .areas
                    .iter()
                    .filter(|(_, area)| area.node_ids.iter().all(|id| self.model.nodes.get(id).is_some_and(|node| (node.z - elevation).abs() < 1e-6)))
                    .map(|(&id, _)| id)
                    .collect();
                self.status_text = format!(
                    "Selected {} joints, {} frames and {} areas on {}.",
                    self.selected_nodes.len(),
                    self.selected_elements.len(),
                    self.selected_areas.len(),
                    self.active_story_name()
                );
            }

            Cmd::FixityFixed => self.assign_restraints([true; 6]),
            Cmd::FixityPinned => self.assign_restraints([true, true, true, false, false, false]),
            Cmd::FixityPinnedTorsion => self.assign_restraints([true, true, true, true, true, false]),
            Cmd::FixityRoller => self.assign_restraints([false, false, true, false, false, false]),
            Cmd::FixityRollerTorsion => self.assign_restraints([false, false, true, true, true, false]),
            Cmd::FixityFree => self.assign_restraints([false; 6]),
            Cmd::AssignPointLoad => self.apply_loads_to_selection(false),
            Cmd::RemovePointLoad => self.apply_loads_to_selection(true),

            Cmd::ImportE2K => self.import_e2k_file(),
            Cmd::ExportE2K => self.export_e2k_dialog_open = true,
            Cmd::DefineDiaphragms => {
                self.diaphragm_draft = self.model.diaphragms.clone();
                self.define_diaphragms_dialog_open = true;
            }
            Cmd::AssignDiaphragm => {
                self.assign_diaphragm_choice = self.model.diaphragms.first().map(|diaphragm| diaphragm.id).unwrap_or(1);
                self.assign_diaphragm_dialog_open = true;
            }
            Cmd::AnalysisOptions => self.analysis_options_dialog_open = true,
            Cmd::ModelCheck => {
                let report = openstruct::check::check_model(&self.model);
                self.status_text = format!("Model check: {}", report.summary());
                self.model_check_report = Some(report);
                self.model_check_dialog_open = true;
            }
            Cmd::ResultsSetup => self.results_setup_dialog_open = true,
            Cmd::FrameDiagrams => self.frame_diagrams_dialog_open = true,
            Cmd::AreaContours => self.area_contours_dialog_open = true,
            Cmd::StoryResults => self.story_results_dialog_open = true,
            Cmd::SteelDesign => self.steel_design_dialog_open = true,
            Cmd::ConcreteDesign => self.concrete_design_dialog_open = true,
            Cmd::CompositeDesign => self.composite_design_dialog_open = true,
            Cmd::ShearWallDesign => self.shear_wall_design_dialog_open = true,
            Cmd::Detailing => self.detailing_dialog_open = true,
            Cmd::Reports => self.reports_dialog_open = true,
            Cmd::Documentation => self.documentation_dialog_open = true,
            Cmd::VerificationCases => self.verification_dialog_open = true,

            Cmd::RunAnalysis => self.run_analysis(),

            Cmd::UnitsMetric => {
                self.units = UnitSystem::Metric;
                self.status_text = "Display units: metric (kN, m).".to_string();
            }
            Cmd::UnitsImperial => {
                self.units = UnitSystem::Imperial;
                self.status_text = "Display units: imperial (kip, ft).".to_string();
            }
            Cmd::ThemeDark => {
                self.dark = true;
                theme::install(&self.ctx, true);
            }
            Cmd::ThemeLight => {
                self.dark = false;
                theme::install(&self.ctx, false);
            }
            Cmd::KeyboardShortcuts => self.shortcuts_open = true,
            Cmd::ResetLayout => {
                self.show_explorer = true;
                self.show_properties = true;
                self.show_tables = true;
                self.view_tab = ViewTab::Plan;
                self.status_text = "Layout reset.".to_string();
            }
            Cmd::About => self.about_open = true,

            _ => {
                if let Some(phase) = cmd.planned() {
                    self.status_text = format!("{} — planned for Phase {}. See Phase.md.", cmd.label(), phase);
                } else {
                    self.status_text = format!("{} is not available.", cmd.label());
                }
            }
        }
    }

    // -- phase 3: load patterns and load assignment ---------------------------

    pub fn open_load_patterns(&mut self) {
        self.load_pattern_draft = self.model.load_patterns.clone();
        if self.load_pattern_draft.is_empty() {
            self.load_pattern_draft = openstruct::loads::default_patterns();
        }
        self.load_pattern_new_name.clear();
        self.define_load_patterns_dialog_open = true;
        self.status_text = format!("{} load pattern(s) defined.", self.model.load_patterns.len());
    }

    pub fn add_load_pattern(&mut self) {
        let name = self.load_pattern_new_name.trim().to_uppercase();
        if name.is_empty() {
            self.status_text = "Enter a pattern name before adding it.".to_string();
            return;
        }
        if self.load_pattern_draft.iter().any(|pattern| pattern.name.eq_ignore_ascii_case(&name)) {
            self.status_text = format!("Pattern {name} already exists.");
            return;
        }
        let id = self.load_pattern_draft.iter().map(|pattern| pattern.id).max().unwrap_or(0) + 1;
        self.load_pattern_draft.push(LoadPattern {
            id,
            name,
            load_type: self.load_pattern_new_type.clone(),
            self_weight_multiplier: 1.0,
        });
        self.load_pattern_new_name.clear();
        self.status_text = format!("{} pattern(s) in the draft.", self.load_pattern_draft.len());
    }

    pub fn add_catalogue_patterns(&mut self) {
        let mut added = 0usize;
        for pattern in openstruct::loads::default_patterns() {
            if self.load_pattern_draft.iter().any(|existing| existing.name.eq_ignore_ascii_case(&pattern.name)) {
                continue;
            }
            let id = self.load_pattern_draft.iter().map(|existing| existing.id).max().unwrap_or(0) + 1;
            self.load_pattern_draft.push(LoadPattern { id, ..pattern });
            added += 1;
        }
        self.status_text = format!("Added {added} reference pattern(s) to the draft.");
    }

    pub fn apply_load_patterns(&mut self) -> bool {
        let mut seen: Vec<String> = Vec::new();
        let mut error: Option<String> = None;
        for pattern in &mut self.load_pattern_draft {
            pattern.name = pattern.name.trim().to_uppercase();
            if pattern.name.is_empty() {
                error = Some("Every load pattern needs a name.".to_string());
                break;
            }
            if seen.iter().any(|name| name == &pattern.name) {
                error = Some(format!("Duplicate load pattern name: {}.", pattern.name));
                break;
            }
            if pattern.self_weight_multiplier < 0.0 {
                error = Some(format!("{} has a negative self-weight multiplier.", pattern.name));
                break;
            }
            seen.push(pattern.name.clone());
        }
        if let Some(message) = error {
            self.status_text = message;
            return false;
        }
        self.push_undo();
        self.model.load_patterns = self.load_pattern_draft.clone();
        self.status_text = format!("{} load pattern(s) applied.", self.model.load_patterns.len());
        true
    }

    /// Human-readable label of a load case id, used in status text.
    fn case_label(&self, case_id: Option<usize>) -> String {
        match case_id {
            None => "all load cases".to_string(),
            Some(id) => self
                .model
                .load_cases
                .iter()
                .find(|case| case.id == id)
                .map(|case| case.name.clone())
                .unwrap_or_else(|| format!("case {id}")),
        }
    }

    pub fn apply_frame_loads(&mut self) -> bool {
        if self.selected_elements.is_empty() {
            self.status_text = "Select at least one frame first.".to_string();
            return false;
        }
        let Some(magnitude) = self.units.parse_distributed(&self.frame_load_magnitude) else {
            self.status_text = format!("Cannot read '{}' as a distributed load.", self.frame_load_magnitude);
            return false;
        };
        let mut intensity = [0.0_f64; 3];
        intensity[self.frame_load_axis.min(2)] = magnitude;
        let ids: Vec<usize> = self.selected_elements.iter().copied().collect();
        let direction = self.frame_load_direction;
        let projected = self.frame_load_projected;
        let case_id = self.frame_load_case;
        let case_label = self.case_label(case_id);
        self.push_undo();
        self.model.frame_loads.retain(|load| !(ids.contains(&load.element_id) && load.case_id == case_id));
        for id in &ids {
            self.model.frame_loads.push(DistributedLoad {
                element_id: *id,
                direction,
                start: intensity,
                end: intensity,
                projected,
                case_id,
            });
        }
        self.status_text = format!("Applied a uniform frame load to {} frame(s) in {}.", ids.len(), case_label);
        true
    }

    pub fn apply_area_loads(&mut self) -> bool {
        if self.selected_areas.is_empty() {
            self.status_text = "Select at least one area first.".to_string();
            return false;
        }
        let Some(pressure) = self.units.parse_pressure(&self.area_load_pressure) else {
            self.status_text = format!("Cannot read '{}' as a pressure.", self.area_load_pressure);
            return false;
        };
        let ids: Vec<usize> = self.selected_areas.iter().copied().collect();
        let direction = self.area_load_direction;
        let case_id = self.area_load_case;
        let case_label = self.case_label(case_id);
        self.push_undo();
        self.model.area_loads.retain(|load| !(ids.contains(&load.area_id) && load.case_id == case_id));
        for id in &ids {
            self.model.area_loads.push(AreaLoad { area_id: *id, direction, pressure, case_id });
        }
        self.status_text = format!("Applied a uniform pressure to {} area(s) in {}.", ids.len(), case_label);
        true
    }

    // -- phase 3: materials and frame sections --------------------------------

    pub fn open_materials(&mut self) {
        let mut draft: Vec<(usize, Material)> = self.model.materials.iter().map(|(&id, material)| (id, material.clone())).collect();
        draft.sort_by_key(|(id, _)| *id);
        if draft.is_empty() {
            draft.push((0, Material::steel()));
        }
        self.material_draft = draft;
        self.material_selected = 0;
        self.define_materials_dialog_open = true;
    }

    /// Open the Add New Material Property chooser at its defaults.
    pub fn open_new_material(&mut self) {
        self.new_material_open = true;
        self.new_material_region = 0;
        self.new_material_type = openstruct::materials::MaterialType::Steel;
        self.new_material_standard = 0;
        self.new_material_grade = 0;
    }

    /// Add a copy of the selected material under a unique name.
    pub fn copy_selected_material(&mut self) {
        let index = self.material_selected.min(self.material_draft.len().saturating_sub(1));
        let Some((_, source)) = self.material_draft.get(index) else {
            return;
        };
        let mut copy = source.clone();
        let base = copy.name.clone();
        let mut name = format!("{base} copy");
        let mut suffix = 2;
        while self.material_draft.iter().any(|(_, material)| material.name.eq_ignore_ascii_case(&name)) {
            name = format!("{base} copy {suffix}");
            suffix += 1;
        }
        copy.name = name;
        self.material_draft.push((0, copy));
        self.material_selected = self.material_draft.len() - 1;
        self.status_text = "Material copied; edit it and press OK to keep it.".to_string();
    }

    /// Delete the selected material from the draft, keeping at least one.
    pub fn delete_selected_material(&mut self) {
        if self.material_draft.len() <= 1 {
            self.status_text = "A model needs at least one material.".to_string();
            return;
        }
        let index = self.material_selected.min(self.material_draft.len() - 1);
        let removed = self.material_draft.remove(index);
        self.material_selected = self.material_selected.min(self.material_draft.len() - 1);
        self.status_text = format!("Removed material {}.", removed.1.name);
    }

    /// Build a material from the region / type / standard / grade library.
    pub fn create_material_from_grade(&mut self) {
        let mut material = crate::material_library::material_from(
            self.new_material_region,
            self.new_material_type,
            self.new_material_standard,
            self.new_material_grade,
        );
        let base = material.name.clone();
        let mut name = base.clone();
        let mut suffix = 2;
        while self.material_draft.iter().any(|(_, existing)| existing.name.eq_ignore_ascii_case(&name)) {
            name = format!("{base} {suffix}");
            suffix += 1;
        }
        material.name = name;
        self.material_draft.push((0, material));
        self.material_selected = self.material_draft.len() - 1;
        self.status_text = format!("Added material {}.", self.material_draft[self.material_selected].1.name);
    }

    /// Open the Material Property Data editor for the selected material.
    pub fn open_material_editor(&mut self) {
        let index = self.material_selected.min(self.material_draft.len().saturating_sub(1));
        if let Some((_, material)) = self.material_draft.get(index) {
            self.material_edit = Some(material.clone());
            self.material_editor = Some(index);
            self.material_mass_input = true;
            self.material_show_design = false;
            self.material_show_damping = false;
        }
    }

    pub fn apply_materials(&mut self) -> bool {
        let mut seen: Vec<String> = Vec::new();
        let mut error: Option<String> = None;
        for (_, material) in &mut self.material_draft {
            material.name = material.name.trim().to_string();
            if material.name.is_empty() {
                error = Some("Every material needs a name.".to_string());
                break;
            }
            if seen.iter().any(|name| name.eq_ignore_ascii_case(&material.name)) {
                error = Some(format!("Duplicate material name: {}.", material.name));
                break;
            }
            if material.e <= 0.0 {
                error = Some(format!("{} needs a positive modulus of elasticity.", material.name));
                break;
            }
            // Keep a user-entered shear modulus; only fill one in when missing.
            if material.g <= 0.0 || !material.g.is_finite() {
                material.g = material.shear_modulus();
            }
            seen.push(material.name.clone());
        }
        if let Some(message) = error {
            self.status_text = message;
            return false;
        }

        let mut materials = std::collections::HashMap::new();
        let mut next_id = self.model.material_counter;
        for (id, material) in &self.material_draft {
            let id = if *id == 0 {
                next_id += 1;
                next_id
            } else {
                *id
            };
            materials.insert(id, material.clone());
        }
        if let Some(element) = self.model.elements.values().find(|element| !materials.contains_key(&element.material_id)) {
            self.status_text = format!("Frame {} still uses a material that is no longer defined.", element.id);
            return false;
        }
        if let Some(area) = self.model.areas.values().find(|area| !materials.contains_key(&area.material_id)) {
            self.status_text = format!("Area {} still uses a material that is no longer defined.", area.id);
            return false;
        }
        self.push_undo();
        self.model.material_counter = next_id.max(self.model.material_counter);
        self.model.materials = materials;
        self.status_text = format!("{} material(s) defined.", self.model.materials.len());
        true
    }

    pub fn assign_material(&mut self, material_id: usize) {
        if self.selected_elements.is_empty() {
            self.status_text = "Select frames to assign a material.".to_string();
            return;
        }
        self.push_undo();
        for &id in &self.selected_elements {
            if let Some(element) = self.model.elements.get_mut(&id) {
                element.material_id = material_id;
            }
        }
        self.status_text = format!("Material assigned to {} frame(s).", self.selected_elements.len());
    }

    /// Seed the release dialog from the lowest selected frame.
    pub fn open_frame_releases(&mut self) {
        if self.selected_elements.is_empty() {
            self.status_text = "Select frames to assign releases.".to_string();
            return;
        }
        let first = *self.selected_elements.iter().min().expect("selection is non-empty");
        if let Some(element) = self.model.elements.get(&first) {
            self.release_draft = element.releases;
            self.fixity_draft = element.partial_fixity;
        }
        self.frame_release_dialog_open = true;
    }

    /// Open the temperature / strain load dialog for the selected frames.
    pub fn open_temperature_loads(&mut self) {
        if self.selected_elements.is_empty() {
            self.status_text = "Select frames to assign temperature loads.".to_string();
            return;
        }
        self.temperature_dialog_open = true;
    }

    /// Replace (or remove) the temperature loads on the selected frames for the
    /// dialog's case. Loads on other cases and other frames are kept.
    pub fn apply_temperature_loads(&mut self, remove: bool) {
        if self.selected_elements.is_empty() {
            self.status_text = "Select frames to assign temperature loads.".to_string();
            self.temperature_dialog_open = false;
            return;
        }
        if !remove && (!self.temperature_delta.is_finite() || !self.temperature_strain.is_finite()) {
            self.status_text = "Temperature and strain values must be finite.".to_string();
            return;
        }
        self.push_undo();
        let case = self.temperature_case;
        let selected: Vec<usize> = self.selected_elements.iter().copied().collect();
        self.model
            .temperature_loads
            .retain(|load| !(load.case_id == case && selected.contains(&load.element_id)));
        if !remove && (self.temperature_delta != 0.0 || self.temperature_strain != 0.0) {
            for &element_id in &selected {
                self.model.temperature_loads.push(TemperatureLoad {
                    element_id,
                    delta_t: self.temperature_delta,
                    strain: self.temperature_strain,
                    case_id: case,
                });
            }
        }
        self.results = None;
        self.temperature_dialog_open = false;
        self.status_text = if remove {
            format!("Removed temperature loads from {} frame(s).", selected.len())
        } else {
            format!("Temperature load assigned to {} frame(s); re-run the analysis (F5).", selected.len())
        };
    }

    /// Seed the joint spring dialog from the lowest selected joint.
    pub fn open_joint_springs(&mut self) {
        if self.selected_nodes.is_empty() {
            self.status_text = "Select joints to assign support springs.".to_string();
            return;
        }
        let first = *self.selected_nodes.iter().min().expect("selection is non-empty");
        if let Some(node) = self.model.nodes.get(&first) {
            self.spring_draft = node.springs;
        }
        self.joint_springs_dialog_open = true;
    }

    /// Write the drafted diaphragm releases onto every selected frame.
    pub fn apply_diaphragm(&mut self) {
        if self.selected_elements.is_empty() {
            self.status_text = "Select frames to assign diaphragm.".to_string();
            self.diaphragm_dialog_open = false;
            return;
        }
        if self.diaphragm_release_draft.iter().any(|&released| !released) {
            self.status_text = "Release flags must be true to release.".to_string();
            return;
        }
        self.push_undo();
        let mut updated = 0usize;
        for &id in &self.selected_elements {
            if let Some(element) = self.model.elements.get_mut(&id) {
                element.releases = self.diaphragm_release_draft;
                updated += 1;
            }
        }
        self.results = None;
        self.diaphragm_dialog_open = false;
        self.status_text = format!("Diaphragm releases assigned to {} frame(s); re-run the analysis (F5).", updated);
    }

    /// Write the drafted support springs onto every selected joint.
    pub fn apply_joint_springs(&mut self) {
        if self.selected_nodes.is_empty() {
            self.status_text = "Select joints to assign support springs.".to_string();
            self.joint_springs_dialog_open = false;
            return;
        }
        if self.spring_draft.iter().any(|&stiffness| !stiffness.is_finite() || stiffness < 0.0) {
            self.status_text = "Spring stiffnesses must be finite and non-negative.".to_string();
            return;
        }
        self.push_undo();
        let mut updated = 0usize;
        for &id in &self.selected_nodes {
            if let Some(node) = self.model.nodes.get_mut(&id) {
                node.springs = self.spring_draft;
                updated += 1;
            }
        }
        self.results = None;
        self.joint_springs_dialog_open = false;
        self.status_text = format!("Support springs assigned to {updated} joint(s); re-run the analysis (F5).");
    }

    /// Write the drafted releases onto every selected frame.
    pub fn apply_frame_releases(&mut self) {
        if self.selected_elements.is_empty() {
            self.status_text = "Select frames to assign releases.".to_string();
            self.frame_release_dialog_open = false;
            return;
        }
        self.push_undo();
        let mut updated = 0usize;
        for &id in &self.selected_elements {
            if let Some(element) = self.model.elements.get_mut(&id) {
                element.releases = self.release_draft;
                element.partial_fixity = self.fixity_draft;
                updated += 1;
            }
        }
        self.results = None;
        self.frame_release_dialog_open = false;
        self.status_text = format!("Releases assigned to {updated} frame(s); re-run the analysis (F5).");
    }

    /// Open the section designer, seeded from a draft row when one is given.
    /// A parametric row seeds equivalent drawn parts so it can be reshaped.
    pub fn open_section_designer(&mut self, target: Option<usize>) {
        let seeded = target.and_then(|index| self.section_draft.get(index)).is_some_and(|draft| draft.designer.is_empty());
        self.designer_parts = match target.and_then(|index| self.section_draft.get(index)) {
            Some(draft) if draft.designer.is_empty() => {
                let parts = shape_from_draft(draft).map(|shape| shape.designer_parts()).unwrap_or_default();
                if !parts.is_empty() {
                    let span = draft.width.max(draft.depth);
                    if span > 0.0 {
                        self.designer_view = (span * 0.8).max(0.2);
                    }
                }
                parts
            }
            Some(draft) => draft.designer.clone(),
            None => {
                self.designer_view = 0.6;
                Vec::new()
            }
        };
        self.designer_target = target;
        self.designer_selected = None;
        self.designer_drag_start = None;
        self.designer_drag_vertex = None;
        self.designer_vertex = None;
        self.designer_pending.clear();
        self.designer_pending_hole.clear();
        self.designer_hole_mode = false;
        self.section_designer_open = true;
        if seeded && !self.designer_parts.is_empty() {
            let name = target
                .and_then(|index| self.section_draft.get(index))
                .map(|draft| draft.name.clone())
                .unwrap_or_default();
            self.status_text = format!("Designer seeded from {name}; Apply replaces it with a drawn section.");
        }
    }

    /// Write the drawn parts into the target draft row (or a new row).
    pub fn apply_section_designer(&mut self) -> bool {
        if self.designer_parts.is_empty() {
            self.status_text = "Draw at least one part before applying the section.".to_string();
            return false;
        }
        let name = format!("CUSTOM{}", self.section_draft.len() + 1);
        let Some(section) = Section::from_designer(&name, &self.designer_parts) else {
            self.status_text = "The drawn section has invalid dimensions.".to_string();
            return false;
        };
        let width = section.width;
        let depth = section.depth;
        let parts = self.designer_parts.clone();
        match self.designer_target {
            Some(index) => {
                if let Some(row) = self.section_draft.get_mut(index) {
                    let id = row.id;
                    let row_name = row.name.clone();
                    let modifiers = row.modifiers;
                    *row = SectionDraft {
                        id,
                        kind: SectionKind::Custom,
                        name: row_name,
                        width,
                        depth,
                        web: 0.01,
                        flange: 0.02,
                        designer: parts,
                        modifiers,
                    };
                }
            }
            None => self.section_draft.push(SectionDraft {
                id: 0,
                kind: SectionKind::Custom,
                name,
                width,
                depth,
                web: 0.01,
                flange: 0.02,
                designer: parts,
                modifiers: Default::default(),
            }),
        }
        self.status_text = format!(
            "Drew a section with {} part(s); Apply in Frame Sections to save it.",
            self.designer_parts.len()
        );
        self.section_designer_open = false;
        true
    }

    pub fn open_sections(&mut self) {
        let mut draft: Vec<SectionDraft> = self
            .model
            .sections
            .iter()
            .map(|(&id, section)| draft_from_section(id, section))
            .collect();
        draft.sort_by_key(|section| section.id);
        if draft.is_empty() {
            draft.push(SectionDraft {
                id: 0,
                kind: SectionKind::Rectangular,
                name: "RECT300X500".into(),
                width: 0.3,
                depth: 0.5,
                web: 0.01,
                flange: 0.02,
                designer: Vec::new(),
                modifiers: Default::default(),
            });
        }
        self.section_draft = draft;
        self.define_frame_sections_dialog_open = true;
    }

    pub fn add_section_draft(&mut self) {
        let kind = self.section_new_kind;
        let name = format!("{}{}", kind.name_prefix(), self.section_draft.len() + 1);
        self.section_draft.push(SectionDraft {
            id: 0,
            kind,
            name,
            width: 0.3,
            depth: 0.5,
            web: 0.01,
            flange: 0.02,
            designer: Vec::new(),
            modifiers: Default::default(),
        });
    }

    pub fn apply_sections(&mut self) -> bool {
        let mut seen: Vec<String> = Vec::new();
        let mut error: Option<String> = None;
        for section in &mut self.section_draft {
            section.name = section.name.trim().to_string();
            if section.name.is_empty() {
                error = Some("Every frame section needs a name.".to_string());
                break;
            }
            if seen.iter().any(|name| name.eq_ignore_ascii_case(&section.name)) {
                error = Some(format!("Duplicate frame section name: {}.", section.name));
                break;
            }
            if section.width <= 0.0 || section.depth <= 0.0 {
                error = Some(format!("{} needs positive dimensions.", section.name));
                break;
            }
            if !section.modifiers.is_valid() {
                error = Some(format!("{} has invalid stiffness modifiers.", section.name));
                break;
            }
            seen.push(section.name.clone());
        }
        if let Some(message) = error {
            self.status_text = message;
            return false;
        }

        let mut sections = std::collections::HashMap::new();
        let mut next_id = self.model.section_counter;
        for section in &self.section_draft {
            let id = if section.id == 0 {
                next_id += 1;
                next_id
            } else {
                section.id
            };
            let built = match section.kind {
                SectionKind::Custom => Section::from_designer(&section.name, &section.designer),
                SectionKind::Rectangular => Section::from_shape(
                    &section.name,
                    openstruct::sections::SectionShape::Rectangular {
                        width: section.width,
                        depth: section.depth,
                    },
                ),
                SectionKind::Circular => Section::from_shape(
                    &section.name,
                    openstruct::sections::SectionShape::Circular { diameter: section.depth },
                ),
                SectionKind::ISection => Section::from_shape(
                    &section.name,
                    openstruct::sections::SectionShape::ISection {
                        depth: section.depth,
                        width: section.width,
                        web: section.web,
                        flange: section.flange,
                    },
                ),
                SectionKind::Channel => Section::from_shape(
                    &section.name,
                    openstruct::sections::SectionShape::Channel {
                        depth: section.depth,
                        width: section.width,
                        web: section.web,
                        flange: section.flange,
                    },
                ),
                SectionKind::Tee => Section::from_shape(
                    &section.name,
                    openstruct::sections::SectionShape::Tee {
                        depth: section.depth,
                        width: section.width,
                        web: section.web,
                        flange: section.flange,
                    },
                ),
                SectionKind::Box => Section::from_shape(
                    &section.name,
                    openstruct::sections::SectionShape::Box {
                        depth: section.depth,
                        width: section.width,
                        wall: section.web,
                    },
                ),
                SectionKind::Pipe => Section::from_shape(
                    &section.name,
                    openstruct::sections::SectionShape::Pipe {
                        diameter: section.depth,
                        wall: section.web,
                    },
                ),
                SectionKind::Angle => Section::from_shape(
                    &section.name,
                    openstruct::sections::SectionShape::Angle {
                        depth: section.depth,
                        width: section.width,
                        thickness: section.web,
                    },
                ),
            };
            let Some(mut built) = built else {
                self.status_text = format!("{} has invalid dimensions for its shape.", section.name);
                return false;
            };
            built.modifiers = section.modifiers;
            sections.insert(id, built);
        }
        if let Some(element) = self.model.elements.values().find(|element| !sections.contains_key(&element.section_id)) {
            self.status_text = format!("Frame {} still uses a section that is no longer defined.", element.id);
            return false;
        }
        self.push_undo();
        self.model.section_counter = next_id.max(self.model.section_counter);
        self.model.sections = sections;
        self.status_text = format!("{} frame section(s) defined.", self.model.sections.len());
        true
    }

    pub fn assign_section(&mut self, section_id: usize) {
        if self.selected_elements.is_empty() {
            self.status_text = "Select frames to assign a section.".to_string();
            return;
        }
        self.push_undo();
        for &id in &self.selected_elements {
            if let Some(element) = self.model.elements.get_mut(&id) {
                element.section_id = section_id;
            }
        }
        self.status_text = format!("Section assigned to {} frame(s).", self.selected_elements.len());
    }

    // -- phase 3: load cases and combinations ---------------------------------

    pub fn open_load_cases(&mut self) {
        self.load_case_draft = self.model.load_cases.clone();
        self.load_case_new_name.clear();
        self.define_load_cases_dialog_open = true;
    }

    pub fn add_load_case(&mut self) {
        let mut name = self.load_case_new_name.trim().to_uppercase();
        if name.is_empty() {
            name = format!("CASE {}", self.load_case_draft.len() + 1);
        }
        if self.load_case_draft.iter().any(|case| case.name.eq_ignore_ascii_case(&name)) {
            self.status_text = format!("Case {name} already exists.");
            return;
        }
        let id = self.load_case_draft.iter().map(|case| case.id).max().unwrap_or(0) + 1;
        let pattern = self.model.load_patterns.first().map(|pattern| pattern.load_type.clone()).unwrap_or(LoadType::Dead);
        self.load_case_draft.push(LoadCase {
            id,
            name,
            pattern,
            self_weight_multiplier: 1.0,
            nodal_loads: Vec::new(),
            distributed_loads: Vec::new(),
            p_delta: false,
        });
        self.load_case_new_name.clear();
        self.status_text = format!("{} case(s) in the draft.", self.load_case_draft.len());
    }

    pub fn apply_load_cases(&mut self) -> bool {
        let mut seen: Vec<String> = Vec::new();
        let mut error: Option<String> = None;
        for case in &mut self.load_case_draft {
            case.name = case.name.trim().to_uppercase();
            if case.name.is_empty() {
                error = Some("Every load case needs a name.".to_string());
                break;
            }
            if seen.iter().any(|name| name == &case.name) {
                error = Some(format!("Duplicate load case name: {}.", case.name));
                break;
            }
            if case.self_weight_multiplier < 0.0 {
                error = Some(format!("{} has a negative self-weight multiplier.", case.name));
                break;
            }
            seen.push(case.name.clone());
        }
        if let Some(message) = error {
            self.status_text = message;
            return false;
        }
        self.push_undo();
        self.model.load_cases = self.load_case_draft.clone();
        // Loads whose case was deleted fall back to applying to every case.
        let case_ids: Vec<usize> = self.model.load_cases.iter().map(|case| case.id).collect();
        for load in &mut self.model.frame_loads {
            if load.case_id.is_some_and(|id| !case_ids.contains(&id)) {
                load.case_id = None;
            }
        }
        for load in &mut self.model.area_loads {
            if load.case_id.is_some_and(|id| !case_ids.contains(&id)) {
                load.case_id = None;
            }
        }
        self.status_text = format!("{} load case(s) defined; the analysis runs the first case.", self.model.load_cases.len());
        true
    }

    pub fn open_combos(&mut self) {
        self.combo_draft = self.model.load_combinations.clone();
        self.combo_new_name.clear();
        self.combo_new_equation.clear();
        self.define_combos_dialog_open = true;
    }

    pub fn add_combo(&mut self) {
        let mut name = self.combo_new_name.trim().to_string();
        let mut equation = self.combo_new_equation.trim().to_string();
        if name.is_empty() && equation.is_empty() {
            self.status_text = "Enter a combination name or equation.".to_string();
            return;
        }
        if name.is_empty() {
            name = equation.clone();
        }
        if equation.is_empty() {
            equation = name.clone();
        }
        if self.combo_draft.iter().any(|combo| combo.name.eq_ignore_ascii_case(&name)) {
            self.status_text = format!("Combination {name} already exists.");
            return;
        }
        let id = self.combo_draft.iter().map(|combo| combo.id).max().unwrap_or(0) + 1;
        self.combo_draft.push(LoadCombination { id, name, equation, combo_type: ComboType::LinearAdd, factors: Vec::new() });
        self.combo_new_name.clear();
        self.combo_new_equation.clear();
    }

    pub fn add_code_combos(&mut self) {
        let mut added = 0usize;
        for (name, combo_type, equation) in openstruct::loads::code_default_hints() {
            if self.combo_draft.iter().any(|combo| combo.name.eq_ignore_ascii_case(&name)) {
                continue;
            }
            let id = self.combo_draft.iter().map(|combo| combo.id).max().unwrap_or(0) + 1;
            self.combo_draft.push(LoadCombination { id, name, equation, combo_type, factors: Vec::new() });
            added += 1;
        }
        self.status_text = format!("Added {added} code default combination(s).");
    }

    pub fn apply_combos(&mut self) -> bool {
        let mut seen: Vec<String> = Vec::new();
        let mut error: Option<String> = None;
        for combo in &mut self.combo_draft {
            combo.name = combo.name.trim().to_string();
            if combo.name.is_empty() {
                error = Some("Every combination needs a name.".to_string());
                break;
            }
            if seen.iter().any(|name| name.eq_ignore_ascii_case(&combo.name)) {
                error = Some(format!("Duplicate combination name: {}.", combo.name));
                break;
            }
            seen.push(combo.name.clone());
        }
        if let Some(message) = error {
            self.status_text = message;
            return false;
        }
        self.push_undo();
        self.model.load_combinations = self.combo_draft.clone();
        self.status_text = format!("{} load combination(s) defined.", self.model.load_combinations.len());
        true
    }

    // -- documents & undo ----------------------------------------------------

    pub fn to_doc(&self) -> Doc {
        Doc {
            model: self.model.clone(),
            nodal_loads: self.nodal_loads.clone(),
            grid: self.grid.clone(),
            ref_planes: self.ref_planes.clone(),
            master_story: self.master_story,
            groups: self.groups.clone(),
        }
    }

    pub fn apply_doc(&mut self, doc: Doc) {
        self.model = doc.model;
        self.nodal_loads = doc.nodal_loads;
        self.grid = doc.grid;
        self.ref_planes = doc.ref_planes;
        self.master_story = doc.master_story.filter(|index| *index < self.model.stories.len());
        self.groups = doc.groups;
        self.story_master_draft = self.master_story;
        if let Some(work) = self.work_plane {
            if !self.ref_planes.iter().any(|plane| plane.axis == work.axis && (plane.offset - work.offset).abs() < 1e-9) {
                self.work_plane = None;
            }
        }
        if self.model.stories.is_empty() {
            self.model.stories.push(Story::new("Story1", 0.0));
        }
        self.active_story = self.active_story.min(self.model.stories.len().saturating_sub(1));
        self.story_draft = self.model.stories.clone();
        self.results = None;
        self.combo_results.clear();
        self.member_envelopes.clear();
        self.result_source = None;
        self.selected_nodes.clear();
        self.selected_elements.clear();
        self.selected_areas.clear();
        self.draw_frame_start = None;
        self.area_points.clear();
        self.draw_wall_start = None;
        self.drag_select_start = None;
        self.edit_mode = None;
    }

    pub fn push_undo(&mut self) {
        self.undo.push(self.to_doc());
        if self.undo.len() > MAX_UNDO {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.results = None;
        self.combo_results.clear();
        self.member_envelopes.clear();
        self.result_source = None;
        self.dirty = true;
    }

    pub fn undo(&mut self) {
        if let Some(previous) = self.undo.pop() {
            let current = self.to_doc();
            self.redo.push(current);
            self.apply_doc(previous);
            self.dirty = true;
            self.status_text = "Undo.".to_string();
        } else {
            self.status_text = "Nothing to undo.".to_string();
        }
    }

    pub fn redo(&mut self) {
        if let Some(next) = self.redo.pop() {
            let current = self.to_doc();
            self.undo.push(current);
            self.apply_doc(next);
            self.dirty = true;
            self.status_text = "Redo.".to_string();
        } else {
            self.status_text = "Nothing to redo.".to_string();
        }
    }

    fn load_benchmark_model(&mut self) {
        self.push_undo();
        self.model = Model::benchmark_building();
        self.grid = GridSpec::new(5.0, 9, 9);
        self.ref_planes.clear();
        self.work_plane = None;
        self.nodal_loads.clear();
        self.groups.clear();
        self.results = None;
        self.selected_nodes.clear();
        self.selected_elements.clear();
        self.selected_areas.clear();
        self.draw_frame_start = None;
        self.area_points.clear();
        self.draw_wall_start = None;
        self.story_draft.clear();
        self.master_story = None;
        self.story_master_draft = None;
        self.active_story = 0;
        self.camera = Camera3D::default();
        self.view = View { center: (20.0, 20.0), scale: 32.0 };
        self.view_tab = ViewTab::View3D;
        self.show_deformed = false;
        self.frame_ms = 0.0;
        self.file_path = None;
        self.pending_fit = Some(());
        self.status_text = format!(
            "Benchmark: {} joints, {} frames, {} stories. Orbit the 3D view; the header shows smoothed frame time.",
            self.model.nodes.len(),
            self.model.elements.len(),
            self.model.stories.len()
        );
    }

    fn reset_model(&mut self) {
        self.push_undo();
        let (model, grid) = blank_model();
        self.model = model;
        self.grid = grid;
        self.ref_planes.clear();
        self.nodal_loads.clear();
        self.groups.clear();
        self.selected_nodes.clear();
        self.selected_elements.clear();
        self.selected_areas.clear();
        self.results = None;
        self.file_path = None;
        self.draw_frame_start = None;
        self.area_points.clear();
        self.draw_wall_start = None;
        self.active_story = 0;
        self.camera = Camera3D::default();
        self.view = View { center: (10.0, 7.5), scale: 32.0 };
        self.pending_fit = Some(());
        self.status_text = "New model created.".to_string();
    }

    // -- file I/O ------------------------------------------------------------

    /// File → Import E2K…: read an E2K text export and make it the open model.
    fn import_e2k_file(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("E2K text", &["e2k", "txt"])
            .pick_file()
        else {
            return;
        };
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) => {
                self.status_text = format!("E2K import failed: {err}");
                return;
            }
        };
        match openstruct::e2k::parse_e2k(&text) {
            Ok((model, _analysis, report)) => {
                self.push_undo();
                self.adopt_imported_model(model, &report);
                self.status_text = report.summary();
                self.dirty = true;
                self.import_e2k_report = Some(report);
                self.e2k_report_dialog_open = true;
            }
            Err(err) => self.status_text = format!("E2K import failed: {err}"),
        }
    }

    /// Make an imported model the open one, with a grid built from its joints.
    fn adopt_imported_model(&mut self, model: Model, report: &openstruct::e2k::E2kImportReport) {
        self.model = model;
        self.master_story = report
            .master_story
            .as_deref()
            .and_then(|name| self.model.stories.iter().position(|story| story.name == name));
        self.story_master_draft = self.master_story;

        let mut xs: Vec<f64> = self.model.nodes.values().map(|node| node.x).collect();
        let mut ys: Vec<f64> = self.model.nodes.values().map(|node| node.y).collect();
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        ys.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        xs.dedup_by(|a, b| (*a - *b).abs() < 1.0e-6);
        ys.dedup_by(|a, b| (*a - *b).abs() < 1.0e-6);
        self.grid = if xs.len() >= 2 && ys.len() >= 2 {
            GridSpec::from_ordinates(xs, ys)
        } else {
            GridSpec::new(5.0, 4, 3)
        };
        self.grid_spacing = self.grid.xs.windows(2).next().map(|pair| pair[1] - pair[0]).unwrap_or(5.0);
        self.grid_nx = self.grid.xs.len().saturating_sub(1) as i32;
        self.grid_ny = self.grid.ys.len().saturating_sub(1) as i32;
        self.grid_x_labels.clear();
        self.grid_y_labels.clear();

        self.units = UnitSystem::Metric;
        self.results = None;
        self.combo_results.clear();
        self.member_envelopes.clear();
        self.model_check_report = None;
        self.ref_planes.clear();
        self.work_plane = None;
        self.nodal_loads.clear();
        self.groups.clear();
        self.selected_nodes.clear();
        self.selected_elements.clear();
        self.selected_areas.clear();
        self.draw_frame_start = None;
        self.area_points.clear();
        self.draw_wall_start = None;
        self.active_story = 0;
        self.camera = Camera3D::default();
        self.show_deformed = false;
        self.view_tab = if self.model.elements.is_empty() { ViewTab::Plan } else { ViewTab::View3D };
        self.file_path = None;
        self.pending_fit = Some(());
    }

    fn open_file(&mut self) {
        let Some(path) = rfd::FileDialog::new().add_filter("OpenStruct model", &["ostrut", "json"]).pick_file() else {
            return;
        };
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) => {
                self.status_text = format!("Open failed: {err}");
                return;
            }
        };
        let parsed = serde_json::from_str::<Doc>(&text).or_else(|_| serde_json::from_str::<Model>(&text).map(Doc::from_model));
        match parsed {
            Ok(doc) => {
                self.push_undo();
                self.apply_doc(doc);
                self.file_path = Some(path);
                self.dirty = false;
                self.pending_fit = Some(());
                self.status_text = "Model opened.".to_string();
            }
            Err(err) => self.status_text = format!("Open failed: {err}"),
        }
    }

    fn save_file(&mut self, save_as: bool) {
        let existing = self.file_path.clone();
        let chosen = if save_as || existing.is_none() {
            let mut dialog = rfd::FileDialog::new().add_filter("OpenStruct model", &["ostrut"]);
            if let Some(path) = &existing {
                if let Some(parent) = path.parent() {
                    dialog = dialog.set_directory(parent);
                }
            }
            dialog.save_file()
        } else {
            existing
        };
        let Some(mut path) = chosen else { return };
        if path.extension().is_none() {
            path.set_extension("ostrut");
        }
        let json = match serde_json::to_string_pretty(&self.to_doc()) {
            Ok(json) => json,
            Err(err) => {
                self.status_text = format!("Save failed: {err}");
                return;
            }
        };
        match std::fs::write(&path, json) {
            Ok(()) => {
                self.status_text = format!("Saved {}.", path.display());
                self.file_path = Some(path);
                self.dirty = false;
            }
            Err(err) => self.status_text = format!("Save failed: {err}"),
        }
    }

    // -- model helpers -------------------------------------------------------

    pub fn default_material_id(&mut self) -> usize {
        if self.model.materials.contains_key(&1) { 1 } else { self.model.add_material(Material::steel()) }
    }

    pub fn default_section_id(&mut self) -> usize {
        if self.model.sections.contains_key(&1) { 1 } else { self.model.add_section(Section::rectangular("RECT300X500", 0.3, 0.5)) }
    }

    // -- editing actions -----------------------------------------------------

    pub fn assign_restraints(&mut self, pattern: [bool; 6]) {
        if self.selected_nodes.is_empty() {
            self.status_text = "Select joints first, then assign a restraint.".to_string();
            return;
        }
        self.push_undo();
        for &id in &self.selected_nodes {
            if let Some(node) = self.model.nodes.get_mut(&id) {
                node.restrained = pattern;
            }
        }
        self.status_text = format!("Restraints assigned to {} joint(s).", self.selected_nodes.len());
    }

    pub fn apply_loads_to_selection(&mut self, clear: bool) {
        if self.selected_nodes.is_empty() {
            self.status_text = "Select joints first, then assign the load.".to_string();
            return;
        }
        self.push_undo();
        for &id in &self.selected_nodes {
            if clear {
                self.nodal_loads.remove(&id);
            } else if self.load_value != [0.0, 0.0, 0.0] {
                self.nodal_loads.insert(id, self.load_value);
            }
        }
        self.status_text = if clear {
            format!("Loads removed from {} joint(s).", self.selected_nodes.len())
        } else {
            format!(
                "Point load ({:.1}, {:.1}, {:.1}) kN assigned to {} joint(s).",
                self.load_value[0],
                self.load_value[1],
                self.load_value[2],
                self.selected_nodes.len()
            )
        };
    }

    pub fn delete_selection(&mut self) {
        if self.selected_nodes.is_empty() && self.selected_elements.is_empty() && self.selected_areas.is_empty() {
            self.status_text = "Nothing selected.".to_string();
            return;
        }
        self.push_undo();
        let mut areas_removed = self.selected_areas.len();
        for &aid in &self.selected_areas {
            self.model.areas.remove(&aid);
        }
        let frames = self.selected_elements.len();
        for &eid in &self.selected_elements {
            self.model.elements.remove(&eid);
        }
        let removed_nodes: Vec<usize> =
            self.selected_nodes.iter().copied().filter(|nid| !self.model.elements.values().any(|e| e.node_i == *nid || e.node_j == *nid)).collect();
        for &nid in &removed_nodes {
            self.model.nodes.remove(&nid);
            self.nodal_loads.remove(&nid);
            if self.draw_frame_start == Some(nid) {
                self.draw_frame_start = None;
            }
        }
        if !removed_nodes.is_empty() {
            self.model.areas.retain(|_, area| {
                let keep = !area.node_ids.iter().any(|id| removed_nodes.contains(id));
                if !keep {
                    areas_removed += 1;
                }
                keep
            });
        }
        for group in &mut self.groups {
            group.nodes.retain(|id| self.model.nodes.contains_key(id));
            group.elements.retain(|id| self.model.elements.contains_key(id));
            group.areas.retain(|id| self.model.areas.contains_key(id));
        }
        self.selected_nodes.clear();
        self.selected_elements.clear();
        self.selected_areas.clear();
        self.area_points.clear();
        self.draw_wall_start = None;
        self.status_text = format!("Deleted {} joint(s), {frames} frame(s) and {areas_removed} area(s).", removed_nodes.len());
    }

    // -- analysis ------------------------------------------------------------

    pub fn run_analysis(&mut self) {
        if self.model.elements.is_empty() {
            self.status_text = "Nothing to analyze: draw some frames first.".to_string();
            self.results = None;
            return;
        }
        if self.check_before_solve {
            let report = openstruct::check::check_model(&self.model);
            if !report.can_analyse() {
                self.status_text = format!("Analysis stopped by the model check: {}", report.summary());
                self.model_check_report = Some(report);
                self.model_check_dialog_open = true;
                self.results = None;
                return;
            }
            self.model_check_report = Some(report);
        }
        let loads: Vec<NodalLoad> = self
            .nodal_loads
            .iter()
            .map(|(&id, &load)| NodalLoad { node_id: id, fx: load[0] * 1000.0, fy: load[1] * 1000.0, fz: load[2] * 1000.0, mx: 0.0, my: 0.0, mz: 0.0 })
            .collect();

        let mut analysis = Analysis::new(self.model.clone());
        if self.model.load_cases.is_empty() {
            analysis.load_cases.push(LoadCase {
                id: 1,
                name: "CASE 1".into(),
                pattern: LoadType::Dead,
                self_weight_multiplier: 1.0,
                nodal_loads: loads,
                distributed_loads: vec![],
                p_delta: false,
            });
        } else {
            let mut cases = self.model.load_cases.clone();
            if !loads.is_empty() {
                // Point loads assigned on the canvas belong to the first case.
                cases[0].nodal_loads.extend(loads);
            }
            analysis.load_cases = cases;
        }
        let results = analysis.run_static();
        if self.solve_combinations {
            self.combo_results = analysis.run_combinations();
            self.member_envelopes = {
                let sets: Vec<&openstruct::analysis::AnalysisResults> = self.combo_results.iter().map(|combo| &combo.results).collect();
                openstruct::analysis::member_envelopes(&sets)
            };
        } else {
            self.combo_results.clear();
            self.member_envelopes.clear();
        }
        self.result_source = None;
        let max_disp = results.displacements.iter().fold(0.0_f64, |max, &value| max.max(value.abs()));
        self.status_text = format!("{}  |  max |disp| = {:.3e} m", results.status, max_disp);
        if !self.model.areas.is_empty() {
            self.status_text.push_str(&format!("  |  {} area(s) excluded (shell solver in Phase 4).", self.model.areas.len()));
        }
        self.results = Some(results);
        self.show_deformed = self.auto_deformed;
    }

    fn prefs(&self) -> Prefs {
        Prefs {
            dark: self.dark,
            units: self.units,
            show_grid: self.show_grid,
            show_loads: self.show_loads,
            show_explorer: self.show_explorer,
            show_properties: self.show_properties,
            show_tables: self.show_tables,
            show_joints: self.show_joints,
            show_frames: self.show_frames,
            show_areas: self.show_areas,
            show_labels: self.show_labels,
            show_supports: self.show_supports,
            show_dimensions: self.show_dimensions,
            show_ref_planes: self.show_ref_planes,
            show_frame_loads: self.show_frame_loads,
            show_area_loads: self.show_area_loads,
            load_symbol_scale: self.load_symbol_scale,
        }
    }

    // -- keyboard ------------------------------------------------------------

    fn handle_shortcuts(&mut self) {
        let typing = self.ctx.egui_wants_keyboard_input();
        let mut actions: Vec<Cmd> = Vec::new();
        let mut escape = false;
        let ctx = self.ctx.clone();
        ctx.input_mut(|input| {
            if input.consume_key(egui::Modifiers::COMMAND, egui::Key::N) {
                actions.push(Cmd::New);
            }
            if input.consume_key(egui::Modifiers::COMMAND, egui::Key::O) {
                actions.push(Cmd::Open);
            }
            if input.consume_key(egui::Modifiers::COMMAND, egui::Key::S) {
                actions.push(Cmd::Save);
            }
            if input.consume_key(egui::Modifiers::COMMAND, egui::Key::Z) {
                actions.push(Cmd::Undo);
            }
            if input.consume_key(egui::Modifiers::COMMAND, egui::Key::Y) {
                actions.push(Cmd::Redo);
            }
            if input.consume_key(egui::Modifiers::COMMAND, egui::Key::A) {
                actions.push(Cmd::SelectAll);
            }
            if input.consume_key(egui::Modifiers::COMMAND, egui::Key::K) {
                actions.push(Cmd::CommandPalette);
            }
            if !typing {
                if input.consume_key(egui::Modifiers::NONE, egui::Key::F5) {
                    actions.push(Cmd::RunAnalysis);
                }
                if input.consume_key(egui::Modifiers::NONE, egui::Key::F6) {
                    actions.push(Cmd::ToggleDeformed);
                }
                if input.consume_key(egui::Modifiers::NONE, egui::Key::F7) {
                    actions.push(Cmd::ToggleGrid);
                }
                if input.consume_key(egui::Modifiers::NONE, egui::Key::Delete) {
                    actions.push(Cmd::Delete);
                }
                if input.consume_key(egui::Modifiers::NONE, egui::Key::Num1) {
                    actions.push(Cmd::ToolSelect);
                }
                if input.consume_key(egui::Modifiers::NONE, egui::Key::Num2) {
                    actions.push(Cmd::DrawJoint);
                }
                if input.consume_key(egui::Modifiers::NONE, egui::Key::Num3) {
                    actions.push(Cmd::DrawFrame);
                }
                if input.consume_key(egui::Modifiers::NONE, egui::Key::Num4) {
                    actions.push(Cmd::DrawArea);
                }
                if input.consume_key(egui::Modifiers::NONE, egui::Key::Num5) {
                    actions.push(Cmd::DrawWall);
                }
                if input.consume_key(egui::Modifiers::NONE, egui::Key::Escape) {
                    escape = true;
                }
            }
        });

        for cmd in actions {
            self.run(cmd);
        }

        if escape {
            if self.palette_open {
                self.palette_open = false;
            } else if self.edit_mode.is_some() {
                self.edit_mode = None;
            } else if self.draw_frame_start.take().is_some() {
                self.status_text = "Frame drawing chain cancelled.".to_string();
            } else if self.draw_wall_start.take().is_some() {
                self.status_text = "Wall chain cancelled.".to_string();
            } else if !self.area_points.is_empty() {
                self.area_points.clear();
                self.status_text = "Area drawing cancelled.".to_string();
            } else if !self.selected_nodes.is_empty() || !self.selected_elements.is_empty() || !self.selected_areas.is_empty() {
                self.selected_nodes.clear();
                self.selected_elements.clear();
                self.selected_areas.clear();
                self.status_text = "Selection cleared.".to_string();
            }
        }
    }
}

/// Seed an editor row from a stored section, using its parametric shape when it
/// has one and falling back to the stored dimensions for older files.
fn draft_from_section(id: usize, section: &Section) -> SectionDraft {
    use openstruct::sections::SectionShape;
    let mut draft = SectionDraft {
        id,
        kind: SectionKind::Rectangular,
        name: section.name.clone(),
        width: section.width,
        depth: section.depth,
        web: 0.01,
        flange: 0.02,
        designer: Vec::new(),
        modifiers: section.modifiers,
    };
    if !section.designer.is_empty() {
        draft.kind = SectionKind::Custom;
        draft.designer = section.designer.clone();
        return draft;
    }
    let Some(shape) = section.shape else {
        if is_circular_section(section) {
            draft.kind = SectionKind::Circular;
        }
        return draft;
    };
    let (kind, width, depth, web, flange) = match shape {
        SectionShape::Rectangular { width, depth } => (SectionKind::Rectangular, width, depth, draft.web, draft.flange),
        SectionShape::Circular { diameter } => (SectionKind::Circular, diameter, diameter, draft.web, draft.flange),
        SectionShape::ISection { depth, width, web, flange } => (SectionKind::ISection, width, depth, web, flange),
        SectionShape::Channel { depth, width, web, flange } => (SectionKind::Channel, width, depth, web, flange),
        SectionShape::Tee { depth, width, web, flange } => (SectionKind::Tee, width, depth, web, flange),
        SectionShape::Box { depth, width, wall } => (SectionKind::Box, width, depth, wall, draft.flange),
        SectionShape::Pipe { diameter, wall } => (SectionKind::Pipe, diameter, diameter, wall, draft.flange),
        SectionShape::Angle { depth, width, thickness } => (SectionKind::Angle, width, depth, thickness, draft.flange),
    };
    draft.kind = kind;
    draft.width = width;
    draft.depth = depth;
    draft.web = web;
    draft.flange = flange;
    draft
}

/// Parametric shape described by a draft row, used to seed the designer.
fn shape_from_draft(draft: &SectionDraft) -> Option<openstruct::sections::SectionShape> {
    use openstruct::sections::SectionShape as Shape;
    Some(match draft.kind {
        SectionKind::Rectangular => Shape::Rectangular { width: draft.width, depth: draft.depth },
        SectionKind::Circular => Shape::Circular { diameter: draft.depth },
        SectionKind::ISection => Shape::ISection { depth: draft.depth, width: draft.width, web: draft.web, flange: draft.flange },
        SectionKind::Channel => Shape::Channel { depth: draft.depth, width: draft.width, web: draft.web, flange: draft.flange },
        SectionKind::Tee => Shape::Tee { depth: draft.depth, width: draft.width, web: draft.web, flange: draft.flange },
        SectionKind::Box => Shape::Box { depth: draft.depth, width: draft.width, wall: draft.web },
        SectionKind::Pipe => Shape::Pipe { diameter: draft.depth, wall: draft.web },
        SectionKind::Angle => Shape::Angle { depth: draft.depth, width: draft.width, thickness: draft.web },
        SectionKind::Custom => return None,
    })
}

/// A section counts as circular when its two dimensions match and its area is
/// that of the circle with the same diameter.
fn is_circular_section(section: &Section) -> bool {
    let d = section.depth;
    if d <= 0.0 {
        return false;
    }
    let circular_area = std::f64::consts::PI * (d / 2.0).powi(2);
    (section.width - d).abs() < 1e-9 && ((section.area - circular_area) / circular_area).abs() < 1e-6
}

fn blank_model() -> (Model, GridSpec) {
    let mut model = Model::new("Model 1");
    model.add_material(Material::steel());
    model.add_section(Section::rectangular("RECT300X500", 0.3, 0.5));
    (model, GridSpec::new(5.0, 4, 3))
}

impl eframe::App for OpenStructApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "openstruct.prefs", &self.prefs());
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.draw(ui);
    }
}

impl OpenStructApp {
    /// Draw the whole application shell into `ui`. Kept separate from the eframe
    /// trait so a headless test can drive exactly the same code.
    pub fn draw(&mut self, ui: &mut egui::Ui) {
        self.ctx = ui.ctx().clone();
        self.handle_shortcuts();

        // Minimum heights, not fixed heights: the toolbar grows when the draw tools
        // add their own controls, and a fixed panel would clip the row and shift the
        // canvas instead of laying itself out again.
        egui::Panel::top("ostr_menu").resizable(false).min_size(28.0).show(ui, |ui| crate::menu::menu_bar(self, ui));
        egui::Panel::top("ostr_toolbar").resizable(false).min_size(38.0).show(ui, |ui| crate::toolbar::toolbar(self, ui));
        egui::Panel::top("ostr_view_tabs").resizable(false).min_size(30.0).show(ui, |ui| crate::toolbar::view_tabs(self, ui));
        // One line only: the bar's message is truncated in `statusbar`, so the panel
        // cannot grow when a long hint appears and shrink the canvas under the pointer.
        let status = egui::Panel::bottom("ostr_status").resizable(false).min_size(26.0).show(ui, |ui| crate::statusbar::statusbar(self, ui));
        self.last_status_rect = Some(status.response.rect);

        if self.show_explorer {
            egui::Panel::left("ostr_explorer").resizable(true).default_size(224.0).min_size(160.0).show(ui, |ui| crate::explorer::explorer(self, ui));
        }
        if self.show_properties {
            egui::Panel::right("ostr_properties").resizable(true).default_size(304.0).min_size(210.0).show(ui, |ui| crate::properties::properties(self, ui));
        }
        if self.show_tables {
            // Fixed height: a resizable panel sizes itself to its content, and the
            // table body is bounded in `tables`, so the dock stays 222 px and the
            // canvas keeps its place when rows appear or disappear.
            let tables = egui::Panel::bottom("ostr_tables")
                .resizable(false)
                .default_size(222.0)
                .min_size(222.0)
                .show(ui, |ui| crate::tables::tables(self, ui));
            self.last_tables_rect = Some(tables.response.rect);
        } else {
            self.last_tables_rect = None;
        }

        egui::CentralPanel::default().show(ui, |ui| match self.view_tab {
            ViewTab::Plan => self.draw_canvas(ui),
            ViewTab::View3D => self.draw_view3d(ui),
        });

        let ctx = self.ctx.clone();
        crate::dialogs::windows(self, &ctx);
    }
}
