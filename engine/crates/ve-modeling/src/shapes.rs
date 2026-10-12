//! The small shape operators: rect and circle profiles, cone, torus and capsule solids, and the
//! patterns (mirror, array, radial, repeat) that turn one solid into the union of several copies.
use crate::cache::Value;
use crate::operations::vector;
use crate::profile_ops::{number, pair, whole};
use std::f64::consts::{FRAC_PI_2, TAU};
use std::rc::Rc;
use ve_graph::Node;
use ve_kernel::{ExecutionContext, KernelError, Profile, Solid};

type Loops = Vec<Vec<[f64; 2]>>;

/// `profile.rect` and `profile.circle`, centred on the origin.
pub(crate) fn profile(node: &Node) -> Result<Loops, KernelError> {
    if node.op.family().as_str() == "profile.rect" {
        let [width, height] = pair(node, "size")?;
        let (x, y) = (width / 2.0, height / 2.0);
        return Profile::polygon(&[[-x, -y], [x, -y], [x, y], [-x, y]])?.loops();
    }
    let (radius, count) = (number(node, "radius")?, whole(node, "segments")?);
    Profile::polygon(&circle(0.0, radius, count))?.loops()
}

/// `count` points evenly spaced on a circle in the XY plane, counter-clockwise.
fn circle(centre_x: f64, radius: f64, count: u32) -> Vec<[f64; 2]> {
    (0..count)
        .map(|i| {
            let angle = TAU * f64::from(i) / f64::from(count);
            [centre_x + radius * angle.cos(), radius * angle.sin()]
        })
        .collect()
}

/// `solid.cone`, `solid.torus` and `solid.capsule`.
pub(crate) fn primitive(node: &Node) -> Result<Solid, KernelError> {
    let segments = whole(node, "segments")?;
    match node.op.family().as_str() {
        "solid.cone" => Solid::cylinder(
            number(node, "height")?,
            number(node, "radius")?,
            number(node, "top_radius")?,
            segments,
            true,
        ),
        "solid.torus" => {
            let (major, minor) = (number(node, "major")?, number(node, "minor")?);
            if minor >= major {
                return Err(KernelError::InvalidArgument(
                    "the tube of a torus must be thinner than its ring",
                ));
            }
            let tube = circle(major, minor, whole(node, "tube_segments")?);
            Profile::polygon(&tube)?.revolve(segments, TAU)
        }
        _ => capsule(number(node, "radius")?, number(node, "height")?, segments),
    }
}

/// A cylinder with a half sphere on each end, revolved from one profile so the surface is closed.
fn capsule(radius: f64, height: f64, segments: u32) -> Result<Solid, KernelError> {
    if height < 2.0 * radius {
        return Err(KernelError::InvalidArgument(
            "a capsule is at least as long as it is wide",
        ));
    }
    let straight = height / 2.0 - radius;
    let steps = (segments / 4).max(2);
    // A quarter circle of the cap: `from` is where the sweep starts, `y` the height of its centre.
    let arc = |from: f64, y: f64| {
        (0..=steps).map(move |i| {
            let angle = from + FRAC_PI_2 * f64::from(i) / f64::from(steps);
            [radius * angle.sin(), y - radius * angle.cos()]
        })
    };
    // Bottom pole, up the lower half sphere, up the side, over the upper half sphere to the top pole.
    let mut points: Vec<[f64; 2]> = arc(0.0, -straight).collect();
    points.extend(arc(FRAC_PI_2, straight).skip(usize::from(straight <= 0.0)));
    Profile::polygon(&points)?.revolve(segments, TAU)
}

/// `xform.mirror`, `xform.array`, `xform.radial` and `xform.repeat`: the union of several copies.
pub(crate) fn pattern(
    node: &Node,
    input: &Rc<Value>,
    context: &ExecutionContext,
) -> Result<Solid, KernelError> {
    let original = Solid::from_mesh(&input.mesh)?;
    let mut copies = Vec::new();
    // The solid the copies are made from: the input itself, or for a radial pattern the input moved out.
    let mut base = None;
    match node.op.family().as_str() {
        "xform.mirror" => {
            let mut flip = [1.0; 3];
            flip[axis(node)?] = -1.0;
            copies.push(original.scale(flip)?);
        }
        "xform.array" => {
            let spacing = vector(node, "spacing")?;
            let (nx, ny, nz) = (
                whole(node, "count_x")?,
                whole(node, "count_y")?,
                whole(node, "count_z")?,
            );
            for i in 0..nx {
                for j in 0..ny {
                    for k in 0..nz {
                        if (i, j, k) != (0, 0, 0) {
                            let at = [i, j, k].map(f64::from);
                            copies.push(original.translate([
                                at[0] * spacing[0],
                                at[1] * spacing[1],
                                at[2] * spacing[2],
                            ])?);
                        }
                    }
                }
            }
        }
        "xform.radial" => {
            let (count, radius) = (whole(node, "count")?, number(node, "radius")?);
            base = Some(original.translate([radius, 0.0, 0.0])?);
            let out = base.as_ref().unwrap_or(&original);
            for i in 1..count {
                let turn = 360.0 * f64::from(i) / f64::from(count);
                copies.push(out.rotate([0.0, turn, 0.0])?);
            }
        }
        _ => {
            let (step, turn) = (vector(node, "step")?, vector(node, "rotate_step")?);
            for i in 1..whole(node, "count")? {
                let n = f64::from(i);
                let turned = original.rotate(turn.map(|t| (t * n).to_degrees()))?;
                copies.push(turned.translate(step.map(|s| s * n))?);
            }
        }
    }
    // Only a 1 x 1 x 1 array has no copies (radial and repeat need at least two).
    if copies.is_empty() {
        return Ok(original);
    }
    let first = base.as_ref().unwrap_or(&original);
    let all: Vec<&Solid> = std::iter::once(first).chain(copies.iter()).collect();
    Solid::union_all_with_context(&all, context)
}

fn axis(node: &Node) -> Result<usize, KernelError> {
    match node.params.get("axis") {
        Some(ve_graph::Value::Str(a)) if a == "x" => Ok(0),
        Some(ve_graph::Value::Str(a)) if a == "y" => Ok(1),
        Some(ve_graph::Value::Str(a)) if a == "z" => Ok(2),
        _ => Err(KernelError::InvalidArgument("axis is x, y or z")),
    }
}
