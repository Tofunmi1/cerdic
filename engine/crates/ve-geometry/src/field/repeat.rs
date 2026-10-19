//! Bounded folded-coordinate repetition for spatial fields.
use super::MAX_REPEAT_COUNT;
use super::model::{Field, FieldExpr};
use crate::GeometryError;

impl Field {
    /// Repeats the field by a finite count of cells on each side of the origin.
    ///
    /// The fold is evaluated once per sample; it does not expand the tape. Counts are bounded
    /// independently per axis and zero disables repetition on that axis.
    ///
    /// # Errors
    /// Periods must be finite and positive; counts cannot exceed `MAX_REPEAT_COUNT`.
    pub fn repeated(self, period: [f64; 3], count: [u32; 3]) -> Result<Self, GeometryError> {
        if period
            .into_iter()
            .any(|value| !value.is_finite() || value <= 0.0)
            || count.into_iter().any(|value| value > MAX_REPEAT_COUNT)
        {
            return Err(GeometryError::FieldParameter(
                "field repeat period or count",
            ));
        }
        let mut bounded_count = [0_u8; 3];
        for axis in 0..3 {
            bounded_count[axis] = u8::try_from(count[axis])
                .map_err(|_| GeometryError::FieldParameter("field repeat count"))?;
        }
        let guarantee = self.guarantee;
        Ok(Self::new(
            FieldExpr::Repeat {
                period,
                count: bounded_count,
                child: Box::new(self.expr),
            },
            guarantee,
        ))
    }
}
