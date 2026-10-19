//! Validated field-expression values and distance-guarantee propagation.
use super::model::{Combine, DistanceGuarantee, Field, FieldExpr, Transform};
use crate::GeometryError;

impl Field {
    /// Creates a constant-radius field around an ordered polyline.
    ///
    /// # Errors
    /// Requires 2 to `MAX_TUBE_POINTS` finite points, positive radius, and no zero-length segment.
    pub fn tube(points: Vec<[f64; 3]>, radius: f64) -> Result<Self, GeometryError> {
        Self::tapered_tube(points, radius, radius)
    }

    /// Creates a tube whose radius changes linearly with distance along the polyline, from `radius` at the first
    /// point to `end_radius` at the last (a limb, a tail or a horn). A tapered tube is not an exact distance.
    ///
    /// # Errors
    /// Same as [`Field::tube`]; the end radius must also be positive.
    pub fn tapered_tube(
        points: Vec<[f64; 3]>,
        radius: f64,
        end_radius: f64,
    ) -> Result<Self, GeometryError> {
        positive(radius, "tube radius")?;
        positive(end_radius, "tube end radius")?;
        if !(2..=super::MAX_TUBE_POINTS).contains(&points.len())
            || points
                .iter()
                .any(|point| finite_vec(*point, "tube point").is_err())
            || points.windows(2).any(|pair| {
                pair[0]
                    .iter()
                    .zip(pair[1])
                    .all(|(left, right)| left.to_bits() == right.to_bits())
            })
        {
            return Err(GeometryError::FieldParameter("tube points"));
        }
        let guarantee = if radius.to_bits() != end_radius.to_bits() {
            DistanceGuarantee::None
        } else if points.len() == 2 {
            DistanceGuarantee::Exact
        } else {
            DistanceGuarantee::LowerBound
        };
        Ok(Self::new(
            FieldExpr::Tube {
                points,
                radius,
                end_radius,
            },
            guarantee,
        ))
    }

    /// Creates an exact sphere at `center`.
    ///
    /// # Errors
    /// Non-finite coordinates or non-positive radius.
    pub fn sphere(center: [f64; 3], radius: f64) -> Result<Self, GeometryError> {
        finite_vec(center, "sphere center")?;
        positive(radius, "sphere radius")?;
        Ok(Self::new(
            FieldExpr::Sphere { center, radius },
            DistanceGuarantee::Exact,
        ))
    }

    /// Creates an axis-aligned ellipsoid with a conservative distance lower bound.
    ///
    /// The field scales the unit-sphere function by the smallest radius. This is not the exact
    /// Euclidean distance except when all radii match, but remains a 1-Lipschitz lower bound.
    ///
    /// # Errors
    /// Every radius must be finite and positive.
    pub fn ellipsoid(radii: [f64; 3]) -> Result<Self, GeometryError> {
        finite_vec(radii, "ellipsoid radii")?;
        if radii.into_iter().any(|value| value <= 0.0) {
            return Err(GeometryError::FieldParameter("ellipsoid radii"));
        }
        let equal =
            radii[0].to_bits() == radii[1].to_bits() && radii[1].to_bits() == radii[2].to_bits();
        let guarantee = if equal {
            DistanceGuarantee::Exact
        } else {
            DistanceGuarantee::LowerBound
        };
        Ok(Self::new(FieldExpr::Ellipsoid { radii }, guarantee))
    }

    /// Creates an exact rounded box centered on the origin.
    ///
    /// # Errors
    /// Non-positive half extent or a rounding radius outside the smallest half extent.
    pub fn rounded_box(half: [f64; 3], rounding: f64) -> Result<Self, GeometryError> {
        finite_vec(half, "box half extents")?;
        if half.iter().any(|value| *value <= 0.0)
            || !rounding.is_finite()
            || rounding < 0.0
            || rounding > half.into_iter().fold(f64::INFINITY, f64::min)
        {
            return Err(GeometryError::FieldParameter(
                "box dimensions or rounding radius",
            ));
        }
        Ok(Self::new(
            FieldExpr::Box { half, rounding },
            DistanceGuarantee::Exact,
        ))
    }

