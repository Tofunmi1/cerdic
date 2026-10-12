//! Literals as written (`3mm`, `90deg`, `[0, 1]`) to the typed values a parameter stores.

use super::lex::{Fail, fail};
use super::parse::Lit;
use crate::diag::Diagnostic;
use crate::value::{ParamType, Unit, Value};

/// What a parameter expects, for "missing parameter" messages.
pub(super) fn describe(ty: &ParamType) -> String {
    match ty {
        ParamType::Num { unit, .. } => format!("a number ({})", unit_name(*unit)),
        ParamType::Int { .. } => "a whole number".to_owned(),
        ParamType::Bool => "true or false".to_owned(),
        ParamType::Str { .. } => "text in quotes".to_owned(),
        ParamType::Vec2 { unit, .. } => format!("[x, y] ({})", unit_name(*unit)),
        ParamType::Vec3 { unit, .. } => format!("[x, y, z] ({})", unit_name(*unit)),
        ParamType::Points2 { .. } => "a list of [x, y] points (metres)".to_owned(),
    }
}

fn unit_name(unit: Unit) -> &'static str {
    match unit {
        Unit::Length => "metres; mm and cm suffixes allowed",
        Unit::Angle => "degrees; write rad for radians",
        Unit::Ratio => "a factor",
        Unit::None => "a plain number",
    }
}

fn scale(unit: Unit, suffix: &str) -> Option<f64> {
    match (unit, suffix) {
        (Unit::Length, "" | "m") | (Unit::Angle, "rad") | (Unit::Ratio | Unit::None, "") => {
            Some(1.0)
        }
        (Unit::Length, "mm") => Some(1.0e-3),
        (Unit::Length, "cm") => Some(1.0e-2),
        (Unit::Angle, "" | "deg") => Some(std::f64::consts::PI / 180.0),
        _ => None,
    }
}

fn number(lit: &Lit, unit: Unit) -> Result<f64, Fail> {
    let Lit::Num {
        value,
        unit: suffix,
        span,
        ..
    } = lit
    else {
        return Err(fail("param.type", lit.span(), "expected a number"));
    };
    let factor = scale(unit, suffix).ok_or_else(|| {
        fail(
            "param.type",
            *span,
            format!(
                "the unit {suffix:?} does not fit here (expected {})",
                unit_name(unit)
            ),
        )
    })?;
    Ok(value * factor)
}

fn coerce(ty: &ParamType, lit: &Lit) -> Result<Value, Fail> {
    let wrong = |what: &str| fail("param.type", lit.span(), format!("expected {what}"));
    match (ty, lit) {
        (ParamType::Num { unit, .. }, _) => number(lit, *unit).map(Value::Num),
        (
            ParamType::Int { .. },
            Lit::Num {
                value,
                whole: true,
                unit,
                ..
            },
        ) if unit.is_empty() =>
        {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "a whole literal; range is checked next"
            )]
            Ok(Value::Int(*value as i64))
        }
        (ParamType::Int { .. }, _) => Err(wrong("a whole number")),
        (ParamType::Bool, Lit::Bool(b, _)) => Ok(Value::Bool(*b)),
        (ParamType::Bool, _) => Err(wrong("true or false")),
        (ParamType::Str { .. }, Lit::Text(t, _)) => Ok(Value::Str(t.clone())),
        (ParamType::Str { .. }, _) => Err(wrong("text in quotes")),
        (ParamType::Vec2 { unit, .. } | ParamType::Vec3 { unit, .. }, Lit::List(items, _)) => items
            .iter()
            .map(|i| number(i, *unit).map(Value::Num))
            .collect::<Result<_, _>>()
            .map(Value::List),
        (ParamType::Points2 { .. }, Lit::List(items, _)) => items
            .iter()
            .map(|point| match point {
                Lit::List(parts, _) => parts
                    .iter()
                    .map(|p| number(p, Unit::Length).map(Value::Num))
                    .collect::<Result<_, _>>()
                    .map(Value::List),
                other => Err(fail("param.type", other.span(), "expected a [x, y] point")),
            })
            .collect::<Result<_, _>>()
            .map(Value::List),
        _ => Err(wrong(&describe(ty))),
    }
}

/// A literal coerced to the parameter's type and checked against its bounds.
pub(super) fn param_value(ty: &ParamType, key: &str, lit: &Lit) -> Result<Value, Fail> {
    let value = coerce(ty, lit)?;
    ty.check(&value)
        .map_err(|issue| Box::new(Diagnostic::from_issue(issue, None, key).at_span(lit.span())))
}
