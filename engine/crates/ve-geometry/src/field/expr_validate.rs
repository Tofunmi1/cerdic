//! Shared scalar validation and distance-guarantee propagation for field constructors.
use super::model::DistanceGuarantee;
use crate::GeometryError;

pub(super) fn hard_boolean_guarantee(
    a: DistanceGuarantee,
    b: DistanceGuarantee,
) -> DistanceGuarantee {
    if a == DistanceGuarantee::None || b == DistanceGuarantee::None {
        DistanceGuarantee::None
    } else {
        DistanceGuarantee::LowerBound
    }
}

pub(super) fn finite_vec(values: [f64; 3], name: &'static str) -> Result<(), GeometryError> {
    if values.into_iter().all(f64::is_finite) {
        Ok(())
    } else {
        Err(GeometryError::FieldParameter(name))
    }
}

pub(super) fn positive(value: f64, name: &'static str) -> Result<(), GeometryError> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(GeometryError::FieldParameter(name))
    }
}

pub(super) fn non_negative(value: f64, name: &'static str) -> Result<(), GeometryError> {
    if value.is_finite() && value >= 0.0 {
        Ok(())
    } else {
        Err(GeometryError::FieldParameter(name))
    }
}