    /// Creates an exact torus around the Y axis.
    ///
    /// # Errors
    /// Radii must be finite, positive and the major radius must exceed the tube radius.
    pub fn torus(major: f64, tube: f64) -> Result<Self, GeometryError> {
        positive(major, "torus major radius")?;
        positive(tube, "torus tube radius")?;
        if major <= tube {
            return Err(GeometryError::FieldParameter(
                "torus major radius must exceed tube radius",
            ));
        }
        Ok(Self::new(
            FieldExpr::Torus { major, tube },
            DistanceGuarantee::Exact,
        ))
    }

    /// Creates an exact capsule along Y.
    ///
    /// # Errors
    /// The radius must be positive; half-length must be finite and non-negative.
    pub fn capsule(radius: f64, half_length: f64) -> Result<Self, GeometryError> {
        positive(radius, "capsule radius")?;
        non_negative(half_length, "capsule half length")?;
        Ok(Self::new(
            FieldExpr::Capsule {
                radius,
                half_length,
            },
            DistanceGuarantee::Exact,
        ))
    }

    /// Creates an exact finite cylinder along Y.
    ///
    /// # Errors
    /// Radius and half-height must be finite and positive.
    pub fn cylinder(radius: f64, half_height: f64) -> Result<Self, GeometryError> {
        positive(radius, "cylinder radius")?;
        positive(half_height, "cylinder half height")?;
        Ok(Self::new(
            FieldExpr::Cylinder {
                radius,
                half_height,
            },
            DistanceGuarantee::Exact,
        ))
    }

    /// Creates a normalized half-space plane.
    ///
    /// # Errors
    /// The normal and offset must be finite and the normal nonzero.
    pub fn plane(normal: [f64; 3], offset: f64) -> Result<Self, GeometryError> {
        finite_vec(normal, "plane normal")?;
        if !offset.is_finite() {
            return Err(GeometryError::FieldParameter("plane offset"));
        }
        let length = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
        if length <= f64::EPSILON {
            return Err(GeometryError::FieldParameter(
                "plane normal must be nonzero",
            ));
        }
        let normal = normal.map(|value| value / length);
        Ok(Self::new(
            FieldExpr::Plane {
                normal,
                offset: offset / length,
            },
            DistanceGuarantee::Exact,
        ))
    }

    /// Combines two fields by a hard union.
    #[must_use]
    pub fn union(self, other: Self) -> Self {
        let guarantee = hard_boolean_guarantee(self.guarantee, other.guarantee);
        Self::new(
            FieldExpr::Combine {
                op: Combine::Union,
                a: Box::new(self.expr),
                b: Box::new(other.expr),
            },
            guarantee,
        )
    }

    /// Combines two fields by intersection.
    #[must_use]
    pub fn intersection(self, other: Self) -> Self {
        let guarantee = hard_boolean_guarantee(self.guarantee, other.guarantee);
        Self::new(
            FieldExpr::Combine {
                op: Combine::Intersection,
                a: Box::new(self.expr),
                b: Box::new(other.expr),
            },
            guarantee,
        )
    }

    /// Subtracts another field from this one.
    #[must_use]
    pub fn difference(self, other: Self) -> Self {
        let guarantee = hard_boolean_guarantee(self.guarantee, other.guarantee);
        Self::new(
            FieldExpr::Combine {
                op: Combine::Difference,
                a: Box::new(self.expr),
                b: Box::new(other.expr),
            },
            guarantee,
        )
    }

    /// Smoothly unites two fields with a polynomial blend.
    ///
    /// # Errors
    /// Radius must be finite and positive.
    pub fn smooth_union(self, other: Self, radius: f64) -> Result<Self, GeometryError> {
        positive(radius, "smooth union radius")?;
        Ok(Self::new(
            FieldExpr::SmoothUnion {
                a: Box::new(self.expr),
                b: Box::new(other.expr),
                radius,
            },
            DistanceGuarantee::None,
        ))
    }

    /// Moves a field without changing its distance guarantee.
    ///
    /// # Errors
    /// Translation components must be finite.
    pub fn moved(self, delta: [f64; 3]) -> Result<Self, GeometryError> {
        finite_vec(delta, "field translation")?;
        let guarantee = self.guarantee;
        Ok(Self::new(
            FieldExpr::Transform {
                op: Transform::Move(delta),
                child: Box::new(self.expr),
            },
            guarantee,
        ))
    }

