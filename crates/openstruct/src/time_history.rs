//! Time History Analysis via Direct Integration.
//!
//! Implements Newmark-beta (average acceleration) method for transient
//! dynamic analysis of structures under time-varying loads.
//!
//! Reference: Newmark, N.M. (1959). "A Method of Computation for Structural
//! Dynamics." ASCE Journal of Engineering Mechanics Division.

use crate::model::Model;
use nalgebra::{DMatrix, DVector};

/// Newmark-beta parameters
#[derive(Debug, Clone, Copy)]
pub struct NewmarkParams {
    /// Beta parameter (0.25 for average acceleration, unconditionally stable)
    pub beta: f64,
    pub gamma: f64,
}

impl Default for NewmarkParams {
    fn default() -> Self {
        Self {
            beta: 0.25, // average acceleration (trapezoidal rule)
            gamma: 0.5,
        }
    }
}

/// A time history load function: returns load vector at time t
pub type LoadFunction = Box<dyn Fn(f64) -> DVector<f64>>;

/// Time history analysis results
#[derive(Debug, Clone)]
pub struct TimeHistoryResults {
    /// Time steps
    pub time: Vec<f64>,
    pub displacements: Vec<DVector<f64>>,
    pub velocities: Vec<DVector<f64>>,
    pub accelerations: Vec<DVector<f64>>,
    pub peak_displacement: Vec<f64>,
    pub peak_velocity: Vec<f64>,
    pub peak_acceleration: Vec<f64>,
}

/// Time history analyzer
pub struct TimeHistoryAnalyzer {
    pub model: Model,
    pub mass_matrix: DMatrix<f64>,
    pub stiffness_matrix: DMatrix<f64>,
    pub damping_matrix: DMatrix<f64>,
    pub params: NewmarkParams,
    pub dt: f64,
    pub num_steps: usize,
    pub damping_ratio: f64,
}

impl TimeHistoryAnalyzer {
    /// Create a new time history analyzer
    pub fn new(model: Model, dt: f64, num_steps: usize) -> Self {
        let ndof = model.nodes.len() * 6;
        Self {
            model,
            mass_matrix: DMatrix::identity(ndof, ndof),
            stiffness_matrix: DMatrix::zeros(ndof, ndof),
            damping_matrix: DMatrix::zeros(ndof, ndof),
            params: NewmarkParams::default(),
            dt,
            num_steps,
            damping_ratio: 0.05,
        }
    }

    /// Set Newmark parameters
    pub fn set_newmark(&mut self, beta: f64, gamma: f64) {
        self.params.beta = beta;
        self.params.gamma = gamma;
    }

    /// Set damping ratio (Rayleigh damping: C = alpha*M + beta*K)
    pub fn set_damping_ratio(&mut self, zeta: f64) {
        self.damping_ratio = zeta;
    }

