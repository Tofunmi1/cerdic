//! Simulation programs: any simulation written as state arrays, kernels and a schedule (SimL), run live.

use super::spec;
use crate::spec::{Arity, OperatorSpec, OutputSpec, ParamSpec, PortSpec, PortType};
use crate::value::{ParamType, Unit, Value};

const POLY: &[PortType] = &[PortType::Poly];

pub(super) fn all() -> Vec<Option<OperatorSpec>> {
    vec![spec(
        "sim.program@1",
        "A simulation you write, run live: cloth, hair, ropes, flocks, swarms, crowds, soft bodies, fire, anything with state that changes by rules. The `source` declares `param` settable numbers, `state` arrays with one value per element (`state pos: vec3[256]`), `kernel`s that run for each element of an array (`kernel move each i in pos { pos[i] += vel[i] * dt }`; a kernel reads the state as it was when it began and writes only its own element) or once, a `schedule` listing the kernels to run each step (`schedule flock; move`, with `repeat 6 { ... }` for solver passes), and an `output particles pos` saying which vec3 array is drawn. Statements: `let`, assignment and `+= -= *= /=`, `if/else`, `for k in 0..8` with constant bounds, and `for j in neighbors(pos, p, radius) cap 24 { ... }` over nearby elements. Values: f32, vec2, vec3, vec4, bool; operators + - * / % < <= > >= == != && || !; swizzles (`p.xz`); functions abs sign floor ceil fract sqrt exp log sin cos min max pow atan2 clamp mix step smoothstep length distance dot cross normalize select, vec2/vec3/vec4, hash(a,b) and rand(a,b) (deterministic, 0 to 1). Builtins: i (the element), n (element count), dt, t, step, seed. A kernel named `init` sets the start state. Every particle is drawn as a small tetrahedron of the given `size`, so the part is a mesh; up to 4,000 particles. The same source gives the same result on every machine. Errors name the line, the column and the fix. A program can also drive a mesh you give on `in` (a cape from poly.grid, a rope, hair strips): declare `input mesh m` and `state pos: vec3[m]` (one element per vertex), read `rest(m, i)` (where vertex i was), gather spring forces with `for j, len in adjacent(m, i) { ... }` (each neighbour and the edge's rest length), follow bones with `input bone chest` and `bone_point(chest, p)` / `bone_vector(chest, d)` (carry a point or direction by the bone's pose; the bone is the part with that output name, in a game the copy's own), collide with `sdf_capsule(p, a, b, r)` and `closest_segment(p, a, b)`, and end with `output mesh pos` to move the vertices. Physics works both ways: `input bone crate` also reads a part that a phys.body moves (its current pose), and `output push crate push` (a vec3 array) changes the speed of the physics body of the part `crate` every step by the sum of the array in metres per second (buoyancy, wind, a magnet); in a game each copy of a model reads and pushes its own body. Set `library=\"cloth\"` (with `source` empty) for the ready-made cloth program: hang it from the bone `anchor`, collide with the bone `body`, and tune `pin_height`, `collider_radius`, `gravity`, `damping` and `relax` by writing your own copy of the text.",
        vec![PortSpec { name: "in", accepts: POLY, arity: Arity::Optional }],
        OutputSpec::Fixed(PortType::Poly),
        vec![
            ParamSpec {
                name: "source",
                ty: ParamType::Str { max_len: 65536 },
                default: Some(Value::Str(String::new())),
                doc: "The program in SimL.",
            },
            ParamSpec {
                name: "library",
                ty: ParamType::Choice { options: &["none", "cloth", "bridge"] },
                default: Some(Value::Str("none".to_owned())),
                doc: "A ready-made program used when `source` is empty: cloth (needs an `in` mesh), or bridge (41 planks hung between two posts that snap at the link `cut` when you set it with sim.set_param; see the sim guide).",
            },
            ParamSpec {
                name: "values",
                ty: ParamType::Str { max_len: 2048 },
                default: Some(Value::Str(String::new())),
                doc: "Settings for the program's `param`s, `name=number` or `name=x,y,z` separated by semicolons, for example \"collider_radius=0.2; collider_top=0,0.7,-0.05\". Unknown names and wrong sizes are refused.",
            },
            ParamSpec {
                name: "seed",
                ty: ParamType::Int { min: 0, max: 2_147_483_647 },
                default: Some(Value::Int(1)),
                doc: "Seeds hash() and rand(); the same seed gives the same simulation.",
            },
            ParamSpec {
                name: "rate",
                ty: ParamType::Num { unit: Unit::None, min: 1.0, max: 240.0 },
                default: Some(Value::Num(60.0)),
                doc: "Steps per second: every step advances the program by 1/rate seconds, however fast frames come.",
            },
            ParamSpec {
                name: "size",
                ty: ParamType::Num { unit: Unit::Length, min: 0.001, max: 5.0 },
                default: Some(Value::Num(0.05)),
                doc: "Width of the small tetrahedron drawn for every particle, in metres.",
            },
        ],
    )]
}