    /// Uniformly scales a field by a finite positive factor.
    ///
    /// # Errors
    /// Scale must be finite and positive.
    pub fn scaled(self, factor: f64) -> Result<Self, GeometryError> {
        positive(factor, "field scale")?;
        let guarantee = self.guarantee;
        Ok(Self::new(
            FieldExpr::Transform {
                op: Transform::Scale(factor),
                child: Box::new(self.expr),
            },
            guarantee,
        ))
    }

    /// Mirrors a field across selected coordinate planes.
    #[must_use]
    pub fn mirrored(self, axes: [bool; 3]) -> Self {
        let guarantee = self.guarantee;
        Self::new(
            FieldExpr::Transform {
                op: Transform::Mirror(axes),
                child: Box::new(self.expr),
            },
            guarantee,
        )
    }

    /// Offsets the child zero surface. The result is marked exact per the bounded offset contract.
    ///
    /// # Errors
    /// Offset must be finite.
    pub fn offset(self, amount: f64) -> Result<Self, GeometryError> {
        if !amount.is_finite() {
            return Err(GeometryError::FieldParameter("field offset"));
        }
        Ok(Self::new(
            FieldExpr::Offset {
                child: Box::new(self.expr),
                amount,
            },
            self.guarantee,
        ))
    }

    /// Creates a symmetric shell around the child zero surface.
    ///
    /// # Errors
    /// Half-width must be finite and positive.
    pub fn shell(self, half_width: f64) -> Result<Self, GeometryError> {
        positive(half_width, "shell half width")?;
        Ok(Self::new(
            FieldExpr::Shell {
                child: Box::new(self.expr),
                half_width,
            },
            self.guarantee,
        ))
    }

    /// Distance guarantee propagated by the expression.
    #[must_use]
    pub const fn guarantee(&self) -> DistanceGuarantee {
        self.guarantee
    }

    /// Approximate heap bytes owned by this expression tree.
    #[must_use]
    pub fn estimated_bytes(&self) -> usize {
        expr_bytes(&self.expr)
    }

    pub(crate) fn new(expr: FieldExpr, guarantee: DistanceGuarantee) -> Self {
        Self { expr, guarantee }
    }
}

fn expr_bytes(expr: &FieldExpr) -> usize {
    let node = std::mem::size_of::<FieldExpr>();
    match expr {
        FieldExpr::Combine { a, b, .. } | FieldExpr::SmoothUnion { a, b, .. } => node
            .saturating_add(expr_bytes(a))
            .saturating_add(expr_bytes(b)),
        FieldExpr::Transform { child, .. }
        | FieldExpr::Repeat { child, .. }
        | FieldExpr::Offset { child, .. }
        | FieldExpr::Shell { child, .. } => node.saturating_add(expr_bytes(child)),
        FieldExpr::Sphere { .. }
        | FieldExpr::Ellipsoid { .. }
        | FieldExpr::Box { .. }
        | FieldExpr::Torus { .. }
        | FieldExpr::Capsule { .. }
        | FieldExpr::Cylinder { .. }
        | FieldExpr::Plane { .. } => node,
        FieldExpr::Tube { points, .. } => {
            node.saturating_add(points.len().saturating_mul(std::mem::size_of::<[f64; 3]>()))
        }
    }
}

fn hard_boolean_guarantee(a: DistanceGuarantee, b: DistanceGuarantee) -> DistanceGuarantee {
    if a == DistanceGuarantee::None || b == DistanceGuarantee::None {
        DistanceGuarantee::None
    } else {
        DistanceGuarantee::LowerBound
    }
}

fn finite_vec(values: [f64; 3], name: &'static str) -> Result<(), GeometryError> {
    if values.into_iter().all(f64::is_finite) {
        Ok(())
    } else {
        Err(GeometryError::FieldParameter(name))
    }
}

fn positive(value: f64, name: &'static str) -> Result<(), GeometryError> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(GeometryError::FieldParameter(name))
    }
}

fn non_negative(value: f64, name: &'static str) -> Result<(), GeometryError> {
    if value.is_finite() && value >= 0.0 {
        Ok(())
    } else {
        Err(GeometryError::FieldParameter(name))
    }
}
