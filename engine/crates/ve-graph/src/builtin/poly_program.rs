//! `poly.program`: a short program run once for every vertex of a polygon mesh.

use super::{single, spec};
use crate::spec::{OperatorSpec, OutputSpec, ParamSpec, PortType};
use crate::value::{ParamType, Value};

const POLY: &[PortType] = &[PortType::Poly];

fn code_param(default: &'static str) -> ParamSpec {
    ParamSpec {
        name: "code",
        ty: ParamType::Str { max_len: 8192 },
        default: Some(Value::Str(default.to_owned())),
        doc: "The program. The last statement sets `out`, a vec3 (a point in metres).",
    }
}

fn count_param(name: &'static str, max: i64, default: i64, doc: &'static str) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Int { min: 1, max },
        default: Some(Value::Int(default)),
        doc,
    }
}

fn generators() -> Vec<Option<OperatorSpec>> {
    vec![
        spec(
            "curve.expr@1",
            "A space curve written as a program of `t`, from 0 to 1 along the curve, sampled `samples` times (also `i`, the sample number, and `count`). `out` is the point: a helix is `out = vec3(cos(t * 6.2832 * 3.0), t * 2.0, sin(t * 6.2832 * 3.0))`. A closed curve stops one step before the start so the first point is not repeated. Two neighbouring samples at the same point are refused. The curve plugs into curve.tube, solid.sweep, curve.resample and the other curve nodes. The program language is the one of field.expr (vec2/vec3, swizzles, operators, abs sign floor ceil fract sqrt exp log pow sin cos atan2 min max clamp mix step smoothstep length dot cross normalize select, ramp, noises), at most 8,192 bytes and 256 operations, deterministic everywhere.",
            vec![],
            OutputSpec::Fixed(PortType::Curve),
            vec![
                code_param("out = vec3(t, 0.0, 0.0)"),
                ParamSpec {
                    name: "samples",
                    ty: ParamType::Int { min: 2, max: 4096 },
                    default: Some(Value::Int(64)),
                    doc: "Points along the curve.",
                },
                ParamSpec {
                    name: "closed",
                    ty: ParamType::Bool,
                    default: Some(Value::Bool(false)),
                    doc: "Join the last point back to the first.",
                },
            ],
        ),
        spec(
            "curve.fit@1",
            "Fits a smooth curve to sketched or freehand points: give the points a pen or a mouse would trace (or measured data) and get a chain of cubic Béziers that stays within `tolerance` of every one of them, sampled `steps` times per piece, instead of the jagged polyline. Where the path turns sharper than `corner_angle` it is cut and each side fitted on its own, so a corner stays a corner and a smooth bend stays smooth; repeated points are ignored. It plugs into curve.tube, solid.sweep, inst.along_curve and the other curve nodes. Points are ordered, 3D, at most 4,096.",
            vec![],
            OutputSpec::Fixed(PortType::Curve),
            vec![
                ParamSpec {
                    name: "points",
                    ty: ParamType::Points3 {
                        min_points: 2,
                        max_points: 4096,
                        min: -1.0e6,
                        max: 1.0e6,
                    },
                    default: None,
                    doc: "Ordered points [x, y, z] in metres.",
                },
                ParamSpec {
                    name: "tolerance",
                    ty: ParamType::Num {
                        unit: crate::value::Unit::Length,
                        min: 1.0e-6,
                        max: 1000.0,
                    },
                    default: Some(Value::Num(0.02)),
                    doc: "The most the fitted curve may stray from an input point, in metres: larger gives fewer, smoother pieces; smaller follows the points closely.",
                },
                ParamSpec {
                    name: "corner_angle",
                    ty: ParamType::Num {
                        unit: crate::value::Unit::Angle,
                        min: 0.0,
                        max: std::f64::consts::PI,
                    },
                    default: Some(Value::Num(60.0_f64.to_radians())),
                    doc: "Turns sharper than this are kept as corners (60 degrees by default); use 180 for a corner-free curve.",
                },
                ParamSpec {
                    name: "closed",
                    ty: ParamType::Bool,
                    default: Some(Value::Bool(false)),
                    doc: "Treat the points as a loop and join the last back to the first smoothly.",
                },
                ParamSpec {
                    name: "steps",
                    ty: ParamType::Int { min: 2, max: 64 },
                    default: Some(Value::Int(12)),
                    doc: "Samples along each fitted piece of the curve.",
                },
            ],
        ),
        spec(
            "poly.surface_expr@1",
            "A surface written as a program of `u` and `v`, each from 0 to 1 across `columns` by `rows` quads (the mesh has UVs, drawn smooth). `out` is the point: a torus is `out = vec3((1.0 + 0.3 * cos(v * 6.2832)) * cos(u * 6.2832), 0.3 * sin(v * 6.2832), (1.0 + 0.3 * cos(v * 6.2832)) * sin(u * 6.2832))`. Link the result into poly.program, poly.subdivide, solid.material and the other polygon nodes. The program language is the one of field.expr, at most 8,192 bytes and 256 operations, deterministic everywhere.",
            vec![],
            OutputSpec::Fixed(PortType::Poly),
            vec![
                code_param("out = vec3(u, 0.0, v)"),
                count_param("columns", 512, 32, "Quads along u."),
                count_param("rows", 512, 32, "Quads along v."),
                ParamSpec {
                    name: "flip",
                    ty: ParamType::Bool,
                    default: Some(Value::Bool(false)),
                    doc: "Reverse which side of the surface is the front. The front follows from the formula; if the surface looks inside out (dark, or seen through), set this.",
                },
            ],
        ),
    ]
}

