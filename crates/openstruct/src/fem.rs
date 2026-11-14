use nalgebra::{DMatrix, DVector};

pub fn local_stiffness_frame(
    e: f64,
    a: f64,
    iz: f64,
    iy: f64,
    g: f64,
    j: f64,
    l: f64,
) -> DMatrix<f64> {
    // 12x12 local stiffness matrix for 3D frame element
    // P-Delta / geometric stiffness terms (will be added to global K as K_geom)
    let mut k = DMatrix::<f64>::zeros(12, 12);

    // Axial terms
    k[(0, 0)] = e * a / l;
    k[(0, 6)] = -e * a / l;
    k[(6, 0)] = -e * a / l;
    k[(6, 6)] = e * a / l;

    // Torsion
    k[(3, 3)] = g * j / l;
    k[(3, 9)] = -g * j / l;
    k[(9, 3)] = -g * j / l;
    k[(9, 9)] = g * j / l;

    // Bending about z (strong axis)
    k[(1, 1)] = 12.0 * e * iz / l.powi(3);
    k[(1, 5)] = 6.0 * e * iz / l.powi(2);
    k[(1, 7)] = -12.0 * e * iz / l.powi(3);
    k[(1, 11)] = 6.0 * e * iz / l.powi(2);

    k[(5, 1)] = 6.0 * e * iz / l.powi(2);
    k[(5, 5)] = 4.0 * e * iz / l;
    k[(5, 7)] = -6.0 * e * iz / l.powi(2);
    k[(5, 11)] = 2.0 * e * iz / l;

    k[(7, 1)] = -12.0 * e * iz / l.powi(3);
    k[(7, 5)] = -6.0 * e * iz / l.powi(2);
    k[(7, 7)] = 12.0 * e * iz / l.powi(3);
    k[(7, 11)] = -6.0 * e * iz / l.powi(2);

    k[(11, 1)] = 6.0 * e * iz / l.powi(2);
    k[(11, 5)] = 2.0 * e * iz / l;
    k[(11, 7)] = -6.0 * e * iz / l.powi(2);
    k[(11, 11)] = 4.0 * e * iz / l;

    // Bending about y
    k[(2, 2)] = 12.0 * e * iy / l.powi(3);
    k[(2, 4)] = -6.0 * e * iy / l.powi(2);
    k[(2, 8)] = -12.0 * e * iy / l.powi(3);
    k[(2, 10)] = -6.0 * e * iy / l.powi(2);

    k[(4, 2)] = -6.0 * e * iy / l.powi(2);
    k[(4, 4)] = 4.0 * e * iy / l;
    k[(4, 8)] = 6.0 * e * iy / l.powi(2);
    k[(4, 10)] = 2.0 * e * iy / l;

    k[(8, 2)] = -12.0 * e * iy / l.powi(3);
    k[(8, 4)] = 6.0 * e * iy / l.powi(2);
    k[(8, 8)] = 12.0 * e * iy / l.powi(3);
    k[(8, 10)] = 6.0 * e * iy / l.powi(2);

    k[(10, 2)] = -6.0 * e * iy / l.powi(2);
    k[(10, 4)] = 2.0 * e * iy / l;
    k[(10, 8)] = 6.0 * e * iy / l.powi(2);
    k[(10, 10)] = 4.0 * e * iy / l;

    k
}

/// A member whose released end dofs are statically condensed onto the retained
/// dofs, with an optional partial-fixity spring on each released dof.
///
/// `fixity[d]` is the fraction (0..=1) of that dof's own member stiffness retained
/// by the connection spring: 0 is a pure release (pin), 0.5 keeps half the
/// stiffness, and 1 means the dof is not released at all (rigid).
pub struct ReleasedMember {
    /// Condensed 12x12 local stiffness, with zero rows and columns on the
    /// released dofs.
    pub stiffness: DMatrix<f64>,
    released: Vec<usize>,
    kept: Vec<usize>,
    correction: DMatrix<f64>,
}

