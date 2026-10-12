//! Parameter values and the checks that turn raw input into canonical, bounded data.

use serde::{Deserialize, Serialize};

use crate::limits::{MAX_LIST_VALUES, MAX_STRING_BYTES};

/// A parameter value as it appears in a document or a command.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub enum Value {
    /// A true or false switch.
    Bool(bool),
    /// A whole number.
    Int(i64),
    /// A real number (stored in SI units: metres, radians).
    Num(f64),
    /// Text.
    Str(String),
    /// A list: vectors and point lists.
    List(Vec<Value>),
}

impl Value {
    /// The value as a real number, accepting whole numbers too.
    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Num(n) => Some(*n),
            #[expect(clippy::cast_precision_loss, reason = "parameters are far below 2^53")]
            Self::Int(i) => Some(*i as f64),
            _ => None,
        }
    }
}

/// What a number means; display and bounds only, values are always stored in SI.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    /// A pure number.
    None,
    /// Metres.
    Length,
    /// Radians.
    Angle,
    /// A dimensionless factor.
    Ratio,
}

/// The type and bounds of a parameter.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ParamType {
    /// A real number within `min..=max`.
    Num {
        /// Meaning of the number.
        unit: Unit,
        /// Smallest accepted value.
        min: f64,
        /// Largest accepted value.
        max: f64,
    },
    /// A whole number within `min..=max`.
    Int {
        /// Smallest accepted value.
        min: i64,
        /// Largest accepted value.
        max: i64,
    },
    /// A switch.
    Bool,
    /// Text of at most `max_len` bytes.
    Str {
        /// Longest accepted text, in bytes.
        max_len: usize,
    },
    /// Two numbers, each within `min..=max`.
    Vec2 {
        /// Meaning of the numbers.
        unit: Unit,
        /// Smallest accepted component.
        min: f64,
        /// Largest accepted component.
        max: f64,
    },
    /// Three numbers, each within `min..=max`.
    Vec3 {
        /// Meaning of the numbers.
        unit: Unit,
        /// Smallest accepted component.
        min: f64,
        /// Largest accepted component.
        max: f64,
    },
    /// A list of `[x, y]` points, each coordinate within `min..=max`.
    Points2 {
        /// Fewest points accepted.
        min_points: usize,
        /// Most points accepted.
        max_points: usize,
        /// Smallest accepted coordinate.
        min: f64,
        /// Largest accepted coordinate.
        max: f64,
    },
}

/// Why a value was refused.
#[derive(Clone, Debug, PartialEq)]
pub struct ParamIssue {
    /// The diagnostic code (`param.type`, `param.range` or `param.non_finite`).
    pub code: &'static str,
    /// What was wrong, in words.
    pub message: String,
    /// The limit that was exceeded, when there is one.
    pub limit: Option<f64>,
    /// The offending value, when there is one.
    pub observed: Option<f64>,
}

impl ParamIssue {
    fn wrong_type(expected: &str) -> Self {
        Self {
            code: "param.type",
            message: format!("expected {expected}"),
            limit: None,
            observed: None,
        }
    }

    fn range(min: f64, max: f64, observed: f64) -> Self {
        Self {
            code: "param.range",
            message: format!("{observed} is outside {min} to {max}"),
            limit: Some(if observed < min { min } else { max }),
            observed: Some(observed),
        }
    }
}

fn number(value: &Value, min: f64, max: f64) -> Result<f64, ParamIssue> {
    let n = value
        .as_f64()
        .ok_or_else(|| ParamIssue::wrong_type("a number"))?;
    if !n.is_finite() {
        return Err(ParamIssue {
            code: "param.non_finite",
            message: "numbers must be finite".to_owned(),
            limit: None,
            observed: None,
        });
    }
    if n < min || n > max {
        return Err(ParamIssue::range(min, max, n));
    }
    Ok(n)
}

fn components(value: &Value, len: usize, min: f64, max: f64) -> Result<Vec<Value>, ParamIssue> {
    let Value::List(items) = value else {
        return Err(ParamIssue::wrong_type(&format!("a list of {len} numbers")));
    };
    if items.len() != len {
        return Err(ParamIssue::wrong_type(&format!("a list of {len} numbers")));
    }
    items
        .iter()
        .map(|item| number(item, min, max).map(Value::Num))
        .collect()
}

