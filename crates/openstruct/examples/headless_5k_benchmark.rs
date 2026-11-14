//! Headless 5,000-joint projection/picking benchmark.
//!
//! This exercises the same camera projection and 12 px joint-picking rule as
//! the GUI, without creating an eframe/wgpu window. It is a CPU baseline, not a
//! GPU frame-time measurement.
//!
//! Run with:
//!   cargo run --release --example headless_5k_benchmark
//!
//! Optional environment variables:
//!   OPENSTRUCT_HEADLESS_REPORT          Write the report to this path.
//!   OPENSTRUCT_HEADLESS_MAX_FRAME_MS    Fail if the mean projection time exceeds it.
//!   OPENSTRUCT_HEADLESS_MIN_HIT_RATE    Fail if the unambiguous picking rate is below it.

use openstruct::Model;
use std::fmt::Write as _;

const WIDTH: f64 = 1280.0;
const HEIGHT: f64 = 720.0;
const HIT_RADIUS: f64 = 12.0;
const WARMUP_FRAMES: usize = 10;
const FRAMES: usize = 190;
const PROBES: usize = 128;

/// Reject a candidate when another joint is within this distance of being
/// equally close to the 1 px probe. The GUI's nearest-joint rule cannot
/// distinguish such overlapping projections.
const PROBE_MARGIN_PX: f64 = 1.0;

#[derive(Clone, Copy)]
struct Camera {
    eye: [f64; 3],
    right: [f64; 3],
    up: [f64; 3],
    forward: [f64; 3],
    focal: f64,
}

impl Camera {
    fn fit(model: &Model) -> Self {
        let mut min = [f64::INFINITY; 3];
        let mut max = [f64::NEG_INFINITY; 3];
        for node in model.nodes.values() {
            let point = [node.x, node.y, node.z];
            for axis in 0..3 {
                min[axis] = min[axis].min(point[axis]);
                max[axis] = max[axis].max(point[axis]);
            }
        }
        let target = [
            (min[0] + max[0]) * 0.5,
            (min[1] + max[1]) * 0.5,
            (min[2] + max[2]) * 0.5,
        ];
        let extent = (max[0] - min[0])
            .max(max[1] - min[1])
            .max(max[2] - min[2])
            .max(4.0);

        // The GUI's default ISO camera.
        let yaw = -1.0_f64;
        let pitch = 0.55_f64;
        let distance = (extent * 1.9).clamp(4.0, 5000.0);
        let cp = pitch.cos();
        let sp = pitch.sin();
        let cy = yaw.cos();
        let sy = yaw.sin();
        let eye = [
            target[0] + distance * cp * cy,
            target[1] + distance * cp * sy,
            target[2] + distance * sp,
        ];
        let forward = normalize([target[0] - eye[0], target[1] - eye[1], target[2] - eye[2]]);
        let world_up = if forward[2].abs() > 0.99 {
            [0.0, 1.0, 0.0]
        } else {
            [0.0, 0.0, 1.0]
        };
        let right = normalize(cross(forward, world_up));
        let up = cross(right, forward);
        Self {
            eye,
            right,
            up,
            forward,
            focal: HEIGHT * 1.15,
        }
    }

