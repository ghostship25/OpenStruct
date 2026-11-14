#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! OpenStruct — standard structural analysis desktop application.
//!
//! Phase 1 shell: command registry, menus, icon toolbars, view tabs, Model Explorer,
//! Properties of Object, Tables dock, status bar, command palette, undo/redo and
//! `.ostrut` save/load around the Phase 0 drawing + linear static analysis core.

mod app;
mod canvas;
mod commands;
mod dialogs;
#[cfg(test)]
mod draw_mode_tests;
mod explorer;
mod material_library;
#[cfg(test)]
mod new_model_tests;
mod icons;
mod menu;
mod properties;
mod statusbar;
mod tables;
mod tables_view;
mod theme;
mod toolbar;
mod wgpu3d;

use app::OpenStructApp;
use commands::Cmd;
use std::path::PathBuf;

const BENCHMARK_FRAMES: u32 = 190;
const BENCHMARK_WARMUP: u32 = 10;

struct BenchmarkApp {
    app: OpenStructApp,
    frames: u32,
    measured_ms: f64,
    report_path: PathBuf,
}

impl BenchmarkApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut app = OpenStructApp::new(cc);
        app.run(Cmd::BenchmarkModel);
        app.show_explorer = false;
        app.show_properties = false;
        app.show_tables = false;
        let report_path = std::env::var_os("OPENSTRUCT_BENCHMARK_REPORT")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("openstruct-benchmark.txt"));
        Self { app, frames: 0, measured_ms: 0.0, report_path }
    }
}

impl eframe::App for BenchmarkApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::App::save(&mut self.app, storage);
    }

    fn ui(&mut self, ui: &mut eframe::egui::Ui, frame: &mut eframe::Frame) {
        eframe::App::ui(&mut self.app, ui, frame);
        self.frames += 1;
        if self.frames > BENCHMARK_WARMUP {
            self.measured_ms += ui.input(|input| input.stable_dt as f64 * 1000.0);
        }
        if self.frames >= BENCHMARK_FRAMES {
            let (pick_hits, pick_attempts, pick_ms) = self.app.benchmark_picking(128);
            let measured_frames = (self.frames - BENCHMARK_WARMUP).max(1) as f64;
            let mean_ms = self.measured_ms / measured_frames;
            let report = format!(
                "OpenStruct 5,000-joint 3D benchmark\n\
                 model: {} joints, {} frames, {} stories\n\
                 frames: {} total, {} measured after {} warm-up\n\
                 mean frame time: {:.3} ms\n\
                 effective frame rate: {:.2} fps\n\
                 picking: {}/{} probes hit in {:.3} ms\n\
                 renderer: painter 3D, wgpu installed: {}\n",
                self.app.model.nodes.len(),
                self.app.model.elements.len(),
                self.app.model.stories.len(),
                self.frames,
                measured_frames,
                BENCHMARK_WARMUP,
                mean_ms,
                if mean_ms > 0.0 { 1000.0 / mean_ms } else { 0.0 },
                pick_hits,
                pick_attempts,
                pick_ms,
                self.app.wgpu_ready,
            );
            if let Some(parent) = self.report_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Err(error) = std::fs::write(&self.report_path, &report) {
                eprintln!("could not write benchmark report: {error}");
            }
            print!("{report}");
            ui.ctx().send_viewport_cmd(eframe::egui::ViewportCommand::Close);
        } else {
            ui.ctx().request_repaint();
        }
    }
}

fn main() -> eframe::Result {
    env_logger::init();
    let benchmark = std::env::args().any(|argument| argument == "--benchmark");
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 920.0])
            .with_min_inner_size([980.0, 620.0])
            .with_title("OpenStruct — Structural Analysis"),
        // Never restore a saved window position: an off-screen or moved-down
        // position made the window unreachable after a restart.
        centered: true,
        persist_window: false,
        ..Default::default()
    };
    eframe::run_native(
        "OpenStruct",
        options,
        Box::new(move |cc| {
            if benchmark {
                Ok(Box::new(BenchmarkApp::new(cc)))
            } else {
                Ok(Box::new(OpenStructApp::new(cc)))
            }
        }),
    )
}
