//! The menu bar. Structure mirrors standard: File, Edit, View, Define, Draw, Select,
//! Assign, Analyze, Display, Design, Options, Window, Help.

use crate::app::{OpenStructApp, UnitSystem};
use crate::canvas::ViewPreset;
use crate::commands::Cmd;
use eframe::egui;

fn item(app: &mut OpenStructApp, ui: &mut egui::Ui, cmd: Cmd) {
    if ui.button(cmd.menu_text()).clicked() {
        app.run(cmd);
        ui.close();
    }
}

fn check_item(app: &mut OpenStructApp, ui: &mut egui::Ui, cmd: Cmd, checked: bool) {
    let text = format!("{} {}", if checked { "[x]" } else { "[ ]" }, cmd.label());
    if ui.button(text).clicked() {
        app.run(cmd);
        ui.close();
    }
}

pub fn menu_bar(app: &mut OpenStructApp, ui: &mut egui::Ui) {
    ui.spacing_mut().item_spacing.x = 2.0;
    ui.horizontal(|ui| {
        ui.menu_button("File", |ui| {
            item(app, ui, Cmd::New);
            item(app, ui, Cmd::BuildingTemplate);
            item(app, ui, Cmd::Open);
            item(app, ui, Cmd::Save);
            item(app, ui, Cmd::SaveAs);
            ui.separator();
            item(app, ui, Cmd::ImportE2K);
            item(app, ui, Cmd::ExportE2K);
            ui.separator();
            item(app, ui, Cmd::Exit);
        });

        ui.menu_button("Edit", |ui| {
            item(app, ui, Cmd::Undo);
            item(app, ui, Cmd::Redo);
            ui.separator();
            item(app, ui, Cmd::Delete);
            item(app, ui, Cmd::MoveSelection);
            item(app, ui, Cmd::CopySelection);
            item(app, ui, Cmd::MirrorSelection);
            item(app, ui, Cmd::RotateSelection);
            item(app, ui, Cmd::AlignSelection);
            ui.separator();
            item(app, ui, Cmd::SelectAll);
            item(app, ui, Cmd::DeselectAll);
            item(app, ui, Cmd::InvertSelection);
            ui.separator();
            item(app, ui, Cmd::CommandPalette);
        });

        ui.menu_button("View", |ui| {
            item(app, ui, Cmd::ViewPlan);
            item(app, ui, Cmd::View3D);
            item(app, ui, Cmd::ElevationView);
            ui.separator();
            ui.menu_button("3D Preset", |ui| {
                if ui.button("Isometric").clicked() {
                    app.set_view_preset(ViewPreset::Iso);
                    ui.close();
                }
                if ui.button("Front Elevation").clicked() {
                    app.set_view_preset(ViewPreset::Front);
                    ui.close();
                }
                if ui.button("Side Elevation").clicked() {
                    app.set_view_preset(ViewPreset::Side);
                    ui.close();
                }
                if ui.button("Top").clicked() {
                    app.set_view_preset(ViewPreset::Plan);
                    ui.close();
                }
            });
            ui.separator();
            item(app, ui, Cmd::ZoomFit);
            item(app, ui, Cmd::ZoomIn);
            item(app, ui, Cmd::ZoomOut);
            ui.separator();
            check_item(app, ui, Cmd::ToggleGrid, app.show_grid);
            check_item(app, ui, Cmd::ToggleDeformed, app.show_deformed);
            check_item(app, ui, Cmd::DisplayLoadSymbols, app.show_loads);
            ui.separator();
            check_item(app, ui, Cmd::ToggleExplorer, app.show_explorer);
            check_item(app, ui, Cmd::ToggleProperties, app.show_properties);
            check_item(app, ui, Cmd::ShowTables, app.show_tables);
        });

        // Ordered like a standard Define menu. Grid Systems and Stories are Edit-menu
        // dialogs there, so they sit after the Define items with our own extras.
        ui.menu_button("Define", |ui| {
            item(app, ui, Cmd::DefineMaterials);
            ui.menu_button("Section Properties", |ui| {
                item(app, ui, Cmd::DefineFrameSections);
                item(app, ui, Cmd::DefineAreaSections);
            });
            item(app, ui, Cmd::DefineDiaphragms);
            item(app, ui, Cmd::DefineGroups);
            ui.separator();
            item(app, ui, Cmd::DefineLoadPatterns);
            item(app, ui, Cmd::DefineLoadCases);
            item(app, ui, Cmd::DefineCombos);
            ui.separator();
            item(app, ui, Cmd::GridSystem);
            item(app, ui, Cmd::DefineStories);
            item(app, ui, Cmd::DefineRefPlanes);
            ui.separator();
            ui.label("Spring properties: Assign → Joint Springs");
            ui.label("Pier/spandrel labels, section cuts, functions, mass source,\nP-Delta and performance checks are not implemented yet.");
        });

        ui.menu_button("Draw", |ui| {
            item(app, ui, Cmd::ToolSelect);
            item(app, ui, Cmd::DrawJoint);
            item(app, ui, Cmd::DrawFrame);
            ui.separator();
            item(app, ui, Cmd::DrawArea);
            item(app, ui, Cmd::DrawWall);
            item(app, ui, Cmd::TypeCoordinates);
            item(app, ui, Cmd::QuickDraw);
            ui.separator();
            item(app, ui, Cmd::CreateGridMembers);
            item(app, ui, Cmd::Replicate);
            item(app, ui, Cmd::MirrorSelection);
            item(app, ui, Cmd::MergeJoints);
            item(app, ui, Cmd::SplitFrames);
            item(app, ui, Cmd::TrimExtend);
            ui.separator();
            item(app, ui, Cmd::BenchmarkModel);
        });

        ui.menu_button("Select", |ui| {
            item(app, ui, Cmd::SelectAll);
            item(app, ui, Cmd::DeselectAll);
            item(app, ui, Cmd::InvertSelection);
            ui.separator();
            item(app, ui, Cmd::SelectByProperty);
            item(app, ui, Cmd::SelectByStory);
            item(app, ui, Cmd::SelectByGroup);
        });

        ui.menu_button("Assign", |ui| {
            ui.menu_button("Joint Restraints", |ui| {
                item(app, ui, Cmd::FixityFixed);
                item(app, ui, Cmd::FixityPinned);
                item(app, ui, Cmd::FixityPinnedTorsion);
                item(app, ui, Cmd::FixityRoller);
                item(app, ui, Cmd::FixityRollerTorsion);
                item(app, ui, Cmd::FixityFree);
                ui.separator();
                ui.label("Plan-view floors need a +Torsion restraint,\notherwise the free rigid twist is unstable.");
            });
            item(app, ui, Cmd::JointSprings);
            ui.separator();
            ui.menu_button("Joint Loads", |ui| {
                item(app, ui, Cmd::AssignPointLoad);
                item(app, ui, Cmd::RemovePointLoad);
            });
            ui.separator();
            item(app, ui, Cmd::AssignFrameLoads);
            item(app, ui, Cmd::AssignAreaLoads);
            item(app, ui, Cmd::TemperatureLoads);
            ui.separator();
            item(app, ui, Cmd::AssignReleases);
            item(app, ui, Cmd::AssignDiaphragm);
        });

        ui.menu_button("Analyze", |ui| {
            item(app, ui, Cmd::RunAnalysis);
            ui.separator();
            item(app, ui, Cmd::AnalysisOptions);
            item(app, ui, Cmd::ModelCheck);
            item(app, ui, Cmd::ResultsSetup);
        });

        ui.menu_button("Display", |ui| {
            item(app, ui, Cmd::ShowTables);
            check_item(app, ui, Cmd::ToggleDeformed, app.show_deformed);
            check_item(app, ui, Cmd::DisplayLoadSymbols, app.show_loads);
            check_item(app, ui, Cmd::ToggleShaded3d, app.shade_3d);
            check_item(app, ui, Cmd::ToggleClip3d, app.clip_3d);
            ui.separator();
            item(app, ui, Cmd::FrameDiagrams);
            item(app, ui, Cmd::AreaContours);
            item(app, ui, Cmd::StoryResults);
            ui.separator();
            item(app, ui, Cmd::DisplayOptions);
        });

        ui.menu_button("Design", |ui| {
            item(app, ui, Cmd::SteelDesign);
            item(app, ui, Cmd::ConcreteDesign);
            item(app, ui, Cmd::CompositeDesign);
            item(app, ui, Cmd::ShearWallDesign);
            ui.separator();
            item(app, ui, Cmd::Detailing);
            item(app, ui, Cmd::Reports);
        });

        ui.menu_button("Options", |ui| {
            ui.menu_button("Units", |ui| {
                if ui.selectable_label(app.units == UnitSystem::Metric, Cmd::UnitsMetric.label()).clicked() {
                    app.run(Cmd::UnitsMetric);
                    ui.close();
                }
                if ui.selectable_label(app.units == UnitSystem::Imperial, Cmd::UnitsImperial.label()).clicked() {
                    app.run(Cmd::UnitsImperial);
                    ui.close();
                }
            });
            ui.menu_button("Theme", |ui| {
                if ui.selectable_label(app.dark, "Dark").clicked() {
                    app.run(Cmd::ThemeDark);
                    ui.close();
                }
                if ui.selectable_label(!app.dark, "Light").clicked() {
                    app.run(Cmd::ThemeLight);
                    ui.close();
                }
            });
            ui.separator();
            item(app, ui, Cmd::KeyboardShortcuts);
            item(app, ui, Cmd::ResetLayout);
            item(app, ui, Cmd::Preferences);
        });

        ui.menu_button("Window", |ui| {
            check_item(app, ui, Cmd::ToggleExplorer, app.show_explorer);
            check_item(app, ui, Cmd::ToggleProperties, app.show_properties);
            check_item(app, ui, Cmd::ShowTables, app.show_tables);
            ui.separator();
            item(app, ui, Cmd::CloseView);
        });

        ui.menu_button("Help", |ui| {
            item(app, ui, Cmd::About);
            ui.separator();
            item(app, ui, Cmd::Documentation);
            item(app, ui, Cmd::VerificationCases);
        });
    });
}