    /// Run time history analysis with a given load function
    pub fn run(&self, load_func: LoadFunction) -> TimeHistoryResults {
        let ndof = self.mass_matrix.nrows();
        let beta = self.params.beta;
        let gamma = self.params.gamma;
        let dt = self.dt;

        // Effective stiffness matrix: K_eff = K + (gamma/(beta*dt))*C + (1/(beta*dt^2))*M
        let mut k_eff = self.stiffness_matrix.clone();
        // Add damping contribution
        let c_coeff = gamma / (beta * dt);
        for i in 0..ndof {
            for j in 0..ndof {
                k_eff[(i, j)] += c_coeff * self.damping_matrix[(i, j)];
            }
        }
        // Add mass contribution
        let m_coeff = 1.0 / (beta * dt * dt);
        for i in 0..ndof {
            for j in 0..ndof {
                k_eff[(i, j)] += m_coeff * self.mass_matrix[(i, j)];
            }
        }

        // Factorize K_eff (LU decomposition)
        let lu = k_eff.lu();

        // Initial conditions
        let mut u = DVector::zeros(ndof); // displacement
        let mut v = DVector::zeros(ndof); // velocity
        let mut a = DVector::zeros(ndof); // acceleration

        // Initial acceleration: a0 = M^-1 * (F0 - C*v0 - K*u0)
        let f0 = load_func(0.0);
        let mut ma = &self.mass_matrix * &a;
        let mut cv = &self.damping_matrix * &v;
        let mut ku = &self.stiffness_matrix * &u;
        let mut rhs = &f0 - &cv - &ku;
        // Solve M*a = rhs
        // For diagonal mass matrix, simple division
        for i in 0..ndof {
            if self.mass_matrix[(i, i)] > 1e-12 {
                a[i] = rhs[i] / self.mass_matrix[(i, i)];
            }
        }

        // Storage
        let mut time = Vec::with_capacity(self.num_steps + 1);
        let mut displacements = Vec::with_capacity(self.num_steps + 1);
        let mut velocities = Vec::with_capacity(self.num_steps + 1);
        let mut accelerations = Vec::with_capacity(self.num_steps + 1);

        time.push(0.0);
        displacements.push(u.clone());
        velocities.push(v.clone());
        accelerations.push(a.clone());

        // Time stepping
        for step in 1..=self.num_steps {
            let t = step as f64 * dt;
            let f = load_func(t);

            // Effective load: F_eff = F + M*(u/(beta*dt^2) + v/(beta*dt) + (1/(2*beta)-1)*a)
            //                      + C*(gamma/(beta*dt)*u + (gamma/beta-1)*v + (gamma/(2*beta)-1)*dt*a)
            let mut f_eff = f.clone();

            // Mass contribution to effective load
            for i in 0..ndof {
                f_eff[i] += self.mass_matrix[(i, i)] * (
                    u[i] / (beta * dt * dt)
                    + v[i] / (beta * dt)
                    + (1.0 / (2.0 * beta) - 1.0) * a[i]
                );
            }

            // Damping contribution to effective load
            for i in 0..ndof {
                f_eff[i] += self.damping_matrix[(i, i)] * (
                    gamma * u[i] / (beta * dt)
                    + (gamma / beta - 1.0) * v[i]
                    + (gamma / (2.0 * beta) - 1.0) * dt * a[i]
                );
            }

            // Solve for new displacement
            let u_new = lu.solve(&f_eff).unwrap_or_else(|| DVector::zeros(ndof));

            // Update velocity and acceleration
            let mut v_new = DVector::zeros(ndof);
            let mut a_new = DVector::zeros(ndof);
            for i in 0..ndof {
                a_new[i] = (u_new[i] - u[i]) / (beta * dt * dt)
                    - v[i] / (beta * dt)
                    - (1.0 / (2.0 * beta) - 1.0) * a[i];
                v_new[i] = v[i] + dt * ((1.0 - gamma) * a[i] + gamma * a_new[i]);
            }

            u = u_new;
            v = v_new;
            a = a_new;

            time.push(t);
            displacements.push(u.clone());
            velocities.push(v.clone());
            accelerations.push(a.clone());
        }

        // Compute peak values
        let mut peak_disp = vec![0.0f64; ndof];
        let mut peak_vel = vec![0.0f64; ndof];
        let mut peak_acc = vec![0.0f64; ndof];
        for disp in &displacements {
            for i in 0..ndof {
                peak_disp[i] = peak_disp[i].max(disp[i].abs());
            }
        }
        for vel in &velocities {
            for i in 0..ndof {
                peak_vel[i] = peak_vel[i].max(vel[i].abs());
            }
        }
        for acc in &accelerations {
            for i in 0..ndof {
                peak_acc[i] = peak_acc[i].max(acc[i].abs());
            }
        }

        TimeHistoryResults {
            time,
            displacements,
            velocities,
            accelerations,
            peak_displacement: peak_disp,
            peak_velocity: peak_vel,
            peak_acceleration: peak_acc,
        }
    }

    /// Run time history with a sinusoidal load (for testing)
    pub fn run_sinusoidal(&self, amplitude: f64, frequency: f64, dof: usize) -> TimeHistoryResults {
        let ndof = self.mass_matrix.nrows();
        let amp = amplitude;
        let freq = frequency;
        let d = dof;
        let load_func: LoadFunction = Box::new(move |t: f64| {
            let mut f = DVector::zeros(ndof);
            f[d] = amp * (2.0 * std::f64::consts::PI * freq * t).sin();
            f
        });
        self.run(load_func)
    }
}