impl ReleasedMember {
    pub fn condense(stiffness: &DMatrix<f64>, releases: &[bool; 12], fixity: &[f64; 12]) -> Self {
        let released: Vec<usize> = (0..12).filter(|&dof| releases[dof] && fixity[dof] < 1.0).collect();
        let kept: Vec<usize> = (0..12).filter(|&dof| !released.contains(&dof)).collect();
        if released.is_empty() {
            return Self { stiffness: stiffness.clone(), released, kept, correction: DMatrix::zeros(0, 0) };
        }
        let mut augmented = stiffness.clone();
        for &dof in &released {
            // A spring of k·f/(1−f) in series with the member end keeps a fraction
            // f of the rigid rotational stiffness (f = 0.5 → same as the member).
            let fraction = fixity[dof].clamp(0.0, 0.999);
            augmented[(dof, dof)] += fraction / (1.0 - fraction) * stiffness[(dof, dof)].abs();
        }
        let mut krr = DMatrix::<f64>::zeros(released.len(), released.len());
        let mut kkr = DMatrix::<f64>::zeros(kept.len(), released.len());
        for (a, &ia) in kept.iter().enumerate() {
            for (b, &ib) in released.iter().enumerate() {
                kkr[(a, b)] = augmented[(ia, ib)];
            }
        }
        for (a, &ia) in released.iter().enumerate() {
            for (b, &ib) in released.iter().enumerate() {
                krr[(a, b)] = augmented[(ia, ib)];
            }
        }
        let inverse = krr
            .clone()
            .svd(true, true)
            .pseudo_inverse(1e-12)
            .unwrap_or_else(|_| DMatrix::zeros(released.len(), released.len()));
        let correction = &kkr * inverse;
        let mut condensed = DMatrix::<f64>::zeros(12, 12);
        for (a, &ia) in kept.iter().enumerate() {
            for (b, &ib) in kept.iter().enumerate() {
                let mut value = augmented[(ia, ib)];
                for (c, _) in released.iter().enumerate() {
                    value -= correction[(a, c)] * kkr[(b, c)];
                }
                condensed[(ia, ib)] = value;
            }
        }
        Self { stiffness: condensed, released, kept, correction }
    }

    /// Condense a local load vector the same way, so a released member does not
    /// carry fixed-end forces its releases cannot transfer.
    pub fn condense_load(&self, load: &DVector<f64>) -> DVector<f64> {
        let mut condensed = load.clone();
        if self.released.is_empty() {
            return condensed;
        }
        for (a, &ia) in self.kept.iter().enumerate() {
            let mut value = load[ia];
            for (b, &ib) in self.released.iter().enumerate() {
                value -= self.correction[(a, b)] * load[ib];
            }
            condensed[ia] = value;
        }
        for &dof in &self.released {
            condensed[dof] = 0.0;
        }
        condensed
    }
}

/// Rotation matrix R (3x3) mapping *global* vectors to *local* element axes.
/// Rows: local x, y, z expressed in global coordinates.
/// Local x points from node i to node j (given as the unit vector `(cx, cy, cz)`).
pub fn frame_rotation(cx: f64, cy: f64, cz: f64) -> DMatrix<f64> {
    let c = nalgebra::Vector3::new(cx, cy, cz);
    // Reference "up" vector: global Z unless the member is (nearly) vertical.
    let up = if cz.abs() > 0.999 {
        nalgebra::Vector3::new(0.0, 1.0, 0.0)
    } else {
        nalgebra::Vector3::new(0.0, 0.0, 1.0)
    };
    let local_y = up.cross(&c).normalize();
    let local_z = c.cross(&local_y).normalize();

    // Rows are the local axes expressed in global coordinates, so R * global
    // gives local components. Storing the axes as columns would transpose the
    // rotation and put a vertical column's axial load into its local Y.
    let mut r = DMatrix::<f64>::zeros(3, 3);
    for (row, v) in [c, local_y, local_z].iter().enumerate() {
        r[(row, 0)] = v.x;
        r[(row, 1)] = v.y;
        r[(row, 2)] = v.z;
    }
    r
}

/// Transform local dofs to global: T = blockdiag(R, R, R, R) for 6 dof pairs per node.
pub fn transformation(r: &DMatrix<f64>) -> DMatrix<f64> {
    let mut t = DMatrix::<f64>::zeros(12, 12);
    for node in 0..4u8 {
        let o = (node as usize) * 3;
        for row in 0..3 {
            for col in 0..3 {
                t[(o + row, o + col)] = r[(row, col)];
            }
        }
    }
    t
}