    fn project(&self, point: [f64; 3]) -> Option<[f64; 2]> {
        let rel = [
            point[0] - self.eye[0],
            point[1] - self.eye[1],
            point[2] - self.eye[2],
        ];
        let cz = dot(rel, self.forward);
        if cz <= 0.05 {
            return None;
        }
        let cx = dot(rel, self.right);
        let cy = dot(rel, self.up);
        Some([
            WIDTH * 0.5 + cx / cz * self.focal,
            HEIGHT * 0.5 - cy / cz * self.focal,
        ])
    }
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalize(v: [f64; 3]) -> [f64; 3] {
    let length = dot(v, v).sqrt();
    if length <= f64::EPSILON {
        [0.0, 0.0, 0.0]
    } else {
        [v[0] / length, v[1] / length, v[2] / length]
    }
}

fn project_into(
    camera: &Camera,
    nodes: &[&openstruct::model::Node],
    out: &mut Vec<(usize, [f64; 2])>,
) {
    out.clear();
    for node in nodes {
        if let Some(point) = camera.project([node.x, node.y, node.z]) {
            out.push((node.id, point));
        }
    }
}

/// Return the nearest and second-nearest joint for a screen-space click.
fn nearest_two(screen: &[(usize, [f64; 2])], target: [f64; 2]) -> Option<(usize, f64, f64)> {
    let mut best: Option<(usize, f64)> = None;
    let mut second = f64::INFINITY;
    for &(id, point) in screen {
        let dx = point[0] - target[0];
        let dy = point[1] - target[1];
        let distance_sq = dx * dx + dy * dy;
        if distance_sq >= HIT_RADIUS * HIT_RADIUS {
            continue;
        }
        match best {
            None => best = Some((id, distance_sq)),
            Some((_, best_sq)) if distance_sq < best_sq => {
                second = best_sq;
                best = Some((id, distance_sq));
            }
            Some((_, best_sq)) => second = second.min(distance_sq.max(best_sq)),
        }
    }
    best.map(|(id, best_sq)| (id, best_sq, second))
}

fn pick_hit(screen: &[(usize, [f64; 2])], expected: usize, base: [f64; 2]) -> bool {
    let target = [base[0] + 1.0, base[1] + 1.0];
    nearest_two(screen, target).is_some_and(|(id, _, _)| id == expected)
}

fn main() {
    let model = Model::benchmark_building();
    let camera = Camera::fit(&model);
    let mut nodes: Vec<_> = model.nodes.values().collect();
    nodes.sort_by_key(|node| node.id);

    let mut initial_screen = Vec::with_capacity(nodes.len());
    project_into(&camera, &nodes, &mut initial_screen);
    let raw_probes: Vec<_> = nodes
        .iter()
        .take(PROBES)
        .map(|node| (node.id, [node.x, node.y, node.z]))
        .collect();

    // Raw samples preserve the GUI's HashMap-order behaviour, including
    // projections that overlap within a pixel. The measured picking set uses
    // only candidates whose expected joint is the unique nearest hit.
    let mut raw_hits = 0usize;
    let mut raw_attempts = 0usize;
    for &(expected, world) in &raw_probes {
        let Some(base) = camera.project(world) else {
            continue;
        };
        raw_attempts += 1;
        if pick_hit(&initial_screen, expected, base) {
            raw_hits += 1;
        }
    }

    let mut probes = Vec::with_capacity(PROBES);
    let mut overlap_rejected = 0usize;
    for node in &nodes {
        if probes.len() == PROBES {
            break;
        }
        let Some(base) = camera.project([node.x, node.y, node.z]) else {
            continue;
        };
        if !(0.0..WIDTH).contains(&base[0]) || !(0.0..HEIGHT).contains(&base[1]) {
            continue;
        }
        let target = [base[0] + 1.0, base[1] + 1.0];
        let Some((best_id, best_sq, second_sq)) = nearest_two(&initial_screen, target) else {
            overlap_rejected += 1;
            continue;
        };
        let margin = second_sq.sqrt() - best_sq.sqrt();
        if best_id != node.id || margin < PROBE_MARGIN_PX {
            overlap_rejected += 1;
            continue;
        }
        probes.push((node.id, [node.x, node.y, node.z]));
    }
    if probes.is_empty() {
        eprintln!("no unambiguous picking probes were available");
        std::process::exit(1);
    }

    let mut screen = Vec::with_capacity(nodes.len());
    let mut warmup_projection = 0usize;
    for _ in 0..WARMUP_FRAMES {
        project_into(&camera, &nodes, &mut screen);
        warmup_projection += std::hint::black_box(screen.len());
    }

    let mut frame_times = Vec::with_capacity(FRAMES);
    let mut pick_times = Vec::with_capacity(FRAMES);
    let mut hits = 0usize;
    let mut attempts = 0usize;
    let mut projected = 0usize;

    for _ in 0..FRAMES {
        let frame_started = std::time::Instant::now();
        project_into(&camera, &nodes, &mut screen);
        projected = std::hint::black_box(screen.len());
        frame_times.push(frame_started.elapsed().as_secs_f64() * 1000.0);

        let pick_started = std::time::Instant::now();
        for &(expected, world) in &probes {
            let Some(base) = camera.project(world) else {
                continue;
            };
            attempts += 1;
            if pick_hit(&screen, expected, base) {
                hits += 1;
            }
        }
        pick_times.push(pick_started.elapsed().as_secs_f64() * 1000.0);
    }

    let mean = |values: &[f64]| values.iter().sum::<f64>() / values.len().max(1) as f64;
    let frame_mean = mean(&frame_times);
    let pick_mean = mean(&pick_times);
    let fps = if frame_mean > 0.0 {
        1000.0 / frame_mean
    } else {
        f64::INFINITY
    };
    let hit_rate = 100.0 * hits as f64 / attempts.max(1) as f64;
    let raw_hit_rate = 100.0 * raw_hits as f64 / raw_attempts.max(1) as f64;

    let mut report = String::new();
    let _ = writeln!(report, "OpenStruct headless 5,000-joint benchmark");
    let _ = writeln!(
        report,
        "kind: CPU projection + joint picking (no GPU/window)"
    );
    let _ = writeln!(report, "joints: {}", nodes.len());
    let _ = writeln!(report, "frames: {FRAMES} measured, {WARMUP_FRAMES} warm-up");
    let _ = writeln!(report, "viewport: {WIDTH:.0}x{HEIGHT:.0}");
    let _ = writeln!(
        report,
        "frame_scope: CPU projection of all joints; excludes GUI paint/GPU"
    );
    let _ = writeln!(report, "mean_frame_ms: {frame_mean:.4}");
    let _ = writeln!(report, "effective_fps: {fps:.2}");
    let _ = writeln!(
        report,
        "min_frame_ms: {:.4}",
        frame_times.iter().copied().fold(f64::INFINITY, f64::min)
    );
    let _ = writeln!(
        report,
        "max_frame_ms: {:.4}",
        frame_times
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max)
    );
    let _ = writeln!(report, "picking_probes: {attempts}");
    let _ = writeln!(report, "picking_hits: {hits}");
    let _ = writeln!(report, "picking_hit_rate: {hit_rate:.2}%");
    let _ = writeln!(report, "picking_mean_frame_ms: {pick_mean:.4}");
    let _ = writeln!(
        report,
        "picking_per_probe_ms: {:.4}",
        pick_mean / probes.len() as f64
    );
    let _ = writeln!(report, "unambiguous_probe_count: {}", probes.len());
    let _ = writeln!(report, "overlap_rejected: {overlap_rejected}");
    let _ = writeln!(report, "raw_sample_probes: {raw_attempts}");
    let _ = writeln!(report, "raw_sample_hits: {raw_hits}");
    let _ = writeln!(report, "raw_sample_hit_rate: {raw_hit_rate:.2}%");
    let _ = writeln!(report, "warmup_projection_samples: {warmup_projection}");
    let _ = writeln!(report, "projected_per_frame: {projected}");