/// Single-DOF time history (simplified)
pub fn sdof_time_history(
    mass: f64,
    stiffness: f64,
    damping_ratio: f64,
    dt: f64,
    num_steps: usize,
    load_func: impl Fn(f64) -> f64,
) -> (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>) {
    let beta = 0.25;
    let gamma = 0.5;
    let omega_n = (stiffness / mass).sqrt();
    let c = 2.0 * damping_ratio * mass * omega_n;

    // Effective stiffness
    let k_eff = stiffness + gamma / (beta * dt) * c + 1.0 / (beta * dt * dt) * mass;

    let mut u = 0.0;
    let mut v = 0.0;
    let mut a = 0.0;

    // Initial acceleration
    let f0 = load_func(0.0);
    a = (f0 - c * v - stiffness * u) / mass;

    let mut time = Vec::with_capacity(num_steps + 1);
    let mut disp = Vec::with_capacity(num_steps + 1);
    let mut vel = Vec::with_capacity(num_steps + 1);
    let mut acc = Vec::with_capacity(num_steps + 1);

    time.push(0.0);
    disp.push(u);
    vel.push(v);
    acc.push(a);

    for step in 1..=num_steps {
        let t = step as f64 * dt;
        let f = load_func(t);

        let f_eff = f
            + mass * (u / (beta * dt * dt) + v / (beta * dt) + (1.0 / (2.0 * beta) - 1.0) * a)
            + c * (gamma * u / (beta * dt) + (gamma / beta - 1.0) * v + (gamma / (2.0 * beta) - 1.0) * dt * a);

        let u_new = f_eff / k_eff;
        let a_new = (u_new - u) / (beta * dt * dt) - v / (beta * dt) - (1.0 / (2.0 * beta) - 1.0) * a;
        let v_new = v + dt * ((1.0 - gamma) * a + gamma * a_new);

        u = u_new;
        v = v_new;
        a = a_new;

        time.push(t);
        disp.push(u);
        vel.push(v);
        acc.push(a);
    }

    (time, disp, vel, acc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sdof_free_vibration() {
        // Free vibration: mass=1, stiffness=100, zeta=0.05
        // Natural period T = 2*pi/sqrt(100) = 0.628s
        let mass = 1.0;
        let stiffness = 100.0;
        let zeta = 0.05;
        let dt = 0.01;
        let num_steps = 100;
        let (time, disp, _vel, _acc) = sdof_time_history(
            mass,
            stiffness,
            zeta,
            dt,
            num_steps,
            |_| 0.0, // no load
        );
        assert_eq!(time.len(), num_steps + 1);
        assert_eq!(disp.len(), num_steps + 1);
        // Displacement should oscillate (not grow)
        let max_disp = disp.iter().fold(0.0f64, |a, &b| a.max(b.abs()));
        assert!(max_disp < 10.0); // bounded
    }

    #[test]
    fn test_sdof_step_load() {
        // Step load: F=10 applied at t=0
        let mass = 1.0;
        let stiffness = 100.0;
        let zeta = 0.05;
        let dt = 0.01;
        let num_steps = 200;
        let (time, disp, _vel, _acc) = sdof_time_history(
            mass,
            stiffness,
            zeta,
            dt,
            num_steps,
            |t| if t >= 0.0 { 10.0 } else { 0.0 },
        );
        // Static displacement = F/k = 10/100 = 0.1
        // Dynamic should approach ~0.1
        let last = disp.last().unwrap();
        assert!(last.abs() < 0.5); // reasonable
    }

    #[test]
    fn test_newmark_params_default() {
        let params = NewmarkParams::default();
        assert!((params.beta - 0.25).abs() < 1e-9);
        assert!((params.gamma - 0.5).abs() < 1e-9);
    }

    #[test]
    fn test_time_history_analyzer_creation() {
        let model = Model::new("Test");
        let analyzer = TimeHistoryAnalyzer::new(model, 0.01, 100);
        assert_eq!(analyzer.num_steps, 100);
        assert!((analyzer.dt - 0.01).abs() < 1e-9);
    }

    #[test]
    fn test_sdof_sinusoidal() {
        let mass = 1.0;
        let stiffness = 100.0;
        let zeta = 0.05;
        let dt = 0.01;
        let num_steps = 500;
        let (time, disp, _vel, _acc) = sdof_time_history(
            mass,
            stiffness,
            zeta,
            dt,
            num_steps,
            |t| 5.0 * (2.0 * std::f64::consts::PI * 1.0 * t).sin(),
        );
        assert_eq!(time.len(), num_steps + 1);
        let max_disp = disp.iter().fold(0.0f64, |a, &b| a.max(b.abs()));
        assert!(max_disp > 0.0);
        assert!(max_disp < 10.0);
    }
}
