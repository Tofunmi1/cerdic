//! `body.hair`: hair on a `body.human`, grown as thousands of tapered ribbon wisps from the scalp of that very head.
//!
//! The scalp is read from the head bone's own vertices (a radius for every direction from the middle of the skull), so hair sits on any head the
//! sliders make. Each wisp starts on the scalp inside a hairline that depends on the direction (a high forehead, temples, above the ear, the nape),
//! leaves it outward, falls under gravity, slides over the head instead of through it, and waves a little. Ribbons are flat strips facing out from
//! the head, drawn from both sides; wisps overlap, so a few thousand read as hair. It is cards, not strands: strands need compute the web renderer
//! does not have.
use std::rc::Rc;

use ve_graph::Node;
use ve_kernel::Mesh;

use crate::cache::Value;
use crate::human::Rig;
use crate::{Fail, fail, fail_suggest};

const SEGMENTS: usize = 8;
const THETA_BINS: usize = 24;
const PHI_BINS: usize = 48;

fn number(node: &Node, name: &str) -> Result<f32, Fail> {
    #[expect(clippy::cast_possible_truncation, reason = "hair parameters are small")]
    node.params
        .get(name)
        .and_then(ve_graph::Value::as_f64)
        .map(|v| v as f32)
        .ok_or_else(|| fail("param.type", format!("body.hair {name} must be a number")))
}

/// Wisps for a density from 0 to 1.
fn wisp_count(density: f32) -> usize {
    #[expect(clippy::cast_possible_truncation, reason = "bounded to a few thousand")]
    #[expect(clippy::cast_sign_loss, reason = "density is 0 to 1")]
    {
        (200.0 + 1800.0 * density.clamp(0.0, 1.0)) as usize
    }
}

/// Most triangles `body.hair` makes at full density (both sides of every segment of every wisp), for the cost estimate.
pub(crate) fn triangles(node: &Node) -> usize {
    let fine = number(node, "fine").unwrap_or(0.0).clamp(0.0, 1.0);
    let strands = 1 + (7.0 * fine).round() as usize;
    let cover = usize::from(matches!(
        node.params.get("cover"),
        Some(ve_graph::Value::Bool(true))
    ));
    let wisps = wisp_count(number(node, "density").unwrap_or(1.0));
    (wisps * if fine > 0.0 { 3 } else { 4 } / 4) * SEGMENTS * 4 * strands + cover * 8_000
}

struct Rng(u32);

impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        #[expect(clippy::cast_precision_loss, reason = "24 bits of a hash")]
        {
            (self.0 >> 8) as f32 / 16_777_216.0
        }
    }
}

fn add(a: [f32; 3], b: [f32; 3], s: f32) -> [f32; 3] {
    [a[0] + b[0] * s, a[1] + b[1] * s, a[2] + b[2] * s]
}
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn unit(a: [f32; 3]) -> [f32; 3] {
    let length = dot(a, a).sqrt();
    if length < 1e-9 {
        [0.0, 1.0, 0.0]
    } else {
        [a[0] / length, a[1] / length, a[2] / length]
    }
}

/// The head as a radius for every direction from `centre`, taken from the head bone's vertices.
struct Scalp {
    centre: [f32; 3],
    radius: Vec<f32>,
}

impl Scalp {
    fn bin(direction: [f32; 3]) -> (usize, usize) {
        let theta = direction[1].clamp(-1.0, 1.0).acos();
        let phi = direction[0].atan2(direction[2]);
        #[expect(clippy::cast_possible_truncation, reason = "bin indices")]
        #[expect(clippy::cast_sign_loss, reason = "angles are made non-negative first")]
        let (t, p) = (
            ((theta / std::f32::consts::PI * THETA_BINS as f32) as usize).min(THETA_BINS - 1),
            (((phi + std::f32::consts::PI) / std::f32::consts::TAU * PHI_BINS as f32) as usize)
                .min(PHI_BINS - 1),
        );
        (t, p)
    }

