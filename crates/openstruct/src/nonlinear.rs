//! Nonlinear Analysis.
//!
//! Implements nonlinear structural analysis:
//! - Newton-Raphson iteration for geometric nonlinearity (P-Delta)
//! - Material nonlinearity (bilinear stress-strain)
//! - Pushover analysis (capacity spectrum)
//! - Convergence criteria (force/displacement/energy)
//!
//! Reference: Bathe, K.J. (2006). "Finite Element Procedures." Prentice Hall.

use nalgebra::{DMatrix, DVector};

/// Nonlinear analysis parameters
#[derive(Debug, Clone)]
pub struct NonlinearParams {
    /// Maximum number of iterations per load step
    pub max_iterations: usize,
    pub force_tol: f64,
    pub disp_tol: f64,
    pub energy_tol: f64,
    pub num_load_steps: usize,
    pub initial_load_factor: f64,
}

impl Default for NonlinearParams {
    fn default() -> Self {
        Self {
            max_iterations: 50,
            force_tol: 1e-3,
            disp_tol: 1e-3,
            energy_tol: 1e-3,
            num_load_steps: 10,
            initial_load_factor: 0.1,
        }
    }
}

/// Convergence status
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConvergenceStatus {
    Converged,
    NotConverged,
    Diverged,
    MaxIterationsReached,
}

/// Result of a single Newton-Raphson iteration
#[derive(Debug, Clone)]
pub struct IterationResult {
    pub iteration: usize,
    pub residual_norm: f64,
    pub displacement_norm: f64,
    pub energy_norm: f64,
    pub status: ConvergenceStatus,
}

/// Nonlinear solver using Newton-Raphson
pub struct NonlinearSolver {
    pub params: NonlinearParams,
}

impl NonlinearSolver {
    pub fn new(params: NonlinearParams) -> Self {
        Self { params }
    }

    /// Newton-Raphson solve for a single load step
    /// K(u) * du = R - F(u)
    /// where K is tangent stiffness, R is applied load, F is internal force
    pub fn newton_raphson_step(
        &self,
        stiffness_fn: &dyn Fn(&DVector<f64>) -> DMatrix<f64>,
        internal_force_fn: &dyn Fn(&DVector<f64>) -> DVector<f64>,
        applied_load: &DVector<f64>,
        u_prev: &DVector<f64>,
    ) -> (DVector<f64>, Vec<IterationResult>, ConvergenceStatus) {
        let mut u = u_prev.clone();
        let mut iterations = Vec::new();
        let mut status = ConvergenceStatus::NotConverged;

        for iter in 0..self.params.max_iterations {
            // Compute tangent stiffness at current u
            let k_tangent = stiffness_fn(&u);

            // Compute internal force at current u
            let f_int = internal_force_fn(&u);

            // Residual: R - F_int
            let residual = applied_load - &f_int;
            let residual_norm = residual.norm();

            // Displacement increment
            let du = match k_tangent.lu().solve(&residual) {
                Some(du) => du,
                None => {
                    status = ConvergenceStatus::Diverged;
                    break;
                }
            };

            // Update displacement
            u += &du;
            let disp_norm = du.norm();
            let energy_norm = residual.dot(&du).abs();

            let iter_result = IterationResult {
                iteration: iter + 1,
                residual_norm,
                displacement_norm: disp_norm,
                energy_norm,
                status: ConvergenceStatus::NotConverged,
            };
            iterations.push(iter_result);

            // Check convergence
            if residual_norm < self.params.force_tol
                && disp_norm < self.params.disp_tol
                && energy_norm < self.params.energy_tol
            {
                status = ConvergenceStatus::Converged;
                iterations.last_mut().unwrap().status = ConvergenceStatus::Converged;
                break;
            }
        }

        if status == ConvergenceStatus::NotConverged && !iterations.is_empty() {
            if iterations.last().unwrap().iteration >= self.params.max_iterations {
                status = ConvergenceStatus::MaxIterationsReached;
            }
        }

        (u, iterations, status)
    }

