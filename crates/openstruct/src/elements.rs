use serde::{Deserialize, Serialize};

/// Where the member axis sits on its section: centre, an edge midpoint or a
/// corner. Offsets are expressed as fractions of the section width/depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CardinalPoint {
    #[default]
    Center,
    TopCenter,
    BottomCenter,
    LeftCenter,
    RightCenter,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl CardinalPoint {
    pub const ALL: [CardinalPoint; 9] = [
        CardinalPoint::Center,
        CardinalPoint::TopCenter,
        CardinalPoint::BottomCenter,
        CardinalPoint::LeftCenter,
        CardinalPoint::RightCenter,
        CardinalPoint::TopLeft,
        CardinalPoint::TopRight,
        CardinalPoint::BottomLeft,
        CardinalPoint::BottomRight,
    ];

    pub fn label(self) -> &'static str {
        match self {
            CardinalPoint::Center => "Centre",
            CardinalPoint::TopCenter => "Top centre",
            CardinalPoint::BottomCenter => "Bottom centre",
            CardinalPoint::LeftCenter => "Left centre",
            CardinalPoint::RightCenter => "Right centre",
            CardinalPoint::TopLeft => "Top left",
            CardinalPoint::TopRight => "Top right",
            CardinalPoint::BottomLeft => "Bottom left",
            CardinalPoint::BottomRight => "Bottom right",
        }
    }

    /// Signed (lateral, vertical) position on the section in -1..=1.
    pub fn offsets(self) -> (f64, f64) {
        let lateral = match self {
            CardinalPoint::LeftCenter | CardinalPoint::TopLeft | CardinalPoint::BottomLeft => -1.0,
            CardinalPoint::RightCenter | CardinalPoint::TopRight | CardinalPoint::BottomRight => 1.0,
            _ => 0.0,
        };
        let vertical = match self {
            CardinalPoint::TopCenter | CardinalPoint::TopLeft | CardinalPoint::TopRight => 1.0,
            CardinalPoint::BottomCenter | CardinalPoint::BottomLeft | CardinalPoint::BottomRight => -1.0,
            _ => 0.0,
        };
        (lateral, vertical)
    }
}

/// World-space offset from the member centreline to its insertion point.
/// Lateral offsets follow the right-hand side of the member in plan; the
/// vertical offset follows global Z. Both are half the section dimension.
pub fn cardinal_offset(cardinal: CardinalPoint, width: f64, depth: f64, pi: [f64; 3], pj: [f64; 3]) -> [f64; 3] {
    let (lateral, vertical) = cardinal.offsets();
    if lateral == 0.0 && vertical == 0.0 {
        return [0.0; 3];
    }
    let dx = pj[0] - pi[0];
    let dy = pj[1] - pi[1];
    let horizontal = (dx * dx + dy * dy).sqrt();
    let (right_x, right_y) = if horizontal > 1e-9 {
        (dy / horizontal, -dx / horizontal)
    } else {
        (1.0, 0.0)
    };
    [
        right_x * lateral * width / 2.0,
        right_y * lateral * width / 2.0,
        vertical * depth / 2.0,
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ElementType {
    Frame, // Beam/column element
    Truss, // Axial only
    Shell, // Plate/shell element
    Link,  // Spring/damper element
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Element {
    pub id: usize,
    pub element_type: ElementType,
    pub node_i: usize,
    pub node_j: usize,
    pub material_id: usize,
    pub section_id: usize,
    pub releases: [bool; 12], // end releases
    /// Partial fixity on a released dof, as the fraction (0..=1) of the member's
    /// own rotational stiffness retained by the connection spring: 0 is a pure
    /// release, 1 means rigid. Ignored unless the dof is released.
    #[serde(default)]
    pub partial_fixity: [f64; 12],
    #[serde(default)]
    pub cardinal_point: CardinalPoint,
}

impl Element {
    pub fn frame(
        id: usize,
        node_i: usize,
        node_j: usize,
        material_id: usize,
        section_id: usize,
    ) -> Self {
        Self {
            id,
            element_type: ElementType::Frame,
            node_i,
            node_j,
            material_id,
            section_id,
            releases: [false; 12],
            partial_fixity: [0.0; 12],
            cardinal_point: CardinalPoint::Center,
        }
    }

    pub fn truss(
        id: usize,
        node_i: usize,
        node_j: usize,
        material_id: usize,
        section_id: usize,
    ) -> Self {
        Self {
            id,
            element_type: ElementType::Truss,
            node_i,
            node_j,
            material_id,
            section_id,
            // Truss: axial force only — release all bending/torsion dofs at both ends.
            releases: [
                false, false, false, true, true, true,                false, false, false, true, true, true,
            ],
            partial_fixity: [0.0; 12],
            cardinal_point: CardinalPoint::Center,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cardinal_points_report_signed_section_offsets() {
        assert_eq!(CardinalPoint::Center.offsets(), (0.0, 0.0));
        assert_eq!(CardinalPoint::TopCenter.offsets(), (0.0, 1.0));
        assert_eq!(CardinalPoint::BottomLeft.offsets(), (-1.0, -1.0));
        assert_eq!(CardinalPoint::RightCenter.offsets(), (1.0, 0.0));
        assert_eq!(CardinalPoint::TopRight.label(), "Top right");
        assert_eq!(CardinalPoint::default(), CardinalPoint::Center);
    }

    #[test]
    fn cardinal_offsets_follow_member_orientation_and_section_size() {
        let x = cardinal_offset(CardinalPoint::TopRight, 0.3, 0.5, [0.0, 0.0, 0.0], [4.0, 0.0, 0.0]);
        assert!((x[0] - 0.0).abs() < 1e-12);
        assert!((x[1] + 0.15).abs() < 1e-12);
        assert!((x[2] - 0.25).abs() < 1e-12);

        let y = cardinal_offset(CardinalPoint::LeftCenter, 0.4, 0.6, [0.0, 0.0, 0.0], [0.0, 4.0, 0.0]);
        assert!((y[0] - -0.2).abs() < 1e-12);
        assert!((y[1] - 0.0).abs() < 1e-12);
        assert!((y[2] - 0.0).abs() < 1e-12);

        assert_eq!(cardinal_offset(CardinalPoint::Center, 0.3, 0.5, [0.0; 3], [4.0, 0.0, 0.0]), [0.0; 3]);
    }
}