    fn new(points: &[[f32; 3]], eye_y: f32) -> Option<Self> {
        let cranium: Vec<&[f32; 3]> = points.iter().filter(|p| p[1] > eye_y + 0.01).collect();
        if cranium.len() < 50 {
            return None;
        }
        #[expect(clippy::cast_precision_loss, reason = "a few thousand points")]
        let n = cranium.len() as f32;
        let centre = [
            cranium.iter().map(|p| p[0]).sum::<f32>() / n,
            eye_y,
            cranium.iter().map(|p| p[2]).sum::<f32>() / n,
        ];
        let mut radius = vec![0.0_f32; THETA_BINS * PHI_BINS];
        for p in points {
            let offset = sub(*p, centre);
            let distance = dot(offset, offset).sqrt();
            if distance < 1e-4 {
                continue;
            }
            let (t, ph) = Self::bin(unit(offset));
            let slot = &mut radius[t * PHI_BINS + ph];
            *slot = slot.max(distance);
        }
        // Near the crown the bins are slivers around the pole and many hold no vertex: fill empty bins of the upper head from their neighbours (the
        // crown is the spot a scalp patch used to show through).
        for _ in 0..3 {
            let before = radius.clone();
            for t in 0..THETA_BINS * 3 / 5 {
                for p in 0..PHI_BINS {
                    if before[t * PHI_BINS + p] > 0.0 {
                        continue;
                    }
                    let (mut sum, mut count) = (0.0_f32, 0.0_f32);
                    for dt in [-1_isize, 0, 1] {
                        for dp in [-1_isize, 0, 1] {
                            let Some(tt) = t.checked_add_signed(dt).filter(|&v| v < THETA_BINS)
                            else {
                                continue;
                            };
                            let value = before
                                [tt * PHI_BINS + (p + PHI_BINS).wrapping_add_signed(dp) % PHI_BINS];
                            if value > 0.0 {
                                sum += value;
                                count += 1.0;
                            }
                        }
                    }
                    if count > 0.0 {
                        radius[t * PHI_BINS + p] = sum / count;
                    }
                }
            }
        }
        // A bin holds only the vertices that fell in it, and the skin bulges between vertices: take the largest of each bin's neighbours, so the
        // radius never dips under the surface.
        let mut dilated = radius.clone();
        for t in 0..THETA_BINS {
            for p in 0..PHI_BINS {
                let mut best = radius[t * PHI_BINS + p];
                for dt in [-1_isize, 0, 1] {
                    for dp in [-1_isize, 0, 1] {
                        let tt = t.checked_add_signed(dt).filter(|&v| v < THETA_BINS);
                        let pp = (p + PHI_BINS).wrapping_add_signed(dp) % PHI_BINS;
                        if let Some(tt) = tt {
                            best = best.max(radius[tt * PHI_BINS + pp]);
                        }
                    }
                }
                // Only grow where the bin had a surface at all, so the hollow below the jaw stays hollow.
                if radius[t * PHI_BINS + p] > 0.0 {
                    dilated[t * PHI_BINS + p] = best;
                }
            }
        }
        Some(Self {
            centre,
            radius: dilated,
        })
    }

    /// The scalp radius in this direction, or 0 where the head has no surface (below the jaw).
    fn at(&self, direction: [f32; 3]) -> f32 {
        let (t, p) = Self::bin(direction);
        self.radius[t * PHI_BINS + p]
    }
}

/// Where the hairline is, in metres above or below the eye line, for a direction around the head (0 front, pi back).
fn hairline(phi: f32, fringe: f32) -> f32 {
    let a = phi.abs().to_degrees();
    let table = [
        (0.0, 0.065 - 0.032 * fringe),
        (50.0, 0.050 - 0.012 * fringe),
        (80.0, 0.022),
        (110.0, 0.0),
        (150.0, -0.05),
        (180.0, -0.07),
    ];
    for pair in table.windows(2) {
        let ((a0, y0), (a1, y1)) = (pair[0], pair[1]);
        if a <= a1 {
            return y0 + (y1 - y0) * (a - a0) / (a1 - a0);
        }
    }
    -0.07
}

