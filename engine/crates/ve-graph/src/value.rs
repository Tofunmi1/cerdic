//! Parameter values and the checks that turn raw input into canonical, bounded data.

use serde::{Deserialize, Serialize};

use crate::limits::{MAX_LIST_VALUES, MAX_PARAM_STRING_BYTES};

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
    /// One of a fixed set of words, stored as text.
    Choice {
        /// The accepted words.
        options: &'static [&'static str],
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
    /// A list of `[x, y, z]` points, each coordinate within `min..=max`.
    Points3 {
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
                Value::Str(s) if s.len() <= max_len.min(MAX_PARAM_STRING_BYTES) => {
                    Ok(value.clone())
                }
                #[expect(clippy::cast_precision_loss, reason = "lengths are far below 2^53")]
                Value::Str(s) => Err(ParamIssue::range(0.0, max_len as f64, s.len() as f64)),
                _ => Err(ParamIssue::wrong_type("text")),
            },
            Self::Choice { options } => match value {
                Value::Str(s) if options.contains(&s.as_str()) => Ok(value.clone()),
                Value::Str(s) => Err(ParamIssue {
                    code: "param.range",
                    message: format!("{s:?} is not one of {}", options.join(", ")),
                    limit: None,
                    observed: None,
                }),
                _ => Err(ParamIssue::wrong_type("one of the listed words")),
            },
            Self::Vec2 { min, max, .. } => components(value, 2, min, max).map(Value::List),
            Self::Vec3 { min, max, .. } => components(value, 3, min, max).map(Value::List),
            Self::Points2 {
                min_points,
                max_points,
                min,
                max,
            } => points(value, 2, (min_points, max_points), (min, max)),
            Self::Points3 {
                min_points,
                max_points,
                min,
                max,
            } => points(value, 3, (min_points, max_points), (min, max)),
        }
    }
}

/// A bounded list of points with `dims` coordinates each; `count` and `range` are `(min, max)`.
fn points(
    value: &Value,
    dims: usize,
    count: (usize, usize),
    range: (f64, f64),
) -> Result<Value, ParamIssue> {
    let Value::List(items) = value else {
        return Err(ParamIssue::wrong_type(&format!(
            "a list of {dims}-number points"
        )));
    };
    let cap = count.1.min(MAX_LIST_VALUES);
    if items.len() < count.0 || items.len() > cap {
        #[expect(clippy::cast_precision_loss, reason = "counts are far below 2^53")]
        return Err(ParamIssue::range(
            count.0 as f64,
            cap as f64,
            items.len() as f64,
        ));
    }
    items
        .iter()
        .map(|item| components(item, dims, range.0, range.1).map(Value::List))
        .collect::<Result<Vec<_>, _>>()
        .map(Value::List)
}

#[cfg(test)]
#[path = "value_tests.rs"]
mod tests;
