//! Field expression data types and public guarantee contract.

/// How closely a field behaves like an exact signed distance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DistanceGuarantee {
    /// The scalar value equals the signed distance to this primitive boundary.
    Exact,
    /// The field is 1-Lipschitz, so its absolute value is a conservative distance lower bound.
    LowerBound,
    /// No signed-distance bound is promised.
    None,
}

/// The radius at the start and end of each segment of a tube whose radius changes linearly with distance along the
/// polyline, from `radius` at the first point to `end_radius` at the last.
pub(crate) fn tube_segment_radii(
    points: &[[f64; 3]],
    radius: f64,
    end_radius: f64,
) -> Vec<[f64; 2]> {
    let lengths: Vec<f64> = points
        .windows(2)
        .map(|pair| {
            let d = [
                pair[1][0] - pair[0][0],
                pair[1][1] - pair[0][1],
                pair[1][2] - pair[0][2],
            ];
            (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
        })
        .collect();
    let total: f64 = lengths.iter().sum();
    let mut travelled = 0.0;
    lengths
        .iter()
        .map(|length| {
            let at = |distance: f64| radius + (end_radius - radius) * (distance / total);
            let segment = [at(travelled), at(travelled + length)];
            travelled += length;
            segment
        })
        .collect()
}

/// A validated expression node in a spatial-field graph.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum FieldExpr {
    Sphere {
        center: [f64; 3],
        radius: f64,
    },
    Ellipsoid {
        radii: [f64; 3],
    },
    Box {
        half: [f64; 3],
        rounding: f64,
    },
    Torus {
        major: f64,
        tube: f64,
    },
    Capsule {
        radius: f64,
        half_length: f64,
    },
    Cylinder {
        radius: f64,
        half_height: f64,
    },
    Plane {
        normal: [f64; 3],
        offset: f64,
    },
    Tube {
        points: Vec<[f64; 3]>,
        /// Radius at the first point.
        radius: f64,
        /// Radius at the last point; the radius changes linearly with distance along the polyline.
        end_radius: f64,
    },
    Repeat {
        period: [f64; 3],
        count: [u8; 3],
        child: Box<Self>,
    },
    Combine {
        op: Combine,
        a: Box<Self>,
        b: Box<Self>,
    },
    SmoothUnion {
        a: Box<Self>,
        b: Box<Self>,
        radius: f64,
    },
    Transform {
        op: Transform,
        child: Box<Self>,
    },
    Offset {
        child: Box<Self>,
        amount: f64,
    },
    Shell {
        child: Box<Self>,
        half_width: f64,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Combine {
    Union,
    Intersection,
    Difference,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Transform {
    Move([f64; 3]),
    Scale(f64),
    Mirror([bool; 3]),
}

/// Immutable expression and the guarantee propagated from its operators.
#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    pub(crate) expr: FieldExpr,
    pub(super) guarantee: DistanceGuarantee,
}
