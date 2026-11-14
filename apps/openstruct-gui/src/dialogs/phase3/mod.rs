//! Phase 3 dialog scaffolding: materials, frame sections, load patterns, load cases,
//! combinations and preferences stubs the GUI wires to next.

pub(crate) mod materials_dialog;
pub(crate) mod sections_dialog;
pub(crate) mod section_designer_dialog;
pub(crate) mod patterns_dialog;
pub(crate) mod cases_dialog;
pub(crate) mod combos_dialog;
pub(crate) mod preferences_dialog;
pub(crate) mod assign_frame_loads_dialog;
pub(crate) mod assign_area_loads_dialog;
pub(crate) mod frame_releases_dialog;
pub(crate) mod joint_springs_dialog;
pub(crate) mod temperature_loads_dialog;
pub(crate) mod diaphragm_dialog;

pub use materials_dialog::open_materials_dialog;
pub use sections_dialog::open_frame_sections_dialog;
pub use section_designer_dialog::open_section_designer_dialog;
pub use patterns_dialog::open_patterns_dialog;
pub use cases_dialog::open_cases_dialog;
pub use combos_dialog::open_combos_dialog;
pub use preferences_dialog::open_preferences_dialog;
pub use assign_frame_loads_dialog::open_frame_loads_dialog;
pub use assign_area_loads_dialog::open_area_loads_dialog;
pub use frame_releases_dialog::open_frame_releases_dialog;
pub use joint_springs_dialog::open_joint_springs_dialog;
pub use temperature_loads_dialog::open_temperature_loads_dialog;
pub use diaphragm_dialog::open_diaphragm_dialog;