impl ParamType {
    /// Checks `value` against the type and returns it in canonical form: numbers become
    /// [`Value::Num`] (or [`Value::Int`] for integer parameters) so equal inputs serialise equally.
    ///
    /// # Errors
    /// A [`ParamIssue`] describing the first problem found.
    pub fn check(&self, value: &Value) -> Result<Value, ParamIssue> {
        match *self {
            Self::Num { min, max, .. } => number(value, min, max).map(Value::Num),
            Self::Int { min, max } => match value {
                Value::Int(i) if (min..=max).contains(i) => Ok(Value::Int(*i)),
                #[expect(clippy::cast_precision_loss, reason = "bounds are far below 2^53")]
                Value::Int(i) => Err(ParamIssue::range(min as f64, max as f64, *i as f64)),
                _ => Err(ParamIssue::wrong_type("a whole number")),
            },
            Self::Bool => match value {
                Value::Bool(_) => Ok(value.clone()),
                _ => Err(ParamIssue::wrong_type("true or false")),
            },
            Self::Str { max_len } => match value {
                Value::Str(s) if s.len() <= max_len.min(MAX_STRING_BYTES) => Ok(value.clone()),
                #[expect(clippy::cast_precision_loss, reason = "lengths are far below 2^53")]
                Value::Str(s) => Err(ParamIssue::range(0.0, max_len as f64, s.len() as f64)),
                _ => Err(ParamIssue::wrong_type("text")),
            },
            Self::Vec2 { min, max, .. } => components(value, 2, min, max).map(Value::List),
            Self::Vec3 { min, max, .. } => components(value, 3, min, max).map(Value::List),
            Self::Points2 {
                min_points,
                max_points,
                min,
                max,
            } => points(value, min_points, max_points, min, max),
        }
    }
}

fn points(
    value: &Value,
    min_points: usize,
    max_points: usize,
    min: f64,
    max: f64,
) -> Result<Value, ParamIssue> {
    let Value::List(items) = value else {
        return Err(ParamIssue::wrong_type("a list of [x, y] points"));
    };
    let cap = max_points.min(MAX_LIST_VALUES);
    if items.len() < min_points || items.len() > cap {
        #[expect(clippy::cast_precision_loss, reason = "counts are far below 2^53")]
        return Err(ParamIssue::range(
            min_points as f64,
            cap as f64,
            items.len() as f64,
        ));
    }
    items
        .iter()
        .map(|item| components(item, 2, min, max).map(Value::List))
        .collect::<Result<Vec<_>, _>>()
        .map(Value::List)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEN: ParamType = ParamType::Num {
        unit: Unit::Length,
        min: 0.0,
        max: 10.0,
    };

    #[test]
    fn numbers_are_canonicalised_and_bounded() {
        assert_eq!(LEN.check(&Value::Int(3)), Ok(Value::Num(3.0)));
        assert_eq!(LEN.check(&Value::Num(3.0)), Ok(Value::Num(3.0)));
        assert_eq!(
            LEN.check(&Value::Num(-1.0)).unwrap_err().code,
            "param.range"
        );
        assert_eq!(
            LEN.check(&Value::Num(f64::NAN)).unwrap_err().code,
            "param.non_finite"
        );
        assert_eq!(
            LEN.check(&Value::Num(f64::INFINITY)).unwrap_err().code,
            "param.non_finite"
        );
        assert_eq!(
            LEN.check(&Value::Str("1".into())).unwrap_err().code,
            "param.type"
        );
    }

    #[test]
    fn integers_do_not_accept_fractions() {
        let t = ParamType::Int { min: 3, max: 512 };
        assert_eq!(t.check(&Value::Int(24)), Ok(Value::Int(24)));
        assert_eq!(t.check(&Value::Int(2)).unwrap_err().code, "param.range");
        assert_eq!(t.check(&Value::Num(24.5)).unwrap_err().code, "param.type");
    }

    #[test]
    fn vectors_and_point_lists_are_checked_per_component() {
        let v3 = ParamType::Vec3 {
            unit: Unit::Length,
            min: -1.0,
            max: 1.0,
        };
        let ok = Value::List(vec![Value::Int(0), Value::Num(0.5), Value::Num(-1.0)]);
        assert!(v3.check(&ok).is_ok());
        let short = Value::List(vec![Value::Int(0)]);
        assert_eq!(v3.check(&short).unwrap_err().code, "param.type");
        let big = Value::List(vec![Value::Int(0), Value::Int(0), Value::Int(9)]);
        assert_eq!(v3.check(&big).unwrap_err().code, "param.range");

        let pts = ParamType::Points2 {
            min_points: 3,
            max_points: 4,
            min: -1.0,
            max: 1.0,
        };
        let pt = |x: f64, y: f64| Value::List(vec![Value::Num(x), Value::Num(y)]);
        assert!(
            pts.check(&Value::List(vec![pt(0.0, 0.0), pt(1.0, 0.0), pt(0.0, 1.0)]))
                .is_ok()
        );
        assert_eq!(
            pts.check(&Value::List(vec![pt(0.0, 0.0)]))
                .unwrap_err()
                .code,
            "param.range"
        );
        let five = Value::List((0..5).map(|_| pt(0.0, 0.0)).collect());
        assert_eq!(pts.check(&five).unwrap_err().code, "param.range");
    }

    #[test]
    fn text_is_bounded_in_bytes() {
        let t = ParamType::Str { max_len: 4 };
        assert!(t.check(&Value::Str("abcd".into())).is_ok());
        assert_eq!(
            t.check(&Value::Str("abcde".into())).unwrap_err().code,
            "param.range"
        );
    }
}