/// A thin skin of hair over the scalp inside the hairline, so the wisps never leave bald patches between them.
#[expect(
    clippy::too_many_arguments,
    reason = "appends to the three mesh buffers of the caller"
)]
fn add_cap(
    scalp: &Scalp,
    rig: &Rig,
    fringe: f32,
    eye_y: f32,
    volume: f32,
    positions: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    indices: &mut Vec<u32>,
) {
    const ROWS: usize = 28;
    const COLUMNS: usize = 72;
    let base = u32::try_from(positions.len()).unwrap_or(u32::MAX);
    let mut valid = vec![false; (ROWS + 1) * (COLUMNS + 1)];
    for i in 0..=ROWS {
        #[expect(clippy::cast_precision_loss, reason = "tiny counts")]
        let theta = std::f32::consts::PI * 0.75 * i as f32 / ROWS as f32;
        for j in 0..=COLUMNS {
            #[expect(clippy::cast_precision_loss, reason = "tiny counts")]
            let phi = -std::f32::consts::PI + std::f32::consts::TAU * j as f32 / COLUMNS as f32;
            let direction = [
                theta.sin() * phi.sin(),
                theta.cos(),
                theta.sin() * phi.cos(),
            ];
            let r = scalp.at(direction);
            let point = add(scalp.centre, direction, r + 0.004 + 0.006 * volume);
            let ear = phi.abs().to_degrees() > 70.0
                && phi.abs().to_degrees() < 115.0
                && point[1] < eye_y + 0.035;
            valid[i * (COLUMNS + 1) + j] =
                r > 0.0 && !ear && point[1] >= scalp.centre[1] + hairline(phi, fringe);
            positions.push(rig.world(point));
            // The cap shows root colour: v stays near 0 (a touch lighter towards the crown's edge), u runs round the head so a texture can streak it.
            uvs.push([
                (phi + std::f32::consts::PI) / std::f32::consts::TAU,
                0.06 * theta / (std::f32::consts::PI * 0.75),
            ]);
        }
    }
    let stride = u32::try_from(COLUMNS + 1).unwrap_or(0);
    let middle = rig.world(scalp.centre);
    for i in 0..ROWS {
        for j in 0..COLUMNS {
            let at = |di: usize, dj: usize| valid[(i + di) * (COLUMNS + 1) + j + dj];
            if at(0, 0) && at(0, 1) && at(1, 0) && at(1, 1) {
                let a = base + u32::try_from(i * (COLUMNS + 1) + j).unwrap_or(0);
                // Wind each triangle away from the middle of the head, so the cap shows from outside.
                for tri in [[a, a + stride, a + 1], [a + 1, a + stride, a + stride + 1]] {
                    let p = tri.map(|k| positions[k as usize]);
                    let outward = sub(p[0], middle);
                    if dot(cross(sub(p[1], p[0]), sub(p[2], p[0])), outward) >= 0.0 {
                        indices.extend_from_slice(&tri);
                    } else {
                        indices.extend_from_slice(&[tri[0], tri[2], tri[1]]);
                    }
                }
            }
        }
    }
}

