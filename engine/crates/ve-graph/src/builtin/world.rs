//! World terrain: tile-aware height grids with mask layers (ADR 0056), built by `ve-world`.
use super::spec;
use crate::spec::{Arity, OperatorSpec, OutputSpec, ParamSpec, PortSpec, PortType};
use crate::value::{ParamType, Unit, Value};

fn number(
    name: &'static str,
    unit: Unit,
    default: f64,
    min: f64,
    max: f64,
    doc: &'static str,
) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Num { unit, min, max },
        default: Some(Value::Num(default)),
        doc,
    }
}

fn whole(name: &'static str, default: i64, min: i64, max: i64, doc: &'static str) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Int { min, max },
        default: Some(Value::Int(default)),
        doc,
    }
}

fn colour(name: &'static str, default: [f64; 3], doc: &'static str) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Vec3 {
            unit: Unit::None,
            min: 0.0,
            max: 1.0,
        },
        default: Some(Value::List(
            default.iter().map(|v| Value::Num(*v)).collect(),
        )),
        doc,
    }
}

fn flag(name: &'static str, default: bool, doc: &'static str) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Bool,
        default: Some(Value::Bool(default)),
        doc,
    }
}

mod flora;
mod river;
mod surface;
mod terrain;

pub(super) fn all() -> Vec<Option<OperatorSpec>> {
    vec![
        terrain::tile(),
        terrain::mask(),
        terrain::biomes(),
        terrain::erode(),
        surface::material(),
        surface::place(),
        river::carve(),
        river::water(),
        river::fill(),
        flora::tree(),
    ]
}