    /// Full nonlinear analysis with load stepping
    pub fn solve(
        &self,
        stiffness_fn: &dyn Fn(&DVector<f64>) -> DMatrix<f64>,
        internal_force_fn: &dyn Fn(&DVector<f64>) -> DVector<f64>,
        total_load: &DVector<f64>,
    ) -> (Vec<DVector<f64>>, Vec<Vec<IterationResult>>, ConvergenceStatus) {
        let mut u = DVector::zeros(total_load.len());
        let mut load_factor = 0.0;
        let load_increment = 1.0 / self.params.num_load_steps as f64;

        let mut displacement_history = Vec::new();
        let mut iteration_history = Vec::new();
        let mut overall_status = ConvergenceStatus::Converged;

        for step in 0..self.params.num_load_steps {
            load_factor += load_increment;
            let applied_load = total_load * load_factor;

            let (u_new, iters, status) = self.newton_raphson_step(
                stiffness_fn,
                internal_force_fn,
                &applied_load,
                &u,
            );

            u = u_new;
            displacement_history.push(u.clone());
            iteration_history.push(iters);

            if status != ConvergenceStatus::Converged {
                overall_status = status;
                break;
            }
        }

        (displacement_history, iteration_history, overall_status)
    }
}

/// Bilinear material model (elastic-perfectly plastic)
#[derive(Debug, Clone)]
pub struct BilinearMaterial {
    pub e: f64,           // elastic modulus
    pub fy: f64,          // yield stress
    pub e_hardening: f64, // hardening modulus (0 for perfectly plastic)
}

impl BilinearMaterial {
    pub fn new(e: f64, fy: f64, e_hardening: f64) -> Self {
        Self { e, fy, e_hardening }
    }

    /// Stress given strain
    pub fn stress(&self, strain: f64) -> f64 {
        let ey = self.fy / self.e;
        if strain.abs() <= ey {
            self.e * strain
        } else {
            let sign = if strain > 0.0 { 1.0 } else { -1.0 };
            self.fy * sign + self.e_hardening * (strain - ey * sign)
        }
    }

    /// Tangent modulus given strain
    pub fn tangent_modulus(&self, strain: f64) -> f64 {
        let ey = self.fy / self.e;
        if strain.abs() <= ey {
            self.e
        } else {
            self.e_hardening
        }
    }
}

/// P-Delta geometric stiffness for a column
/// K_g = (P/L) * [matrix] for buckling
pub fn geometric_stiffness_column(p: f64, l: f64) -> DMatrix<f64> {
    // Simplified geometric stiffness matrix for a 2-node frame element
    // (6 DOFs per node = 12 total)
    let mut kg = DMatrix::zeros(12, 12);
    let coeff = p / (30.0 * l);
    // Lateral displacement terms (v and w)
    // kg[1,1] = kg[7,7] = 36*coeff
    // kg[1,7] = kg[7,1] = -36*coeff
    // etc.
    kg[(1, 1)] = 36.0 * coeff;
    kg[(7, 7)] = 36.0 * coeff;
    kg[(1, 7)] = -36.0 * coeff;
    kg[(7, 1)] = -36.0 * coeff;

    kg[(2, 2)] = 36.0 * coeff;
    kg[(8, 8)] = 36.0 * coeff;
    kg[(2, 8)] = -36.0 * coeff;
    kg[(8, 2)] = -36.0 * coeff;

    kg[(5, 5)] = 4.0 * l * l * coeff;
    kg[(11, 11)] = 4.0 * l * l * coeff;
    kg[(5, 11)] = -4.0 * l * l * coeff;
    kg[(11, 5)] = -4.0 * l * l * coeff;

    kg[(4, 4)] = 4.0 * l * l * coeff;
    kg[(10, 10)] = 4.0 * l * l * coeff;
    kg[(4, 10)] = -4.0 * l * l * coeff;
    kg[(10, 4)] = -4.0 * l * l * coeff;

    kg
}

/// Pushover analysis result
#[derive(Debug, Clone)]
pub struct PushoverResult {
    /// Base shear at each step
    pub base_shear: Vec<f64>,
    pub roof_displacement: Vec<f64>,
    pub capacity_curve: Vec<(f64, f64)>,
}

