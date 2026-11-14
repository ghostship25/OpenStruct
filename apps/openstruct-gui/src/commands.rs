//! The single command registry.
//!
//! Every action in the application has exactly one [`Cmd`] variant. Menus, toolbar
//! buttons, keyboard shortcuts and the command palette all dispatch through
//! [`OpenStructApp::run`](crate::app::OpenStructApp::run), so an action can never
//! exist in one surface and be missing from another.
//!
//! Commands that are planned but not yet built return their phase from
//! [`Cmd::planned`]; the app reports that instead of silently doing nothing.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cmd {
    // File
    New,
    BuildingTemplate,
    Open,
    Save,
    SaveAs,
    ImportE2K,
    ExportE2K,
    Exit,
    // Edit
    Undo,
    Redo,
    Delete,
    MoveSelection,
    CopySelection,
    MirrorSelection,
    RotateSelection,
    AlignSelection,
    CommandPalette,
    // View
    ZoomFit,
    ZoomIn,
    ZoomOut,
    ViewPlan,
    View3D,
    ElevationView,
    ToggleGrid,
    ToggleDeformed,
    DisplayLoadSymbols,
    ToggleExplorer,
    ToggleProperties,
    ShowTables,
    // Define
    GridSystem,
    DefineStories,
    DefineRefPlanes,
    DefineMaterials,
    DefineFrameSections,
    DefineAreaSections,
    DefineLoadPatterns,
    DefineLoadCases,
    DefineCombos,
    DefineGroups,
    DefineDiaphragms,
    // Draw
    ToolSelect,
    DrawJoint,
    DrawFrame,
    DrawArea,
    DrawWall,
    TypeCoordinates,
    QuickDraw,
    Replicate,
    MergeJoints,
    SplitFrames,
    TrimExtend,
    CreateGridMembers,
    BenchmarkModel,
    ToggleShaded3d,
    ToggleClip3d,
    // Select
    SelectAll,
    DeselectAll,
    InvertSelection,
    SelectByProperty,
    SelectByStory,
    SelectByGroup,
    // Assign
    FixityFixed,
    FixityPinned,
    FixityPinnedTorsion,
    FixityRoller,
    FixityRollerTorsion,
    FixityFree,
    AssignPointLoad,
    RemovePointLoad,
    AssignFrameLoads,
    AssignAreaLoads,
    TemperatureLoads,
    AssignReleases,
    JointSprings,
    AssignDiaphragm,
    AssignGroups,
    // Analyze
    RunAnalysis,
    AnalysisOptions,
    ModelCheck,
    ResultsSetup,
    // Display
    FrameDiagrams,
    AreaContours,
    StoryResults,
    DisplayOptions,
    // Design
    SteelDesign,
    ConcreteDesign,
    CompositeDesign,
    ShearWallDesign,
    Detailing,
    Reports,
    // Options
    UnitsMetric,
    UnitsImperial,
    ThemeDark,
    ThemeLight,
    KeyboardShortcuts,
    ResetLayout,
    Preferences,
    // Window
    CloseView,
    // Help
    About,
    Documentation,
    VerificationCases,
}

