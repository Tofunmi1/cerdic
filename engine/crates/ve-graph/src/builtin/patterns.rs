//! Patterns: functions of position written in the field-expression language (ADR 0060), and terrain layers seen as functions.
use super::spec;
use crate::spec::{Arity, OperatorSpec, OutputSpec, ParamSpec, PortSpec, PortType};
use crate::value::{ParamType, Value};

const PATTERN: &[PortType] = &[PortType::Pattern];
const TILE: &[PortType] = &[PortType::Poly];

fn bound(name: &'static str) -> PortSpec {
    PortSpec {
        name,
        accepts: PATTERN,
        arity: Arity::Optional,
    }
}

fn expr() -> Option<OperatorSpec> {
    spec(
        "field.expr@1",
        "A function of position, written as a short program: `out = ...` is the result, built from the position `p`, the surface normal `n` (`n.y` is 1 on flat ground and falls on slopes), `uv`, numbers, vec2/vec3/vec4, swizzles (`p.xz`), + - * / %, comparisons, && || !, and the functions abs sign floor ceil fract sqrt exp log pow sin cos atan2 min max clamp mix step smoothstep length distance dot cross normalize select, ramp(x, stop, value, ...) and curve(x, stop, value, ...) for piecewise-linear curves, and the noises perlin(p, seed), fbm(p, octaves, seed), ridged(p, octaves, seed), voronoi(p, seed). Statements are separated by `;`, for example `h = smoothstep(10.0, 40.0, p.y); out = h * select(n.y > 0.9, 1.0, 0.0)`. Link other patterns into a, b, c, d and read them by those names. Plug the result into a pattern input such as scatter.place's `where`: 1 keeps full density, 0 plants nothing. Numbers are metres and unitless; everything is deterministic and identical on every platform.",
        vec![bound("a"), bound("b"), bound("c"), bound("d")],
        OutputSpec::Fixed(PortType::Pattern),
        vec![ParamSpec {
            name: "code",
            ty: ParamType::Str { max_len: 8192 },
            default: Some(Value::Str("out = 1.0".to_owned())),
            doc: "The expression program, at most 8,192 bytes and 256 operations. The last statement must set `out`.",
        }],
    )
}

fn mask_from() -> Option<OperatorSpec> {
    spec(
        "field.mask_from@1",
        "A terrain layer as a pattern: the value of a named layer of a terrain tile (made by terrain.mask, erosion or a river) at any position, smoothly interpolated, 0 outside the tile. Use it to let one mask drive several things (scatter, other patterns).",
        vec![super::single("in", TILE)],
        OutputSpec::Fixed(PortType::Pattern),
        vec![ParamSpec {
            name: "layer",
            ty: ParamType::Str { max_len: 32 },
            default: Some(Value::Str(String::new())),
            doc: "Name of the layer, made first with terrain.mask or a node that writes layers.",
        }],
    )
}

pub(super) fn all() -> Vec<Option<OperatorSpec>> {
    vec![expr(), mask_from()]
}