/// Simple pushover analysis
pub fn pushover_analysis(
    num_steps: usize,
    load_increment: f64,
    stiffness: f64,
    yield_displacement: f64,
    ultimate_displacement: f64,
) -> PushoverResult {
    let mut base_shear = Vec::new();
    let mut roof_disp = Vec::new();
    let mut capacity = Vec::new();

    let mut disp = 0.0;
    for step in 0..num_steps {
        disp += load_increment;
        // Bilinear response: elastic then plastic
        let shear = if disp <= yield_displacement {
            stiffness * disp
        } else if disp <= ultimate_displacement {
            stiffness * yield_displacement // plateau
        } else {
            0.0 // collapse
        };
        base_shear.push(shear);
        roof_disp.push(disp);
        capacity.push((disp, shear));
    }

    PushoverResult {
        base_shear,
        roof_displacement: roof_disp,
        capacity_curve: capacity,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nonlinear_params_default() {
        let params = NonlinearParams::default();
        assert_eq!(params.max_iterations, 50);
        assert!((params.force_tol - 1e-3).abs() < 1e-9);
    }

    #[test]
    fn test_bilinear_material_elastic() {
        let mat = BilinearMaterial::new(200000.0, 350.0, 0.0);
        let stress = mat.stress(0.001);
        assert!((stress - 200.0).abs() < 1e-9);
    }

    #[test]
    fn test_bilinear_material_yield() {
        let mat = BilinearMaterial::new(200000.0, 350.0, 0.0);
        let stress = mat.stress(0.002);
        // Should yield at 350
        assert!((stress - 350.0).abs() < 1e-9);
    }

    #[test]
    fn test_bilinear_tangent_modulus() {
        let mat = BilinearMaterial::new(200000.0, 350.0, 1000.0);
        let et = mat.tangent_modulus(0.001);
        assert!((et - 200000.0).abs() < 1e-9);
        let et2 = mat.tangent_modulus(0.002);
        assert!((et2 - 1000.0).abs() < 1e-9);
    }

    #[test]
    fn test_newton_raphson_linear() {
        // Linear problem: K*u = F, K=10, F=100 => u=10
        let solver = NonlinearSolver::new(NonlinearParams::default());
        let k = 10.0;
        let stiffness_fn = |_: &DVector<f64>| DMatrix::from_element(1, 1, k);
        let internal_fn = |u: &DVector<f64>| DVector::from_vec(vec![k * u[0]]);
        let load = DVector::from_vec(vec![100.0]);
        let u_prev = DVector::from_vec(vec![0.0]);
        let (u, iters, status) = solver.newton_raphson_step(
            &stiffness_fn,
            &internal_fn,
            &load,
            &u_prev,
        );
        assert!((u[0] - 10.0).abs() < 1e-6);
        assert_eq!(status, ConvergenceStatus::Converged);
    }

    #[test]
    fn test_pushover_analysis() {
        let result = pushover_analysis(10, 0.01, 1000.0, 0.05, 0.15);
        assert_eq!(result.base_shear.len(), 10);
        assert_eq!(result.roof_displacement.len(), 10);
        assert!(result.base_shear[0] > 0.0);
    }

    #[test]
    fn test_geometric_stiffness() {
        let kg = geometric_stiffness_column(100.0, 10.0);
        assert_eq!(kg.nrows(), 12);
        assert_eq!(kg.ncols(), 12);
    }

    #[test]
    fn test_nonlinear_solve() {
        let solver = NonlinearSolver::new(NonlinearParams {
            num_load_steps: 5,
            ..Default::default()
        });
        let k = 10.0;
        let stiffness_fn = |_: &DVector<f64>| DMatrix::from_element(1, 1, k);
        let internal_fn = |u: &DVector<f64>| DVector::from_vec(vec![k * u[0]]);
        let load = DVector::from_vec(vec![100.0]);
        let (history, iters, status) = solver.solve(&stiffness_fn, &internal_fn, &load);
        assert_eq!(history.len(), 5);
        assert_eq!(status, ConvergenceStatus::Converged);
        // Final displacement should be ~10
        let last = history.last().unwrap();
        assert!((last[0] - 10.0).abs() < 1e-4);
    }
}
