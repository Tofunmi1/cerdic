//! The arithmetic and functions of SimL, on typed values. Everything is finite in and finite out: a NaN or infinity becomes 0.
#![expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "hash inputs are small non-negative numbers carried as f32 in the language"
)]
use super::ast::{Bin, Func};
use super::{Ty, Value};
use crate::expr::math;

pub(super) fn finite(x: f32) -> f32 {
    if x.is_finite() { x } else { 0.0 }
}

pub(super) fn zip(a: Value, b: Value, ty: Ty, f: impl Fn(f32, f32) -> f32) -> Value {
    let mut v = [0.0; 4];
    for (k, out) in v.iter_mut().enumerate().take(ty.width()) {
        let x = if a.ty == Ty::Float { a.v[0] } else { a.v[k] };
        let y = if b.ty == Ty::Float { b.v[0] } else { b.v[k] };
        *out = finite(f(x, y));
    }
    Value { ty, v }
}

pub(super) fn map(a: Value, f: impl Fn(f32) -> f32) -> Value {
    let mut v = [0.0; 4];
    for (k, out) in v.iter_mut().enumerate().take(a.ty.width()) {
        *out = finite(f(a.v[k]));
    }
    Value { ty: a.ty, v }
}

fn dot(a: Value, b: Value) -> f32 {
    (0..a.ty.width()).fold(0.0, |s, k| s + a.v[k] * b.v[k])
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    if a == b {
        return if x < a { 0.0 } else { 1.0 };
    }
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn hash_u32(a: u32, b: u32, seed: u32) -> u32 {
    let mut x = a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA6B) ^ seed.wrapping_mul(0xC2B2_AE35);
    x ^= x >> 16;
    x = x.wrapping_mul(0x7FEB_352D);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846C_A68B);
    x ^ (x >> 16)
}

pub(super) fn arith(op: Bin, a: Value, b: Value, ty: Ty) -> Value {
    zip(a, b, ty, |x, y| match op {
        Bin::Add => x + y,
        Bin::Sub => x - y,
        Bin::Mul => x * y,
        Bin::Div => {
            if y == 0.0 {
                0.0
            } else {
                x / y
            }
        }
        _ => {
            if y == 0.0 {
                0.0
            } else {
                x - y * (x / y).floor()
            }
        }
    })
}

fn vec3(v: [f32; 3]) -> Value {
    Value { ty: Ty::Vec3, v: [v[0], v[1], v[2], 0.0] }
}

/// The point of segment `a`-`b` nearest to `p`.
fn closest_on_segment(p: Value, a: Value, b: Value) -> [f32; 3] {
    let ab = [b.v[0] - a.v[0], b.v[1] - a.v[1], b.v[2] - a.v[2]];
    let len2 = ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2];
    let t = if len2 > 0.0 { (((p.v[0] - a.v[0]) * ab[0] + (p.v[1] - a.v[1]) * ab[1] + (p.v[2] - a.v[2]) * ab[2]) / len2).clamp(0.0, 1.0) } else { 0.0 };
    [a.v[0] + ab[0] * t, a.v[1] + ab[1] * t, a.v[2] + ab[2] * t]
}

pub(super) fn call(f: Func, a: &[Value], ty: Ty, seed: u32) -> Value {
    match f {
        Func::Abs => map(a[0], f32::abs),
        Func::Sign => map(a[0], |x| if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { 0.0 }),
        Func::Floor => map(a[0], f32::floor),
        Func::Ceil => map(a[0], f32::ceil),
        Func::Fract => map(a[0], |x| x - x.floor()),
        Func::Sqrt => map(a[0], |x| if x > 0.0 { x.sqrt() } else { 0.0 }),
        Func::Exp => map(a[0], math::exp),
        Func::Log => map(a[0], math::log),
        Func::Sin => map(a[0], math::sin),
        Func::Cos => map(a[0], math::cos),
        Func::Min => zip(a[0], a[1], ty, f32::min),
        Func::Max => zip(a[0], a[1], ty, f32::max),
        Func::Pow => zip(a[0], a[1], ty, math::pow),
        Func::Atan2 => zip(a[0], a[1], ty, math::atan2),
        Func::Step => zip(a[0], a[1], ty, |edge, x| if x < edge { 0.0 } else { 1.0 }),
        Func::Clamp => zip(zip(a[0], a[1], ty, f32::max), a[2], ty, f32::min),
        Func::Mix => {
            let delta = zip(a[1], a[0], ty, |x, y| x - y);
            let scaled = zip(delta, a[2], ty, |x, y| x * y);
            zip(a[0], scaled, ty, |x, y| x + y)
        }
        Func::Smoothstep => {
            let lo = zip(a[0], a[2], ty, |l, _| l);
            let hi = zip(a[1], a[2], ty, |h, _| h);
            let mut v = [0.0; 4];
            for (k, out) in v.iter_mut().enumerate().take(ty.width()) {
                *out = smoothstep(lo.v[k], hi.v[k], if a[2].ty == Ty::Float { a[2].v[0] } else { a[2].v[k] });
            }
            Value { ty, v }
        }
        Func::Length => Value::float(finite(dot(a[0], a[0]).sqrt())),
        Func::Distance => {
            let d = zip(a[0], a[1], a[0].ty, |x, y| x - y);
            Value::float(finite(dot(d, d).sqrt()))
        }
        Func::Dot => Value::float(finite(dot(a[0], a[1]))),
        Func::Cross => {
            let (p, q) = (a[0].v, a[1].v);
            vec3([p[1] * q[2] - p[2] * q[1], p[2] * q[0] - p[0] * q[2], p[0] * q[1] - p[1] * q[0]])
        }
        Func::Normalize => {
            let len = dot(a[0], a[0]).sqrt();
            if len > 0.0 { map(a[0], |x| x / len) } else { map(a[0], |_| 0.0) }
        }
        Func::Select => {
            if a[0].v[0] != 0.0 { a[1] } else { a[2] }
        }
        Func::Hash | Func::Rand => {
            let h = hash_u32(a[0].v[0].abs() as u32, a[1].v[0].abs() as u32, seed);
            if f == Func::Hash { Value::float((h >> 8) as f32) } else { Value::float((h >> 8) as f32 / 16_777_216.0) }
        }
        Func::ClosestSegment => vec3(closest_on_segment(a[0], a[1], a[2])),
        Func::Pull => {
            // the move of `p` that gives the distance-constraint between p and q its rest length: d * (dist - rest) / dist
            let d = [a[1].v[0] - a[0].v[0], a[1].v[1] - a[0].v[1], a[1].v[2] - a[0].v[2]];
            let dist = finite((d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()).max(0.0001);
            let k = (dist - a[2].v[0]) / dist;
            vec3([finite(d[0] * k), finite(d[1] * k), finite(d[2] * k)])
        }
        Func::SdfCapsule => {
            let c = closest_on_segment(a[0], a[1], a[2]);
            let d = [a[0].v[0] - c[0], a[0].v[1] - c[1], a[0].v[2] - c[2]];
            Value::float(finite((d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() - a[3].v[0]))
        }
    }
}
