//! Headless driving of the application shell.
//!
//! These tests build the real `OpenStructApp`, run its real `draw` pass inside an
//! `egui::Context` with synthesised pointer and keyboard input, and check the two
//! things the drawing tools must never break:
//!
//! - the plan canvas keeps its screen rectangle when a draw tool adds controls to
//!   the toolbar (the "window moved below" symptom), and
//! - the rest of the shell (menus, dialogs that a menu item opens) keeps working
//!   while a frame or area chain is active.

use crate::app::{OpenStructApp, Tool};
use crate::commands::Cmd;
use eframe::egui;

const WIDTH: f32 = 1440.0;
const HEIGHT: f32 = 920.0;

/// Runs the real shell one frame at a time with scripted input.
struct Harness {
    ctx: egui::Context,
    app: OpenStructApp,
    time: f64,
}

impl Harness {
    fn new() -> Self {
        Self {
            ctx: egui::Context::default(),
            app: OpenStructApp::fresh(),
            time: 0.0,
        }
    }

    fn frame(&mut self, events: Vec<egui::Event>) {
        self.time += 1.0 / 60.0;
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(WIDTH, HEIGHT))),
            time: Some(self.time),
            events,
            ..Default::default()
        };
        let ctx = self.ctx.clone();
        let app = &mut self.app;
        // Nothing uploads textures in a headless run, so acknowledge them.
        let mut output = ctx.run_ui(raw, |ui| app.draw(ui));
        output.textures_delta.clear();
    }

    /// Run one quiet frame so the layout and the canvas rectangle settle.
    fn settle(&mut self) {
        self.frame(Vec::new());
        self.frame(Vec::new());
    }

    /// Press and release the primary button at `pos`, one frame per step.
    fn click(&mut self, pos: egui::Pos2) {
        self.frame(vec![egui::Event::PointerMoved(pos)]);
        self.frame(vec![egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        }]);
        self.frame(vec![egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }]);
    }

    fn canvas(&self) -> egui::Rect {
        self.app
            .last_canvas_rect
            .expect("the plan canvas is drawn on every frame")
    }
}

#[test]
fn switching_to_the_frame_tool_does_not_move_the_canvas() {
    let mut harness = Harness::new();
    harness.settle();
    let before = harness.canvas();

    harness.app.run(Cmd::DrawFrame);
    harness.settle();
    let after = harness.canvas();

    assert!(matches!(harness.app.tool, Tool::DrawFrame));
    assert_eq!(
        before.min, after.min,
        "the canvas moved when the draw-frame controls appeared in the toolbar"
    );
    assert_eq!(before.size(), after.size(), "the canvas changed size with the draw tool active");
}

#[test]
fn every_draw_tool_keeps_the_same_canvas() {
    let mut harness = Harness::new();
    harness.settle();
    let reference = harness.canvas();

    for cmd in [Cmd::DrawJoint, Cmd::DrawFrame, Cmd::DrawArea, Cmd::DrawWall, Cmd::ToolSelect] {
        harness.app.run(cmd);
        harness.settle();
        assert_eq!(
            harness.canvas().min,
            reference.min,
            "selecting a draw tool moved the canvas"
        );
    }
}

#[test]
fn canvas_clicks_draw_a_frame_chain() {
    let mut harness = Harness::new();
    harness.settle();

    harness.app.run(Cmd::DrawFrame);
    harness.settle();

    let canvas = harness.canvas();
    let start = canvas.center() - egui::vec2(80.0, 0.0);
    let end = canvas.center() + egui::vec2(80.0, 0.0);
    let frames_before = harness.app.model.elements.len();

    harness.click(start);
    assert!(
        harness.app.draw_frame_start.is_some(),
        "the first canvas click must start a frame chain"
    );

    harness.click(end);
    assert!(
        harness.app.model.elements.len() > frames_before,
        "the second canvas click must create a frame"
    );
    assert!(
        harness.app.draw_frame_start.is_some(),
        "a normal (not quick) draw keeps the chain open on the new joint"
    );
}

#[test]
fn a_dialog_opened_mid_chain_keeps_the_chain_and_the_layout() {
    let mut harness = Harness::new();
    harness.settle();

    harness.app.run(Cmd::DrawFrame);
    harness.settle();
    let canvas = harness.canvas();
    harness.click(canvas.center());
    let chain = harness.app.draw_frame_start;
    assert!(chain.is_some(), "the chain must be active before the dialog opens");

    // The same call the Define/Design menu items make.
    harness.app.run(Cmd::DefineStories);
    harness.settle();

    assert!(harness.app.story_dialog_open, "the dialog must open while a chain is active");
    assert_eq!(harness.app.draw_frame_start, chain, "opening a dialog dropped the frame chain");
    assert_eq!(
        harness.canvas(),
        canvas,
        "a floating dialog moved the canvas; status {:?} tables {:?} status_text {:?}",
        harness.app.last_status_rect,
        harness.app.last_tables_rect,
        harness.app.status_text
    );
}

#[test]
fn escape_cancels_the_chain_without_touching_the_model() {
    let mut harness = Harness::new();
    harness.settle();

    harness.app.run(Cmd::DrawFrame);
    harness.settle();
    harness.click(harness.canvas().center());
    assert!(harness.app.draw_frame_start.is_some());
    let frames = harness.app.model.elements.len();

    harness.frame(vec![egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]);

    assert!(harness.app.draw_frame_start.is_none(), "Escape must cancel the chain");
    assert_eq!(harness.app.model.elements.len(), frames, "cancelling must not edit the model");
}

#[test]
fn the_menu_bar_still_takes_input_while_drawing() {
    let mut harness = Harness::new();
    harness.settle();

    harness.app.run(Cmd::DrawFrame);
    harness.settle();

    // Click the menu strip above the canvas. The canvas must not swallow it, so
    // no joint or frame may be created by a click that lands in the menu.
    let canvas = harness.canvas();
    let menu_point = egui::pos2(canvas.center().x, canvas.min.y - 40.0);
    assert!(menu_point.y > 0.0, "the menu strip must be on screen");
    let nodes_before = harness.app.model.nodes.len();
    let frames_before = harness.app.model.elements.len();

    harness.click(menu_point);

    assert_eq!(harness.app.model.nodes.len(), nodes_before, "the menu click created a joint");
    assert_eq!(harness.app.model.elements.len(), frames_before, "the menu click created a frame");
    assert!(
        harness.app.draw_frame_start.is_none(),
        "the menu click started a canvas chain"
    );
}