pub(crate) fn build(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let input = inputs
        .first()
        .ok_or_else(|| fail("graph.missing_input", "body.hair needs a body.human"))?;
    let Some(rig) = input.human.as_ref() else {
        return Err(fail_suggest(
            "human.not_a_body",
            "body.hair needs the body.human node itself as its input",
            "connect the body.human node directly, before solid.material and transforms, and give body.hair its own output",
        ));
    };
    let (length, density, volume) = (
        number(node, "length")?,
        number(node, "density")?,
        number(node, "volume")?,
    );
    let (wave, fringe, width) = (
        number(node, "wave")?,
        number(node, "fringe")?,
        number(node, "width")?,
    );
    let fine = number(node, "fine").unwrap_or(0.0).clamp(0.0, 1.0);
    let strands = 1 + (7.0 * fine).round() as usize;
    let cover = matches!(node.params.get("cover"), Some(ve_graph::Value::Bool(true)));
    #[expect(clippy::cast_possible_truncation, reason = "a seed is a small integer")]
    #[expect(clippy::cast_sign_loss, reason = "a seed is not negative")]
    let seed = number(node, "seed")?.max(0.0) as u32;
    let eye_y = rig.eyes[0].0[1] - rig.at[1];
    let Some(scalp) = Scalp::new(&rig.head_points, eye_y) else {
        return Err(fail(
            "human.no_head",
            "this body has no head to grow hair on",
        ));
    };
    let mut rng = Rng(seed.wrapping_mul(2_654_435_761).wrapping_add(0x9E37_79B9) | 1);
    let base_count = wisp_count(density);
    let count = if fine > 0.0 {
        base_count * 3 / 4
    } else {
        base_count
    };
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let down = [0.0, -1.0, 0.0];
    let mut attempts = 0;
    let mut grown = 0;
    while grown < count && attempts < count * 6 {
        attempts += 1;
        // A direction on the upper head, evenly spread in area.
        let theta = (rng.next().sqrt() * 0.5 * std::f32::consts::PI * 1.25)
            .min(std::f32::consts::PI * 0.75);
        let phi = (rng.next() * 2.0 - 1.0) * std::f32::consts::PI;
        let direction = [
            theta.sin() * phi.sin(),
            theta.cos(),
            theta.sin() * phi.cos(),
        ];
        let r = scalp.at(direction);
        if r <= 0.0 {
            continue;
        }
        let root = add(scalp.centre, direction, r);
        if root[1] < scalp.centre[1] + hairline(phi, fringe) {
            continue;
        }
        // Ears are not scalp: skip the side directions at ear height.
        if phi.abs().to_degrees() > 70.0
            && phi.abs().to_degrees() < 115.0
            && root[1] < eye_y + 0.035
            && r > 0.0
        {
            continue;
        }
        grown += 1;
        // Hair over the forehead stops above the brows; everywhere else it may hang its full length.
        let forehead = 1.0 - ((phi.abs().to_degrees() - 40.0) / 25.0).clamp(0.0, 1.0);
        let free = length * (0.85 + 0.3 * rng.next());
        let wisp_length = if forehead > 0.0 {
            let room = ((root[1] - (eye_y + 0.03)) * 1.15).max(0.012);
            free + (room.min(free) - free) * forehead
        } else {
            free
        };
        let step = wisp_length / SEGMENTS as f32;
        let phase = rng.next() * std::f32::consts::TAU;
        let side = rng.next() * 2.0 - 1.0;
        let w0 = width * (0.7 + 0.6 * rng.next());
        // Top hair is combed back and down, the fringe forward and down, the rest straight down.
        let top = (direction[1] - 0.6).max(0.0) / 0.4;
        let front = (direction[2] - 0.2).max(0.0);
        // Over the front the hair sweeps sideways and back, away from the face, instead of dropping straight down over it.
        let sweep = [
            (side + direction[0] * 2.0).clamp(-1.0, 1.0) * 0.6 * front,
            0.0,
            -0.9 * front,
        ];
        let mut heading = unit(add(
            add(
                add(sweep, direction, 0.45 + volume),
                down,
                0.9 - 0.5 * front,
            ),
            [side * 0.25, 0.0, -0.6 * top],
            1.0,
        ));
        let mut at = add(root, direction, 0.0015 + 0.012 * volume);
        let mut points = vec![at];
        for i in 0..SEGMENTS {
            let outward = unit(sub(at, scalp.centre));
            let lateral = unit(cross(heading, outward));
            #[expect(clippy::cast_precision_loss, reason = "a handful of segments")]
            let wobble = (phase + i as f32 * 1.3).sin() * wave * 0.5;
            heading = unit(add(add(heading, down, 0.22), lateral, wobble));
            at = add(at, heading, step);
            // Slide over the head instead of through it.
            let offset = sub(at, scalp.centre);
            let distance = dot(offset, offset).sqrt();
            let facing = unit(offset);
            let surface = scalp.at(facing);
            if surface > 0.0 && distance < surface + 0.002 + 0.01 * volume {
                at = add(scalp.centre, facing, surface + 0.002 + 0.01 * volume);
                let inward = dot(heading, facing).min(0.0);
                heading = unit(add(heading, facing, -inward));
            }
            points.push(at);
        }
        let base = u32::try_from(positions.len()).unwrap_or(u32::MAX);
        for strand in 0..strands {
            let strand_base = base + u32::try_from(strand * 2 * (SEGMENTS + 1)).unwrap_or(0);
            let lateral_shift = if strands == 1 {
                0.0
            } else {
                #[expect(clippy::cast_precision_loss, reason = "at most eight strands")]
                {
                    (strand as f32 / (strands - 1) as f32 - 0.5) * width * 0.72
                }
            };
            for back in [false, true] {
                for (i, &p) in points.iter().enumerate() {
                    let tangent = unit(sub(
                        points[(i + 1).min(SEGMENTS)],
                        points[i.saturating_sub(1)],
                    ));
                    let outward = unit(sub(p, scalp.centre));
                    let across = unit(cross(tangent, outward));
                    #[expect(clippy::cast_precision_loss, reason = "a handful of segments")]
                    let t = i as f32 / SEGMENTS as f32;
                    let strand_width = if fine > 0.0 {
                        0.0025 + (1.0 - fine) * (w0 - 0.0025).max(0.0)
                    } else {
                        w0
                    };
                    let half = 0.5 * strand_width * (1.0 - 0.9 * t.powf(1.4));
                    for (sign, u) in [(-1.0, 0.0), (1.0, 1.0)] {
                        let world =
                            rig.world(add(add(p, across, sign * half), across, lateral_shift));
                        positions.push(world);
                        // u names the wisp (a hash of its number) with a hair's width of spread across it, so a texture varies the colour wisp to wisp
                        // and strand to strand; v runs 0 at the root to 1 at the tip.
                        #[expect(
                            clippy::cast_precision_loss,
                            reason = "a wisp number is a few thousand"
                        )]
                        let name = ((grown as f32 * 12.989_8).sin() * 43_758.547).fract().abs();
                        uvs.push([name * 0.96 + 0.04 * u, t]);
                    }
                }
                let half_base = strand_base
                    + if back {
                        u32::try_from(2 * (SEGMENTS + 1)).unwrap_or(0)
                    } else {
                        0
                    };
                for i in 0..SEGMENTS {
                    let a = half_base + u32::try_from(2 * i).unwrap_or(0);
                    let quad = [a, a + 1, a + 3, a + 2];
                    if back {
                        indices.extend_from_slice(&[
                            quad[0], quad[2], quad[1], quad[0], quad[3], quad[2],
                        ]);
                    } else {
                        indices.extend_from_slice(&[
                            quad[0], quad[1], quad[2], quad[0], quad[2], quad[3],
                        ]);
                    }
                }
            }
        }
    }
    if grown == 0 {
        return Err(fail("human.no_hair", "no scalp was found to grow hair on"));
    }
    add_cap(
        &scalp,
        rig,
        fringe,
        eye_y,
        volume,
        &mut positions,
        &mut uvs,
        &mut indices,
    );
    if cover {
        add_cap(
            &scalp,
            rig,
            fringe,
            eye_y,
            volume + 0.25,
            &mut positions,
            &mut uvs,
            &mut indices,
        );
    }
    let corner_uvs: Vec<[f32; 2]> = indices.iter().map(|&i| uvs[i as usize]).collect();
    let mut value = Value::solid(Mesh { positions, indices }, 0.0);
    value.uvs = corner_uvs;
    Ok(value)
}