impl Cmd {
    /// Every command, in menu order. Drives the command palette and the shortcut list.
    pub const ALL: &'static [Cmd] = &[
        Cmd::New,
        Cmd::BuildingTemplate,
        Cmd::Open,
        Cmd::Save,
        Cmd::SaveAs,
        Cmd::ImportE2K,
        Cmd::ExportE2K,
        Cmd::Exit,
        Cmd::Undo,
        Cmd::Redo,
        Cmd::Delete,
        Cmd::MoveSelection,
        Cmd::CopySelection,
        Cmd::MirrorSelection,
        Cmd::RotateSelection,
        Cmd::AlignSelection,
        Cmd::CommandPalette,
        Cmd::ZoomFit,
        Cmd::ZoomIn,
        Cmd::ZoomOut,
        Cmd::ViewPlan,
        Cmd::View3D,
        Cmd::ElevationView,
        Cmd::ToggleGrid,
        Cmd::ToggleDeformed,
        Cmd::DisplayLoadSymbols,
        Cmd::ToggleExplorer,
        Cmd::ToggleProperties,
        Cmd::ShowTables,
        Cmd::GridSystem,
        Cmd::DefineStories,
        Cmd::DefineRefPlanes,
        Cmd::DefineMaterials,
        Cmd::DefineFrameSections,
        Cmd::DefineAreaSections,
        Cmd::DefineLoadPatterns,
        Cmd::DefineLoadCases,
        Cmd::DefineCombos,
        Cmd::DefineGroups,
        Cmd::DefineDiaphragms,
        Cmd::ToolSelect,
        Cmd::DrawJoint,
        Cmd::DrawFrame,
        Cmd::DrawArea,
        Cmd::DrawWall,
        Cmd::TypeCoordinates,
        Cmd::QuickDraw,
        Cmd::Replicate,
        Cmd::MergeJoints,
        Cmd::SplitFrames,
        Cmd::TrimExtend,
        Cmd::CreateGridMembers,
        Cmd::BenchmarkModel,
        Cmd::ToggleShaded3d,
        Cmd::ToggleClip3d,
        Cmd::SelectAll,
        Cmd::DeselectAll,
        Cmd::InvertSelection,
        Cmd::SelectByProperty,
        Cmd::SelectByStory,
        Cmd::SelectByGroup,
        Cmd::FixityFixed,
        Cmd::FixityPinned,
        Cmd::FixityPinnedTorsion,
        Cmd::FixityRoller,
        Cmd::FixityRollerTorsion,
        Cmd::FixityFree,
        Cmd::AssignPointLoad,
        Cmd::RemovePointLoad,
        Cmd::AssignFrameLoads,
        Cmd::AssignAreaLoads,
        Cmd::TemperatureLoads,
        Cmd::AssignReleases,
        Cmd::JointSprings,
        Cmd::AssignDiaphragm,
        Cmd::AssignGroups,
        Cmd::RunAnalysis,
        Cmd::AnalysisOptions,
        Cmd::ModelCheck,
        Cmd::ResultsSetup,
        Cmd::FrameDiagrams,
        Cmd::AreaContours,
        Cmd::StoryResults,
        Cmd::DisplayOptions,
        Cmd::SteelDesign,
        Cmd::ConcreteDesign,
        Cmd::CompositeDesign,
        Cmd::ShearWallDesign,
        Cmd::Detailing,
        Cmd::Reports,
        Cmd::UnitsMetric,
        Cmd::UnitsImperial,
        Cmd::ThemeDark,
        Cmd::ThemeLight,
        Cmd::KeyboardShortcuts,
        Cmd::ResetLayout,
        Cmd::Preferences,
        Cmd::CloseView,
        Cmd::About,
        Cmd::Documentation,
        Cmd::VerificationCases,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Cmd::New => "New Model",
            Cmd::BuildingTemplate => "New Model…",
            Cmd::Open => "Open…",
            Cmd::Save => "Save",
            Cmd::SaveAs => "Save As…",
            Cmd::ImportE2K => "Import E2K…",
            Cmd::ExportE2K => "Export E2K…",
            Cmd::Exit => "Exit",
            Cmd::Undo => "Undo",
            Cmd::Redo => "Redo",
            Cmd::Delete => "Delete Selection",
            Cmd::MoveSelection => "Move Selection…",
            Cmd::CopySelection => "Copy Selection…",
            Cmd::MirrorSelection => "Mirror Selection…",
            Cmd::RotateSelection => "Rotate Selection…",
            Cmd::AlignSelection => "Align Selection…",
            Cmd::CommandPalette => "Command Palette…",
            Cmd::ZoomFit => "Zoom Fit",
            Cmd::ZoomIn => "Zoom In",
            Cmd::ZoomOut => "Zoom Out",
            Cmd::ViewPlan => "Plan View",
            Cmd::View3D => "3D View",
            Cmd::ElevationView => "Elevation View",
            Cmd::ToggleGrid => "Show Grid",
            Cmd::ToggleDeformed => "Deformed Shape",
            Cmd::DisplayLoadSymbols => "Load Symbols",
            Cmd::ToggleExplorer => "Model Explorer",
            Cmd::ToggleProperties => "Properties of Object",
            Cmd::ShowTables => "Tables",
            Cmd::GridSystem => "Grid System…",
            Cmd::DefineStories => "Stories…",
            Cmd::DefineRefPlanes => "Reference Planes…",
            Cmd::DefineMaterials => "Materials…",
            Cmd::DefineFrameSections => "Frame Sections…",
            Cmd::DefineAreaSections => "Area Sections…",
            Cmd::DefineLoadPatterns => "Load Patterns…",
            Cmd::DefineLoadCases => "Load Cases…",
            Cmd::DefineCombos => "Load Combinations…",
            Cmd::DefineGroups => "Groups…",
            Cmd::DefineDiaphragms => "Diaphragms…",
            Cmd::ToolSelect => "Select Object",
            Cmd::DrawJoint => "Draw Joints",
            Cmd::DrawFrame => "Draw Frames",
            Cmd::DrawArea => "Draw Areas / Slabs",
            Cmd::DrawWall => "Draw Walls",
            Cmd::TypeCoordinates => "Type Coordinates…",
            Cmd::QuickDraw => "Quick Draw",
            Cmd::Replicate => "Replicate…",
            Cmd::MergeJoints => "Merge Joints",
            Cmd::SplitFrames => "Split Frames",
            Cmd::TrimExtend => "Trim / Extend Frames",
            Cmd::CreateGridMembers => "Create Frames / Walls on Grid…",
            Cmd::BenchmarkModel => "Generate 5,000-Joint Benchmark",
            Cmd::ToggleShaded3d => "Shaded Sections (wgpu)",
            Cmd::ToggleClip3d => "Clip Above Active Story",
            Cmd::SelectAll => "Select All",
            Cmd::DeselectAll => "Deselect All",
            Cmd::InvertSelection => "Invert Selection",
            Cmd::SelectByProperty => "Select by Property",
            Cmd::SelectByStory => "Select by Story",
            Cmd::SelectByGroup => "Select by Group",
            Cmd::FixityFixed => "Fixed",
            Cmd::FixityPinned => "Pinned",
            Cmd::FixityPinnedTorsion => "Pinned + Torsion",
            Cmd::FixityRoller => "Roller (Z)",
            Cmd::FixityRollerTorsion => "Roller + Torsion",
            Cmd::FixityFree => "Free (no restraint)",
            Cmd::AssignPointLoad => "Assign Point Load",
            Cmd::RemovePointLoad => "Remove Point Load",
            Cmd::AssignFrameLoads => "Frame Loads…",
            Cmd::AssignAreaLoads => "Area Loads…",
            Cmd::TemperatureLoads => "Temperature / Strain Loads…",
            Cmd::AssignReleases => "Releases / Partial Fixity",
            Cmd::JointSprings => "Joint Springs…",
            Cmd::AssignDiaphragm => "Diaphragm Assignment…",
            Cmd::AssignGroups => "Groups…",
            Cmd::RunAnalysis => "Run Analysis",
            Cmd::AnalysisOptions => "Analysis Options…",
            Cmd::ModelCheck => "Model Check",
            Cmd::ResultsSetup => "Results Setup…",
            Cmd::FrameDiagrams => "Frame Force Diagrams",
            Cmd::AreaContours => "Area Contours",
            Cmd::StoryResults => "Story Results",
            Cmd::DisplayOptions => "Display Options…",
            Cmd::SteelDesign => "Steel Frame Design",
            Cmd::ConcreteDesign => "Concrete Frame Design",
            Cmd::CompositeDesign => "Composite Beam Design",
            Cmd::ShearWallDesign => "Shear Wall Design",
            Cmd::Detailing => "Detailing",
            Cmd::Reports => "Reports…",
            Cmd::UnitsMetric => "Metric (kN, m)",
            Cmd::UnitsImperial => "Imperial (kip, ft)",
            Cmd::ThemeDark => "Dark Theme",
            Cmd::ThemeLight => "Light Theme",
            Cmd::KeyboardShortcuts => "Keyboard Shortcuts…",
            Cmd::ResetLayout => "Reset Layout",
            Cmd::Preferences => "Preferences…",
            Cmd::CloseView => "Close View",
            Cmd::About => "About OpenStruct",
            Cmd::Documentation => "Documentation",
            Cmd::VerificationCases => "Verification Cases",
        }
    }

    pub fn shortcut(self) -> Option<&'static str> {
        match self {
            Cmd::New => Some("Ctrl+N"),
            Cmd::Open => Some("Ctrl+O"),
            Cmd::Save => Some("Ctrl+S"),
            Cmd::SaveAs => None,
            Cmd::Exit => Some("Alt+F4"),
            Cmd::Undo => Some("Ctrl+Z"),
            Cmd::Redo => Some("Ctrl+Y"),
            Cmd::Delete => Some("Del"),
            Cmd::CommandPalette => Some("Ctrl+K"),
            Cmd::ToggleGrid => Some("F7"),
            Cmd::ToggleDeformed => Some("F6"),
            Cmd::RunAnalysis => Some("F5"),
            Cmd::SelectAll => Some("Ctrl+A"),
            Cmd::ToolSelect => Some("1"),
            Cmd::DrawJoint => Some("2"),
            Cmd::DrawFrame => Some("3"),
            Cmd::DrawArea => Some("4"),
            Cmd::DrawWall => Some("5"),
            _ => None,
        }
    }

    /// `None` means implemented. `Some(n)` means "planned for Phase n" (see Phase.md).
    pub fn planned(self) -> Option<u8> {
        None
    }

    /// Menu text with the shortcut, or a phase tag for planned items.
    pub fn menu_text(self) -> String {
        match (self.shortcut(), self.planned()) {
            (_, Some(phase)) => format!("{}   ·  Phase {}", self.label(), phase),
            (Some(shortcut), None) => format!("{}    {}", self.label(), shortcut),
            (None, None) => self.label().to_string(),
        }
    }

    /// Subsequence match against the command label, for the palette.
    pub fn score(self, query: &str) -> Option<i32> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return Some(0);
        }
        let label = self.label().to_lowercase();
        let mut chars = label.chars();
        let mut score = 0;
        for c in q.chars() {
            let mut found = false;
            let mut steps = 0;
            for lc in chars.by_ref() {
                steps += 1;
                if lc == c {
                    found = true;
                    break;
                }
            }
            if !found {
                return None;
            }
            score += 10 - steps.min(9) as i32;
        }
        if label.starts_with(&q) {
            score += 25;
        } else if label.contains(&q) {
            score += 10;
        }
        Some(score)
    }
}
