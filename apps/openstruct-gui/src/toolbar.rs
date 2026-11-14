//! Icon toolbars and the model view tab strip.

use crate::app::{AreaShape, OpenStructApp, Tool, UnitSystem, ViewTab};
use crate::canvas::ViewPreset;
use crate::commands::Cmd;
use crate::icons::{Icon, icon_button};
use crate::theme;
use eframe::egui;
use openstruct::elements::CardinalPoint;

pub fn toolbar(app: &mut OpenStructApp, ui: &mut egui::Ui) {
    ui.horizontal_centered(|ui| {
        // The draw tools add controls to this row (area shape, wall height, frame
        // insertion). The left-hand group scrolls inside the space it is given so
        // the row can never overflow, and the theme/units controls on the right
        // stay pinned and clickable whatever tool is active.
        let reserved = 240.0_f32.min(ui.available_width() * 0.45);
        let left_width = (ui.available_width() - reserved).max(140.0);
        // The bar stays hidden on purpose: a scrollbar appearing would make the
        // panel taller and shift the canvas down, which is the jump we are fixing.
        egui::ScrollArea::horizontal()
            .id_salt("ostr_toolbar_scroll")
            .max_width(left_width)
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
            .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
        // Main.
        if icon_button(ui, Icon::New, "New model (Ctrl+N)", false).clicked() {
            app.run(Cmd::New);
        }
        if icon_button(ui, Icon::Open, "Open model (Ctrl+O)", false).clicked() {
            app.run(Cmd::Open);
        }
        if icon_button(ui, Icon::Save, "Save model (Ctrl+S)", false).clicked() {
            app.run(Cmd::Save);
        }
        ui.separator();
        if icon_button(ui, Icon::Undo, "Undo (Ctrl+Z)", false).clicked() {
            app.run(Cmd::Undo);
        }
        if icon_button(ui, Icon::Redo, "Redo (Ctrl+Y)", false).clicked() {
            app.run(Cmd::Redo);
        }
        ui.separator();

        // Draw tools.
        if icon_button(ui, Icon::Select, "Select object (1)", app.tool == Tool::Select).clicked() {
            app.run(Cmd::ToolSelect);
        }
        if icon_button(ui, Icon::Joint, "Draw joints (2)", app.tool == Tool::DrawNode).clicked() {
            app.run(Cmd::DrawJoint);
        }
        if icon_button(ui, Icon::Frame, "Draw frames (3)", app.tool == Tool::DrawFrame).clicked() {
            app.run(Cmd::DrawFrame);
        }
        if icon_button(ui, Icon::Area, "Draw areas / slabs (4)", app.tool == Tool::DrawArea).clicked() {
            app.run(Cmd::DrawArea);
        }
        if icon_button(ui, Icon::Wall, "Draw walls (5)", app.tool == Tool::DrawWall).clicked() {
            app.run(Cmd::DrawWall);
        }
        if icon_button(ui, Icon::Delete, "Delete selection (Del)", false).clicked() {
            app.run(Cmd::Delete);
        }
        ui.separator();

        // Assign.
        if icon_button(ui, Icon::Fixed, "Assign fixed restraint", false).clicked() {
            app.run(Cmd::FixityFixed);
        }
        if icon_button(ui, Icon::Pinned, "Assign pinned restraint", false).clicked() {
            app.run(Cmd::FixityPinned);
        }
        if icon_button(ui, Icon::Apply, "Assign point load to selection", false).clicked() {
            app.run(Cmd::AssignPointLoad);
        }
        ui.separator();

        // View.
        if icon_button(ui, Icon::ZoomFit, "Zoom fit", false).clicked() {
            app.run(Cmd::ZoomFit);
        }
        if icon_button(ui, Icon::Grid, "Show grid (F7)", app.show_grid).clicked() {
            app.run(Cmd::ToggleGrid);
        }
        if icon_button(ui, Icon::Deformed, "Deformed shape (F6)", app.show_deformed).clicked() {
            app.run(Cmd::ToggleDeformed);
        }
        if icon_button(ui, Icon::Table, "Show tables", app.show_tables).clicked() {
            app.run(Cmd::ShowTables);
        }
        ui.separator();

        // Analyze.
        if icon_button(ui, Icon::Run, "Run analysis (F5)", false).clicked() {
            app.run(Cmd::RunAnalysis);
        }
        ui.separator();

        // Active story.
        let story_text = format!("{}  @ {:.2} m", app.active_story_name(), app.active_elevation());
        egui::ComboBox::from_id_salt("story_combo").width(170.0).selected_text(story_text).show_ui(ui, |ui| {
            let stories: Vec<(usize, String)> =
                app.model.stories.iter().enumerate().map(|(index, story)| (index, format!("{}  @ {:.2} m", story.name, story.elevation))).collect();
            for (index, label) in stories {
                if ui.selectable_label(app.active_story == index, label).clicked() {
                    app.active_story = index;
                    app.work_plane = None;
                    app.draw_frame_start = None;
                    app.area_points.clear();
                    app.draw_wall_start = None;
                    app.pending_fit = Some(());
                    app.status_text = format!("Active story: {}.", app.active_story_name());
                    ui.close();
                }
            }
            ui.separator();
            if ui.button("Story manager…").clicked() {
                app.run(Cmd::DefineStories);
                ui.close();
            }
        });

        // Work plane: active story or any reference plane (horizontal or vertical).
        let work_text = if app.work_plane_active() {
            format!("{}  ({} = {:.2} m)", app.work_plane_name(), app.active_work_plane().axis.label(), app.active_work_plane().offset)
        } else {
            "Work plane: story".to_string()
        };
        egui::ComboBox::from_id_salt("work_plane_combo").width(190.0).selected_text(work_text).show_ui(ui, |ui| {
            if ui.selectable_label(!app.work_plane_active(), "Active story").clicked() {
                app.work_plane = None;
                app.draw_frame_start = None;
                app.area_points.clear();
                app.draw_wall_start = None;
                app.pending_fit = Some(());
                app.status_text = format!("Work plane: {}.", app.work_plane_name());
                ui.close();
            }
            let planes: Vec<(crate::canvas::WorkPlane, String)> = app
                .ref_planes
                .iter()
                .map(|plane| {
                    (crate::canvas::WorkPlane::new(plane.axis, plane.offset), format!("{}  ({} = {:.2} m)", plane.name, plane.axis.label(), plane.offset))
                })
                .collect();
            for (work, label) in planes {
                let selected = app.work_plane.is_some_and(|active| active.axis == work.axis && (active.offset - work.offset).abs() < 1e-9);
                if ui.selectable_label(selected, label).clicked() {
                    app.work_plane = Some(work);
                    app.draw_frame_start = None;
                    app.area_points.clear();
                    app.draw_wall_start = None;
                    app.pending_fit = Some(());
                    app.status_text = format!("Work plane: {}.", app.work_plane_name());
                    ui.close();
                }
            }
            ui.separator();
            if ui.button("Reference planes…").clicked() {
                app.run(Cmd::DefineRefPlanes);
                ui.close();
            }
        });

        if app.tool == Tool::DrawArea {
            ui.separator();
            egui::ComboBox::from_id_salt("area_shape").width(100.0).selected_text(app.area_shape.label()).show_ui(ui, |ui| {
                for shape in AreaShape::ALL {
                    if ui.selectable_label(app.area_shape == shape, shape.label()).clicked() {
                        app.area_shape = shape;
                        app.area_points.clear();
                        app.status_text = format!("Area shape: {}.", shape.label());
                        ui.close();
                    }
                }
            });
            if matches!(app.area_shape, AreaShape::Circle | AreaShape::Ellipse | AreaShape::Arc) {
                ui.add(egui::DragValue::new(&mut app.area_segments).range(8..=128).speed(1.0)).on_hover_text("Curve segments");
            }
        }
        if app.tool == Tool::DrawWall {
            ui.separator();
            ui.label("Height");
            ui.add(egui::DragValue::new(&mut app.wall_height).speed(0.1).range(0.1..=100.0).suffix(" m")).on_hover_text("Wall height above the active story");
        }
        if app.tool == Tool::DrawFrame {
            ui.separator();
            ui.label("Insertion");
            egui::ComboBox::from_id_salt("frame_cardinal")
                .width(120.0)
                .selected_text(app.cardinal_point.label())
                .show_ui(ui, |ui| {
                    for point in CardinalPoint::ALL {
                        if ui.selectable_label(app.cardinal_point == point, point.label()).clicked() {
                            app.cardinal_point = point;
                            app.status_text = format!("New frames use insertion point: {}.", point.label());
                            ui.close();
                        }
                    }
                });
        }

        if app.view_tab == ViewTab::View3D {
            ui.separator();
            for (preset, label) in [(ViewPreset::Iso, "Iso"), (ViewPreset::Front, "Front"), (ViewPreset::Side, "Side"), (ViewPreset::Plan, "Top")] {
                if ui.small_button(label).on_hover_text("3D view preset").clicked() {
                    app.set_view_preset(preset);
                }
            }
        }

            });
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let theme_label = if app.dark { "Light" } else { "Dark" };
            if ui.button(theme_label).on_hover_text("Switch between the dark and light theme").clicked() {
                let cmd = if app.dark { Cmd::ThemeLight } else { Cmd::ThemeDark };
                app.run(cmd);
            }
            egui::ComboBox::from_id_salt("units_toolbar").width(136.0).selected_text(app.units.label()).show_ui(ui, |ui| {
                if ui.selectable_label(app.units == UnitSystem::Metric, Cmd::UnitsMetric.label()).clicked() {
                    app.run(Cmd::UnitsMetric);
                    ui.close();
                }
                if ui.selectable_label(app.units == UnitSystem::Imperial, Cmd::UnitsImperial.label()).clicked() {
                    app.run(Cmd::UnitsImperial);
                    ui.close();
                }
            });
        });
    });
}

pub fn view_tabs(app: &mut OpenStructApp, ui: &mut egui::Ui) {
    let pal = theme::palette(app.dark);
    ui.horizontal_centered(|ui| {
        for (tab, label) in [(ViewTab::Plan, "Plan View"), (ViewTab::View3D, "3D View")] {
            if ui.selectable_label(app.view_tab == tab, label).clicked() {
                app.view_tab = tab;
            }
        }
        ui.separator();
        ui.label(egui::RichText::new(format!("Model: {}", app.model.name)).weak());
        ui.separator();
        let plane = if app.work_plane_active() {
            format!("Work plane: {} ({})", app.work_plane_name(), app.work_plane_description())
        } else {
            format!("Story: {} @ {:.2} m", app.active_story_name(), app.active_elevation())
        };
        ui.label(egui::RichText::new(plane).weak());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if app.dirty {
                ui.label(egui::RichText::new("● modified").color(pal.warn)).on_hover_text("The model has unsaved changes");
            }
            ui.label(egui::RichText::new(format!("Units: {}", app.units.label())).weak());
        });
    });
}