/// Transformation from the two global node DOF triplets to the local end DOFs
/// of a member whose ends are rigidly offset from the nodes by `oi` and `oj`.
///
/// The translation at an offset point is `u + theta x offset`; rotations are
/// unchanged. The returned 12x12 matrix is used exactly like [`transformation`]
/// in the assembly (K_global = T^T k_local T).
pub fn offset_transformation(r: &DMatrix<f64>, oi: [f64; 3], oj: [f64; 3]) -> DMatrix<f64> {
    let mut t = DMatrix::<f64>::zeros(12, 12);
    for (end, offset) in [oi, oj].iter().enumerate() {
        let base = end * 6;
        // skew(offset): theta x offset = -skew(offset) * theta
        let skew = [
            [0.0, -offset[2], offset[1]],
            [offset[2], 0.0, -offset[0]],
            [-offset[1], offset[0], 0.0],
        ];
        for row in 0..3 {
            for col in 0..3 {
                t[(base + row, base + col)] = r[(row, col)];
                t[(base + 3 + row, base + 3 + col)] = r[(row, col)];
                let mut coupling = 0.0;
                for k in 0..3 {
                    coupling += r[(row, k)] * skew[k][col];
                }
                t[(base + row, base + 3 + col)] = -coupling;
            }
        }
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_offset_reduces_to_the_plain_transformation() {
        let r = frame_rotation(1.0, 0.0, 0.0);
        let plain = transformation(&r);
        let offset = offset_transformation(&r, [0.0; 3], [0.0; 3]);
        for row in 0..12 {
            for col in 0..12 {
                assert!((plain[(row, col)] - offset[(row, col)]).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn vertical_offset_couples_rotation_to_translation() {
        let identity = DMatrix::<f64>::identity(3, 3);
        let t = offset_transformation(&identity, [0.0, 0.0, 0.25], [0.0, 0.0, 0.25]);
        // A rotation about Y at node i moves the offset point in +X by 0.25.
        assert!((t[(0, 4)] - 0.25).abs() < 1e-12);
        assert!((t[(6, 10)] - 0.25).abs() < 1e-12);
        // The rotation itself is unchanged.
        assert!((t[(4, 4)] - 1.0).abs() < 1e-12);
    }
}

pub fn element_length(x1: f64, y1: f64, z1: f64, x2: f64, y2: f64, z2: f64) -> f64 {
    ((x2 - x1).powi(2) + (y2 - y1).powi(2) + (z2 - z1).powi(2)).sqrt()
}

/// Hermite cubic shape functions N1..N4 at parameter `s` in [0, 1]
/// (dimensionless; N2/N4 must be multiplied by the member length).
fn hermite(s: f64) -> [f64; 4] {
    let s2 = s * s;
    let s3 = s2 * s;
    [
        1.0 - 3.0 * s2 + 2.0 * s3,
        s - 2.0 * s2 + s3,
        3.0 * s2 - 2.0 * s3,
        -s2 + s3,
    ]
}

/// Sampled deflected shape of an element, including an amplification factor.
///
/// `d_local` must be the element displacement vector in *local* dofs (12),
/// Returns `samples + 1` points in global coordinates (already amplified).
pub fn deflected_points(
    p_i: [f64; 3],
    p_j: [f64; 3],
    d_local: &DVector<f64>,
    rot: &DMatrix<f64>,
    samples: usize,
    amplification: f64,
) -> Vec<[f64; 3]> {
    let l = element_length(p_i[0], p_i[1], p_i[2], p_j[0], p_j[1], p_j[2]);
    let dir = [
        (p_j[0] - p_i[0]) / l,
        (p_j[1] - p_i[1]) / l,
        (p_j[2] - p_i[2]) / l,
    ];
    let yl = [rot[(0, 1)], rot[(1, 1)], rot[(2, 1)]];
    let zl = [rot[(0, 2)], rot[(1, 2)], rot[(2, 2)]];

    let mut points = Vec::with_capacity(samples + 1);
    for k in 0..=samples {
        let s = k as f64 / samples as f64;
        let n = hermite(s);
        // Axial: linear interpolation between end axial displacements.
        let off_x = (n[0] * d_local[0] + n[2] * d_local[6]) * amplification;
        // Bending in the x-y plane (couples with theta-z, positive convention).
        let v =
            n[0] * d_local[1] + n[1] * l * d_local[5] + n[2] * d_local[7] + n[3] * l * d_local[11];
        // Bending in the x-z plane (couples with theta-y, negative convention).
        let w =
            n[0] * d_local[2] - n[1] * l * d_local[4] + n[2] * d_local[8] - n[3] * l * d_local[10];
        let off_y = v * amplification;
        let off_z = w * amplification;

        let t = s * l;
        let base = [
            p_i[0] + dir[0] * t,
            p_i[1] + dir[1] * t,
            p_i[2] + dir[2] * t,
        ];
        let p = [
            base[0] + dir[0] * off_x + yl[0] * off_y + zl[0] * off_z,
            base[1] + dir[1] * off_x + yl[1] * off_y + zl[1] * off_z,
            base[2] + dir[2] * off_x + yl[2] * off_y + zl[2] * off_z,
        ];
        points.push(p);
    }
    points
}

/// Consistent nodal load vector (12 local dofs) for a linearly varying
/// distributed load over a member of length `l`.
///
/// `q_start` / `q_end` are the load intensities in the member's *local* axes
/// (x axial, y and z transverse) at the two ends, in force per unit length.
/// The transverse terms follow the same sign convention as
/// [`local_stiffness_frame`]: v bends about z, w bends about y.
pub fn local_distributed_load(q_start: [f64; 3], q_end: [f64; 3], l: f64) -> DVector<f64> {
    let mut f = DVector::<f64>::zeros(12);
    if l <= 0.0 {
        return f;
    }
    for axis in 0..3 {
        let (q1, q2) = (q_start[axis], q_end[axis]);
        let (f1, f2) = match axis {
            0 => (0, 6),
            1 => (1, 7),
            _ => (2, 8),
        };
        f[f1] = l * (2.0 * q1 + q2) / 6.0;
        f[f2] = l * (q1 + 2.0 * q2) / 6.0;
    }
    // Bending in the local x-y plane (about z).
    let (qy1, qy2) = (q_start[1], q_end[1]);
    f[5] = l * l * (3.0 * qy1 + 2.0 * qy2) / 60.0;
    f[11] = -l * l * (2.0 * qy1 + 3.0 * qy2) / 60.0;
    // Bending in the local x-z plane (about y, mirrored rotation convention).
    let (qz1, qz2) = (q_start[2], q_end[2]);
    f[4] = -l * l * (3.0 * qz1 + 2.0 * qz2) / 60.0;
    f[10] = l * l * (2.0 * qz1 + 3.0 * qz2) / 60.0;
    f
}

#[cfg(test)]
mod distributed_load_tests {
    use super::*;

    #[test]
    fn uniform_load_is_split_evenly_and_balances() {
        let l = 8.0;
        let q = -10_000.0;
        let f = local_distributed_load([0.0, 0.0, q], [0.0, 0.0, q], l);
        // Half the total load at each end.
        assert!((f[2] - q * l / 2.0).abs() < 1e-9);
        assert!((f[8] - q * l / 2.0).abs() < 1e-9);
        // Force balance: the two ends carry the whole load.
        assert!((f[2] + f[8] - q * l).abs() < 1e-9);
        // The fixed-end moments are equal and opposite, qL^2/12.
        assert!((f[4] + q * l * l / 12.0).abs() < 1e-9);
        assert!((f[10] - q * l * l / 12.0).abs() < 1e-9);
        assert!((f[4] + f[10]).abs() < 1e-9);
    }

    #[test]
    fn axial_load_has_no_bending_terms() {
        let f = local_distributed_load([1000.0, 0.0, 0.0], [1000.0, 0.0, 0.0], 2.0);
        assert!((f[0] - 1000.0).abs() < 1e-9);
        assert!((f[6] - 1000.0).abs() < 1e-9);
        for dof in [4, 5, 10, 11] {
            assert!(f[dof].abs() < 1e-12);
        }
    }

    #[test]
    fn triangular_load_matches_closed_form_end_moments() {
        let l = 6.0;
        let q = 3_000.0;
        let f = local_distributed_load([0.0, 0.0, 0.0], [0.0, 0.0, q], l);
        // Local z, so the moments appear on the my dofs.
        assert!((f[4] + q * l * l / 30.0).abs() < 1e-9);
        assert!((f[10] - q * l * l / 20.0).abs() < 1e-9);
    }
}
