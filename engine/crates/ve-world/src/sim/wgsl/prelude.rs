//! The fixed part of every generated shader (clock, params, hash, geometry helpers) and the per-type helpers WGSL needs because it has
//! no overloading.
use std::fmt::Write;

use crate::sim::Ty;

pub(super) fn wty(t: Ty) -> &'static str {
    match t {
        Ty::Float => "f32",
        Ty::Vec2 => "vec2<f32>",
        Ty::Vec3 => "vec3<f32>",
        Ty::Vec4 => "vec4<f32>",
        Ty::Bool => "bool",
    }
}

pub(super) fn tag(t: Ty) -> &'static str {
    match t {
        Ty::Float | Ty::Bool => "f",
        Ty::Vec2 => "2",
        Ty::Vec3 => "3",
        Ty::Vec4 => "4",
    }
}

pub(super) fn splat(code: &str, from: Ty, to: Ty) -> String {
    if from == to || to == Ty::Float || to == Ty::Bool { code.to_owned() } else { format!("{}({code})", wty(to)) }
}

pub(super) fn float_lit(x: f32) -> String {
    format!("{x:?}").replace("inf", "3.4e38").replace("NaN", "0.0")
}

pub(super) const PRELUDE: &str = "struct Clock { dt: f32, t: f32, step: f32, seed: u32, count: u32, p0: u32, p1: u32, p2: u32 }
@group(0) @binding(0) var<uniform> clock: Clock;
@group(0) @binding(1) var<storage, read> params: array<f32>;
fn hash_u32(a: u32, b: u32, seed: u32) -> u32 {
  var x = (a * 0x9E3779B1u) ^ (b * 0x85EBCA6Bu) ^ (seed * 0xC2B2AE35u);
  x = x ^ (x >> 16u); x = x * 0x7FEB352Du; x = x ^ (x >> 15u); x = x * 0x846CA68Bu;
  return x ^ (x >> 16u);
}
fn to_u(x: f32) -> u32 { return u32(clamp(abs(x), 0.0, 4294967040.0)); }
fn closest_seg(p: vec3<f32>, a: vec3<f32>, b: vec3<f32>) -> vec3<f32> {
  let ab = b - a; let l2 = dot(ab, ab);
  if (l2 <= 0.0) { return a; }
  return a + ab * clamp(dot(p - a, ab) / l2, 0.0, 1.0);
}
fn pull_move(p: vec3<f32>, q: vec3<f32>, rest: f32) -> vec3<f32> {
  let d = q - p; let dist = max(length(d), 0.0001);
  return d * ((dist - rest) / dist);
}
fn ss_f(a: f32, b: f32, x: f32) -> f32 {
  if (a == b) { if (x < a) { return 0.0; } return 1.0; }
  let t = clamp((x - a) / (b - a), 0.0, 1.0); return t * t * (3.0 - 2.0 * t);
}
";

/// Carries a point (`w` = 1) or direction (`w` = 0) by the pose of bone input `b` (16 column-major f32s per input number).
pub(super) fn bone_helper() -> String {
    "fn bone_xf(b: u32, v: vec3<f32>, w: f32) -> vec3<f32> {
  let o = b * 16u;
  return vec3<f32>(Bn[o] * v.x + Bn[o + 4u] * v.y + Bn[o + 8u] * v.z + Bn[o + 12u] * w,
                   Bn[o + 1u] * v.x + Bn[o + 5u] * v.y + Bn[o + 9u] * v.z + Bn[o + 13u] * w,
                   Bn[o + 2u] * v.x + Bn[o + 6u] * v.y + Bn[o + 10u] * v.z + Bn[o + 14u] * w);
}
"
    .to_owned()
}

/// Reads the rest point `v` of mesh input `m` (see `MeshData::gpu_words` for the layout).
pub(super) fn mesh_helper(m: usize) -> String {
    format!(
        "fn rest{m}(v: u32) -> vec3<f32> {{
  let u = min(v, M{m}[0] - 1u);
  let o = M{m}[4] + 3u * u;
  return vec3<f32>(bitcast<f32>(M{m}[o]), bitcast<f32>(M{m}[o + 1u]), bitcast<f32>(M{m}[o + 2u]));
}}
"
    )
}

/// Per-type helpers (WGSL has no overloading): division and remainder that give 0 for a zero divisor, smoothstep, normalize.
pub(super) fn helpers() -> String {
    let mut s = String::new();
    for t in [Ty::Float, Ty::Vec2, Ty::Vec3, Ty::Vec4] {
        let (n, ty) = (tag(t), wty(t));
        let zero = if t == Ty::Float { "0.0".to_owned() } else { format!("{ty}(0.0)") };
        let _ = writeln!(s, "fn div_{n}(a: {ty}, b: {ty}) -> {ty} {{ return select(a / b, {zero}, b == {zero}); }}");
        let _ = writeln!(s, "fn rem_{n}(a: {ty}, b: {ty}) -> {ty} {{ return select(a - b * floor(a / b), {zero}, b == {zero}); }}");
        if t == Ty::Float {
            let _ = writeln!(s, "fn nrm_f(a: f32) -> f32 {{ return sign(a); }}");
        } else {
            let _ = writeln!(s, "fn nrm_{n}(a: {ty}) -> {ty} {{ let l = length(a); return select(a / l, {zero}, l <= 0.0); }}");
            let _ = writeln!(s, "fn ss_{n}(a: {ty}, b: {ty}, x: {ty}) -> {ty} {{ return {ty}({}); }}", (0..t.width()).map(|k| format!("ss_f(a[{k}], b[{k}], x[{k}])")).collect::<Vec<_>>().join(", "));
        }
    }
    s
}
