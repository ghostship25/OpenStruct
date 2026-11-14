use nalgebra::{DMatrix, DVector};

pub struct Solver;

impl Solver {
    pub fn solve_linear(k: &DMatrix<f64>, f: &DVector<f64>, fixed_dofs: &[usize]) -> DVector<f64> {
        let n = k.nrows();
        let mut reduced_k = k.clone();
        let mut reduced_f = f.clone();

        // Remove fixed degrees of freedom (simplified - penalty method style)
        for &dof in fixed_dofs.iter().rev() {
            reduced_k = DMatrix::from_fn(n - 1, n - 1, |i, j| {
                let ii = if i >= dof { i + 1 } else { i };
                let jj = if j >= dof { j + 1 } else { j };
                k[(ii, jj)]
            });
            let mut rf = DVector::zeros(n - 1);
            for i in 0..(n - 1) {
                let ii = if i >= dof { i + 1 } else { i };
                rf[i] = f[ii];
            }
            reduced_f = rf;
        }

        // Solve K*d = F
        let decomp = reduced_k.clone().lu();
        match decomp.solve(&reduced_f) {
            Some(d) => d,
            None => DVector::zeros(reduced_f.len()),
        }
    }
}