    if let Ok(path) = std::env::var("OPENSTRUCT_HEADLESS_REPORT") {
        if let Err(error) = std::fs::write(&path, &report) {
            eprintln!("could not write {path}: {error}");
            std::process::exit(1);
        }
    }
    print!("{report}");

    if let Ok(limit) = std::env::var("OPENSTRUCT_HEADLESS_MAX_FRAME_MS") {
        match limit.parse::<f64>() {
            Ok(limit) if frame_mean <= limit => {}
            Ok(limit) => {
                eprintln!("mean frame time {frame_mean:.4} ms exceeds limit {limit:.4} ms");
                std::process::exit(1);
            }
            Err(error) => {
                eprintln!("invalid OPENSTRUCT_HEADLESS_MAX_FRAME_MS: {error}");
                std::process::exit(1);
            }
        }
    }
    if let Ok(limit) = std::env::var("OPENSTRUCT_HEADLESS_MIN_HIT_RATE") {
        match limit.parse::<f64>() {
            Ok(limit) if hit_rate >= limit => {}
            Ok(limit) => {
                eprintln!("picking hit rate {hit_rate:.2}% is below limit {limit:.2}%");
                std::process::exit(1);
            }
            Err(error) => {
                eprintln!("invalid OPENSTRUCT_HEADLESS_MIN_HIT_RATE: {error}");
                std::process::exit(1);
            }
        }
    }
}