fn script() -> Option<OperatorSpec> {
    spec(
        "poly.script@1",
        "A JavaScript program builds a polygon mesh, for shapes the other nodes cannot describe (a loop with branches, a maze, a rule that grows geometry). Make a `Mesh`: `const m = new Mesh(); const a = m.vertex(x, y, z); ... m.face(a, b, c, d); m` (the last expression is the result; `vertex` returns the vertex number, `face` takes three or more). When a mesh is linked into `in` the script can read it as `input`: `input.points` ([[x, y, z], ...]) and `input.faces` ([[a, b, c], ...]) and build a changed copy. Plain JavaScript (let, const, loops, functions, arrays, Math); no clock, files or network, and `Math.random()` is seeded by `seed`, so the same script and seed always give the same mesh. It is stopped after `budget` units of work (about 10,000 steps each), at 64 MB of memory, and at 200,000 points or faces; an error names the line and column. Example, a staircase of n steps: `const m = new Mesh(); const n = 6; for (let i = 0; i < n; i++) { const y = i * 0.2, z = i * 0.3; const a = m.vertex(0, y, z), b = m.vertex(1, y, z), c = m.vertex(1, y, z + 0.3), d = m.vertex(0, y, z + 0.3); m.face(a, d, c, b); } m`. op(id, params, inputs) calls any other operator as a function and returns its result as data: a mesh as {points, faces}, a curve as {points, closed}, a number as {number}; params are the node's parameters by name in canonical units (metres, radians; missing ones take their defaults and a wrong name or value is refused with the reason); inputs is an object by port name (an array feeds a variadic port such as solid.difference's `in`), each a mesh, a curve {points, closed} or a number. Example, a block drilled by six spheres: `let s = op('solid.box@1', {size: [3, 3, 3]}); for (let i = 0; i < 6; i++) { const b = op('solid.sphere@1', {radius: 0.4}); const at = op('xform.move@1', {by: [Math.cos(i * 1.1), Math.sin(i * 2.3), Math.sin(i * 1.1)]}, {in: b}); s = op('solid.difference@1', {}, {in: [s, at]}); } s`. At most 128 op calls per script, and a script cannot call poly.script. Prefer poly.program, curve.expr and the editing nodes when they say it; use the script for logic they cannot express.",
        vec![super::PortSpec {
            name: "in",
            accepts: POLY,
            arity: crate::spec::Arity::Optional,
        }],
        OutputSpec::Fixed(PortType::Poly),
        vec![
            ParamSpec {
                name: "code",
                ty: ParamType::Str { max_len: 16384 },
                default: Some(Value::Str("const m = new Mesh();\nconst a = m.vertex(0, 0, 0), b = m.vertex(1, 0, 0), c = m.vertex(0, 1, 0);\nm.face(a, b, c);\nm".to_owned())),
                doc: "The JavaScript program, at most 16,384 bytes. Its last expression is the result.",
            },
            ParamSpec {
                name: "seed",
                ty: ParamType::Int { min: 0, max: 4_294_967_295 },
                default: Some(Value::Int(1)),
                doc: "Seed of the script's Math.random().",
            },
            ParamSpec {
                name: "budget",
                ty: ParamType::Int { min: 1, max: 20_000 },
                default: Some(Value::Int(2_000)),
                doc: "How much work the script may do before it is stopped, in units of about 10,000 steps.",
            },
        ],
    )
}

pub(super) fn all() -> Vec<Option<OperatorSpec>> {
    let mut all = generators();
    all.push(script());
    all.extend(vertex_program());
    all
}

fn vertex_program() -> Vec<Option<OperatorSpec>> {
    vec![spec(
        "poly.program@1",
        "Runs a short program once for every vertex and uses the result: `out = ...` in the field-expression language. The program sees the vertex position `p`, its smoothed normal `n`, its first UV `uv`, its number `i` (0 for the first vertex) and the vertex count `count`, plus `attr(\"name\")` for any per-vertex number layer of the mesh. Operators: + - * / %, comparisons, && || !, vec2/vec3 and swizzles (`p.xz`); functions: abs sign floor ceil fract sqrt exp log pow sin cos atan2 min max clamp mix step smoothstep length distance dot cross normalize select, ramp(x, stop, value, ...), perlin(p, seed), fbm(p, octaves, seed), ridged(p, octaves, seed), voronoi(p, seed). `mode` says what `out` means: `position` (a vec3, the new position), `offset` (a vec3 added to the position), `along_normal` (a number, the distance to move along the normal) or `layer` (a number stored as the per-vertex layer called `layer_name`, for other nodes to read with attr). Example, a rippled sheet: mode=offset, code=`out = vec3(0.0, 0.1 * sin(p.x * 8.0 + p.z * 5.0), 0.0)`. Faces and connectivity are unchanged. Everything is deterministic and identical on every platform.",
        vec![single("in", POLY)],
        OutputSpec::Fixed(PortType::Poly),
        vec![
            ParamSpec {
                name: "code",
                ty: ParamType::Str { max_len: 8192 },
                default: Some(Value::Str("out = vec3(0.0, 0.0, 0.0)".to_owned())),
                doc: "The program, at most 8,192 bytes and 256 operations. The last statement must set `out`, of the type `mode` asks for.",
            },
            ParamSpec {
                name: "mode",
                ty: ParamType::Choice {
                    options: &["position", "offset", "along_normal", "layer"],
                },
                default: Some(Value::Str("offset".to_owned())),
                doc: "What `out` is: the new position, an offset added to the position, a distance along the normal, or a number stored in a layer.",
            },
            ParamSpec {
                name: "layer_name",
                ty: ParamType::Str { max_len: 32 },
                default: Some(Value::Str("program".to_owned())),
                doc: "Name of the per-vertex layer written when mode is `layer`.",
            },
        ],
    )]
}
