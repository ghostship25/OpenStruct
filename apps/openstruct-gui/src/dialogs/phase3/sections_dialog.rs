use crate::app::{OpenStructApp, SectionKind};
use eframe::egui;

/// Phase 3 frame-section library: parametric shapes, drawn sections and assignment.
pub fn open_frame_sections_dialog(app: &mut OpenStructApp, ctx: &egui::Context) {
    if !app.define_frame_sections_dialog_open {
        return;
    }
    let mut open = app.define_frame_sections_dialog_open;
    let mut close = false;
    let mut apply = false;
    let mut add = false;
    let mut remove: Option<usize> = None;
    let mut assign: Option<usize> = None;
    let mut design: Option<usize> = None;
    let units = app.units;
    let len_label = units.len_label();

    egui::Window::new("Frame Sections…")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(1180.0)
        .show(ctx, |ui| {
            ui.label("Width × depth define the outline; web/wall and flange thickness apply to I, channel, tee, box, pipe and angle shapes. Modifiers scale the stiffness the solver uses (1.0 = unmodified). Drawn sections come from the Section Designer; properties are recomputed on Apply.");
            ui.separator();
            egui::ScrollArea::both().max_height(320.0).id_salt("section_rows").show(ui, |ui| {
                egui::Grid::new("section_grid").striped(true).num_columns(13).show(ui, |ui| {
                    ui.strong("Section");
                    ui.strong("Shape");
                    ui.strong(format!("Width ({len_label})"));
                    ui.strong(format!("Depth ({len_label})"));
                    ui.strong(format!("Web/Wall ({len_label})"));
                    ui.strong(format!("Flange ({len_label})"));
                    ui.strong("A mod");
                    ui.strong("Iy mod");
                    ui.strong("Iz mod");
                    ui.strong("J mod");
                    ui.strong("");
                    ui.strong("");
                    ui.strong("");
                    ui.end_row();
                    for (index, section) in app.section_draft.iter_mut().enumerate() {
                        let kind = section.kind;
                        ui.add(egui::TextEdit::singleline(&mut section.name).desired_width(140.0));
                        egui::ComboBox::from_id_salt(("section_kind", index))
                            .selected_text(kind.label())
                            .show_ui(ui, |ui| {
                                for option in SectionKind::ALL {
                                    ui.selectable_value(&mut section.kind, option, option.label());
                                }
                            });
                        let mut width = units.length_to_display(section.width);
                        if ui
                            .add_enabled(
                                kind.uses_width(),
                                egui::DragValue::new(&mut width).speed(0.5).range(0.0..=100_000.0),
                            )
                            .changed()
                        {
                            section.width = units.length_from_display(width);
                        }
                        let mut depth = units.length_to_display(section.depth);
                        if ui
                            .add_enabled(
                                kind != SectionKind::Custom,
                                egui::DragValue::new(&mut depth).speed(0.5).range(0.0..=100_000.0),
                            )
                            .changed()
                        {
                            section.depth = units.length_from_display(depth);
                            if matches!(kind, SectionKind::Circular | SectionKind::Pipe) {
                                section.width = section.depth;
                            }
                        }
                        let mut web = units.length_to_display(section.web);
                        if ui
                            .add_enabled(
                                kind.uses_wall(),
                                egui::DragValue::new(&mut web).speed(0.1).range(0.0..=10_000.0),
                            )
                            .changed()
                        {
                            section.web = units.length_from_display(web);
                        }
                        let mut flange = units.length_to_display(section.flange);
                        if ui
                            .add_enabled(
                                kind.uses_flange(),
                                egui::DragValue::new(&mut flange).speed(0.1).range(0.0..=10_000.0),
                            )
                            .changed()
                        {
                            section.flange = units.length_from_display(flange);
                        }
                        for (label, value) in [
                            ("A", &mut section.modifiers.area),
                            ("Iy", &mut section.modifiers.iy),
                            ("Iz", &mut section.modifiers.iz),
                            ("J", &mut section.modifiers.j),
                        ] {
                            ui.add(
                                egui::DragValue::new(value)
                                    .speed(0.01)
                                    .range(0.0..=10.0)
                                    .prefix(format!("{label} "))
                                    .max_decimals(3),
                            );
                        }
                        let drawn = !section.designer.is_empty();
                        if ui
                            .button(if drawn { "Edit" } else { "Draw" })
                            .on_hover_text(if drawn {
                                "Edit the drawn parts of this section in the Section Designer"
                            } else {
                                "Draw this section in the Section Designer"
                            })
                            .clicked()
                        {
                            design = Some(index);
                        }
                        let saved = section.id != 0;
                        if ui
                            .add_enabled(saved, egui::Button::new("→ frames"))
                            .on_hover_text("Assign this section to the selected frames")
                            .clicked()
                        {
                            assign = Some(section.id);
                        }
                        if ui.small_button("✕").on_hover_text("Delete this section").clicked() {
                            remove = Some(index);
                        }
                        ui.end_row();
                    }
                });
            });
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Add section:");
                egui::ComboBox::from_id_salt("section_new_kind")
                    .selected_text(app.section_new_kind.label())
                    .show_ui(ui, |ui| {
                        for option in SectionKind::ALL {
                            ui.selectable_value(&mut app.section_new_kind, option, option.label());
                        }
                    });
                if ui.button("Add").clicked() {
                    add = true;
                }
                if ui.button("Design new…").clicked() {
                    app.open_section_designer(None);
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Apply").clicked() {
                    apply = true;
                }
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
            ui.label(
                egui::RichText::new("Circular and pipe shapes use depth as the diameter. Modifiers scale stiffness only; reported section properties stay geometric. Sections in use by a frame cannot be deleted.")
                    .weak(),
            );
        });

    if let Some(index) = remove {
        app.section_draft.remove(index);
    }
    if add {
        app.add_section_draft();
    }
    if let Some(index) = design {
        app.open_section_designer(Some(index));
    }
    if let Some(id) = assign {
        app.assign_section(id);
    }
    if close {
        open = false;
    }
    if apply {
        if app.apply_sections() {
            open = false;
        }
    }
    app.define_frame_sections_dialog_open = open;
}
