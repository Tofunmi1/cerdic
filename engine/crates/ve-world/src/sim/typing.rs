//! Type rules of SimL: what operators and functions accept and return, swizzles, and the "did you mean" search.
use super::ast::{Bin, Expr, Func};
use super::Ty;

pub(super) fn edit_distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = (above + 1).min(row[j] + 1).min(diagonal + usize::from(ca != cb));
            diagonal = above;
        }
    }
    row[b.len()]
}

/// The option closest to `name` when it is near enough to be a typo.
pub(super) fn closest<'a>(name: &str, options: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    let limit = (name.len() / 3).max(2);
    options.map(|o| (edit_distance(name, o), o)).filter(|(d, _)| *d <= limit).min_by_key(|(d, _)| *d).map(|(_, o)| o)
}

pub(super) fn swizzle_indices(s: &str) -> Option<Vec<usize>> {
    if s.is_empty() || s.len() > 4 {
        return None;
    }
    s.chars().map(|c| "xyzw".find(c).or_else(|| "rgba".find(c))).collect()
}

pub(super) fn binary_type(op: Bin, a: Ty, b: Ty) -> Option<Ty> {
    match op {
        Bin::Or | Bin::And => (a == Ty::Bool && b == Ty::Bool).then_some(Ty::Bool),
        Bin::Eq | Bin::Ne => (a == b && (a == Ty::Float || a == Ty::Bool)).then_some(Ty::Bool),
        Bin::Lt | Bin::Le | Bin::Gt | Bin::Ge => (a == Ty::Float && b == Ty::Float).then_some(Ty::Bool),
        _ => {
            if a == Ty::Bool || b == Ty::Bool {
                None
            } else if a == b || b == Ty::Float {
                Some(a)
            } else if a == Ty::Float {
                Some(b)
            } else {
                None
            }
        }
    }
}

/// The widest of numeric arguments, where a lone number mixes with any vector.
fn numeric_common(args: &[Expr]) -> Result<Ty, String> {
    let mut ty = Ty::Float;
    for a in args {
        if a.ty == Ty::Bool {
            return Err("takes numbers, not bools".to_owned());
        }
        if a.ty != Ty::Float {
            if ty != Ty::Float && ty != a.ty {
                return Err("mixes vectors of different sizes".to_owned());
            }
            ty = a.ty;
        }
    }
    Ok(ty)
}

pub(super) fn func_type(f: Func, args: &[Expr]) -> Result<Ty, String> {
    let want = |n: usize| {
        if args.len() == n { Ok(()) } else { Err(format!("takes {n} arguments, got {}", args.len())) }
    };
    match f {
        Func::Abs | Func::Sign | Func::Floor | Func::Ceil | Func::Fract | Func::Sqrt | Func::Exp | Func::Log | Func::Sin | Func::Cos | Func::Normalize => {
            want(1)?;
            numeric_common(args)
        }
        Func::Min | Func::Max | Func::Pow | Func::Atan2 | Func::Step => {
            want(2)?;
            numeric_common(args)
        }
        Func::Clamp | Func::Mix | Func::Smoothstep => {
            want(3)?;
            numeric_common(args)
        }
        Func::Length => {
            want(1)?;
            numeric_common(args).map(|_| Ty::Float)
        }
        Func::Distance | Func::Dot => {
            want(2)?;
            if args[0].ty != args[1].ty || args[0].ty == Ty::Bool {
                return Err("takes two vectors of the same size".to_owned());
            }
            Ok(Ty::Float)
        }
        Func::Cross => {
            want(2)?;
            if args[0].ty != Ty::Vec3 || args[1].ty != Ty::Vec3 {
                return Err("takes two vec3".to_owned());
            }
            Ok(Ty::Vec3)
        }
        Func::Select => {
            want(3)?;
            if args[0].ty != Ty::Bool || args[1].ty != args[2].ty {
                return Err("select(condition: bool, a, b) with a and b of one type".to_owned());
            }
            Ok(args[1].ty)
        }
        Func::SdfCapsule => {
            want(4)?;
            if args[..3].iter().any(|a| a.ty != Ty::Vec3) || args[3].ty != Ty::Float {
                return Err("sdf_capsule(point: vec3, a: vec3, b: vec3, radius: f32)".to_owned());
            }
            Ok(Ty::Float)
        }
        Func::Pull => {
            want(3)?;
            if args[0].ty != Ty::Vec3 || args[1].ty != Ty::Vec3 || args[2].ty != Ty::Float {
                return Err("pull(p: vec3, q: vec3, rest: f32)".to_owned());
            }
            Ok(Ty::Vec3)
        }
        Func::ClosestSegment => {
            want(3)?;
            if args.iter().any(|a| a.ty != Ty::Vec3) {
                return Err("closest_segment(point: vec3, a: vec3, b: vec3)".to_owned());
            }
            Ok(Ty::Vec3)
        }
        Func::Hash | Func::Rand => {
            want(2)?;
            if args.iter().any(|a| a.ty != Ty::Float) {
                return Err("takes two numbers".to_owned());
            }
            Ok(Ty::Float)
        }
    }
}
