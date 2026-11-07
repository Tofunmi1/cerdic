//! `detail.panels`: a grid of plates laid on one face of a solid, some raised, some vented, the seams between them left open.
//!
//! Every plate and slat is a closed box; the boxes are joined by the kernel's union (slats start inside their plate so they fuse with it), so
//! the result is one valid solid that touches the face plane exactly. What each plate is depends only on the seed and the plate's number.

use ve_graph::{Node, Value as Param};
use ve_kernel::{ExecutionContext, KernelError, Solid};

/// Most boxes (plates and slats) one node may make.
const MAX_BOXES: usize = 4000;

fn mix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// A number in `[0, 1)` from the seed, a plate number and a purpose.
fn draw(seed: u64, plate: u64, purpose: u64) -> f64 {
    #[expect(clippy::cast_precision_loss, reason = "53 bits fit an f64 exactly")]
    let unit = (mix(seed ^ mix(plate * 8 + purpose)) >> 11) as f64 / (1_u64 << 53) as f64;
    unit
}

fn number(node: &Node, name: &str) -> Result<f64, KernelError> {
    node.params.get(name).and_then(Param::as_f64).ok_or(KernelError::InvalidArgument("a number parameter is missing"))
}

/// Joins any number of solids: batches of 64 through the kernel's batch union, then the results, until one is left.
pub(crate) fn union_many(mut solids: Vec<Solid>, context: &ExecutionContext) -> Result<Solid, KernelError> {
    while solids.len() > 1 {
        let mut next = Vec::with_capacity(solids.len() / 64 + 1);
        let mut rest = solids;
        while !rest.is_empty() {
            let take = rest.len().min(64);
            let mut batch: Vec<Solid> = rest.drain(..take).collect();
            if batch.len() == 1 {
                next.append(&mut batch);
            } else {
                let refs: Vec<&Solid> = batch.iter().collect();
                next.push(Solid::union_all_with_context(&refs, context)?);
            }
        }
        solids = next;
    }
    solids.pop().ok_or(KernelError::InvalidArgument("nothing to join"))
}

/// A box from `lo` to `hi` (corners in world axes).
pub(crate) fn cuboid(lo: [f64; 3], hi: [f64; 3]) -> Result<Solid, KernelError> {
    let size = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]];
    Solid::cube(size, true)?.translate([(lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5, (lo[2] + hi[2]) * 0.5])
}

/// The plates for `node` on the face of `base`.
pub(crate) fn build(node: &Node, base: &Solid, context: &ExecutionContext) -> Result<Solid, KernelError> {
    let face = match node.params.get("face") {
        Some(Param::Str(face)) => face.as_str(),
        _ => "+z",
    };
    let (axis, sign) = match face {
        "+x" => (0, 1.0),
        "-x" => (0, -1.0),
        "+y" => (1, 1.0),
        "-y" => (1, -1.0),
        "+z" => (2, 1.0),
        "-z" => (2, -1.0),
        _ => return Err(KernelError::InvalidArgument("face must be +x, -x, +y, -y, +z or -z")),
    };
    let (u_axis, v_axis) = match axis {
        0 => (2, 1),
        1 => (0, 2),
        _ => (0, 1),
    };
    let (cols, rows) = (number(node, "cols")?, number(node, "rows")?);
    let (gap, depth) = (number(node, "gap")?, number(node, "depth")?);
    let (raised, vents) = (number(node, "raised")?, number(node, "vents")?);
    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "the spec bounds the seed to 31 bits")]
    let seed = number(node, "seed")? as u64;
    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "the spec bounds both counts to 1..32")]
    let (cols, rows) = (cols as usize, rows as usize);
    if base.is_empty() {
        return Err(KernelError::InvalidArgument("detail.panels needs a solid with a face to cover"));
    }
    let bounds = base.bounds()?;
    let (u0, u1) = (bounds.min[u_axis], bounds.max[u_axis]);
    let (v0, v1) = (bounds.min[v_axis], bounds.max[v_axis]);
    let plane = if sign > 0.0 { bounds.max[axis] } else { bounds.min[axis] };
    #[expect(clippy::cast_precision_loss, reason = "at most 32 cells")]
    let (cell_u, cell_v) = ((u1 - u0) / cols as f64, (v1 - v0) / rows as f64);
    if gap >= cell_u.min(cell_v) - 1e-6 {
        return Err(KernelError::InvalidArgument("the gap leaves no plate: make it narrower than a plate"));
    }
    if cols * rows * 7 > MAX_BOXES * 2 {
        return Err(KernelError::InvalidArgument("too many plates for one node"));
    }
    // A box from `a` to `b` along the face's directions and from height `z0` to `z1` out of the face.
    let plate_box = |ua: f64, ub: f64, va: f64, vb: f64, z0: f64, z1: f64| -> Result<Solid, KernelError> {
        let (mut lo, mut hi) = ([0.0; 3], [0.0; 3]);
        lo[u_axis] = ua;
        hi[u_axis] = ub;
        lo[v_axis] = va;
        hi[v_axis] = vb;
        if sign > 0.0 {
            lo[axis] = plane + z0;
            hi[axis] = plane + z1;
        } else {
            lo[axis] = plane - z1;
            hi[axis] = plane - z0;
        }
        cuboid(lo, hi)
    };
    let mut boxes = Vec::new();
    for j in 0..rows {
        for i in 0..cols {
            let plate = (j * cols + i) as u64;
            #[expect(clippy::cast_precision_loss, reason = "at most 32 cells")]
            let (pu0, pu1, pv0, pv1) = (
                u0 + i as f64 * cell_u + gap * 0.5,
                u0 + (i + 1) as f64 * cell_u - gap * 0.5,
                v0 + j as f64 * cell_v + gap * 0.5,
                v0 + (j + 1) as f64 * cell_v - gap * 0.5,
            );
            let vented = draw(seed, plate, 1) < vents;
            let stands = !vented && draw(seed, plate, 2) < raised;
            let thick = if stands { depth * (1.5 + 1.5 * draw(seed, plate, 3)) } else { depth };
            boxes.push(plate_box(pu0, pu1, pv0, pv1, 0.0, thick)?);
            if vented {
                #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "a draw in [0, 1) times three")]
                let slats = 4 + (draw(seed, plate, 4) * 3.0) as usize;
                let (width, height) = (pu1 - pu0, pv1 - pv0);
                #[expect(clippy::cast_precision_loss, reason = "at most six slats")]
                for k in 0..slats {
                    let centre = pv0 + height * (k as f64 + 0.5) / slats as f64;
                    let half = height * 0.25 / slats as f64;
                    // a slat stands up to `depth` above the flat plate, starting inside the plate so the two fuse
                    let up = depth * (0.25 + 0.75 * draw(seed, plate, 5 + k as u64));
                    boxes.push(plate_box(pu0 + width * 0.1, pu1 - width * 0.1, centre - half, centre + half, thick * 0.5, thick + up)?);
                }
            }
            if boxes.len() > MAX_BOXES {
                return Err(KernelError::InvalidArgument("too many plates and slats for one node"));
            }
        }
    }
    union_many(boxes, context)
}
