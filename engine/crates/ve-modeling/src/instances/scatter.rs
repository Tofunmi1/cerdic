//! Seeded scatter and per-instance variation: deterministic, bounded and independent of platform maths.
//!
//! Randomness comes from one `SplitMix64` stream per node (scatter) or per element (jitter), so the same
//! authored seed gives the same placements on every platform and re-evaluation. Only `+ - * /` and the
//! rotation matrix are used on the random values.
use super::InstanceSet;
use crate::{
    Fail, fail,
    operations::vector,
    profile_ops::{number, whole},
};
use glam::{DMat4, DVec3};
use std::collections::HashMap;
use ve_graph::Node;

/// The most placements one scatter node may produce.
pub(crate) const MAX_SCATTER: u32 = 16_384;
/// Random draws allowed per requested placement before the spacing is declared unsatisfiable.
const ATTEMPTS_PER_POINT: u32 = 32;

/// `SplitMix64`: tiny, fast and bit-identical everywhere.
struct Stream(u64);

impl Stream {
    fn new(seed: u64, salt: u64) -> Self {
        let mut stream = Self(seed ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        stream.next_u64();
        stream
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `[-1, 1)`.
    fn signed(&mut self) -> f64 {
        #[expect(
            clippy::cast_precision_loss,
            reason = "53 random bits fit an f64 mantissa exactly"
        )]
        let unit = (self.next_u64() >> 11) as f64 / 9_007_199_254_740_992.0;
        unit * 2.0 - 1.0
    }
}

fn seed(node: &Node) -> Result<u64, Fail> {
    Ok(u64::from(whole(node, "seed").map_err(|_| {
        fail("graph.invalid_param", "seed must be a whole number")
    })?))
}

/// Yaw about Y and uniform scale drawn within the authored jitter, as a local matrix.
fn local_variation(stream: &mut Stream, yaw: f64, scale: f64) -> DMat4 {
    let angle = stream.signed() * yaw;
    let factor = 1.0 + stream.signed() * scale;
    DMat4::from_rotation_y(angle) * DMat4::from_scale(DVec3::splat(factor))
}

/// Scattered placements over a `width` x `depth` rectangle centred on the origin in the XZ plane.
///
/// Points closer than `min_spacing` are redrawn (dart throwing over a hash grid). If `count` points
/// cannot be placed within the attempt budget the node refuses instead of returning fewer.
pub(crate) fn scatter(node: &Node) -> Result<Vec<DMat4>, Fail> {
    let count = whole(node, "count").map_err(|_| fail("graph.invalid_param", "count"))?;
    if count == 0 || count > MAX_SCATTER {
        return Err(fail(
            "budget.instances",
            format!("scatter count must be 1..={MAX_SCATTER}"),
        ));
    }
    let measure = |name| number(node, name).map_err(|_| fail("graph.invalid_param", name));
    let (width, depth) = (measure("width")?, measure("depth")?);
    let spacing = measure("min_spacing")?;
    let (yaw, scale) = (measure("yaw_jitter")?, measure("scale_jitter")?);
    let mut stream = Stream::new(seed(node)?, 1);
    let mut points: Vec<(f64, f64)> = Vec::new();
    let mut grid: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    let cell = spacing.max(1.0e-9);
    let budget = count.saturating_mul(ATTEMPTS_PER_POINT).saturating_add(64);
    let mut transforms = Vec::new();
    let mut attempts = 0;
    while points.len() < count as usize {
        attempts += 1;
        if attempts > budget {
            return Err(Box::new(
                ve_graph::Diagnostic::error(
                    "scatter.unsatisfiable",
                    format!(
                        "placed {} of {count} points with min_spacing {spacing}; enlarge the area, lower the spacing or the count",
                        points.len()
                    ),
                )
                .at_node(&node.id)
                .suggest("enlarge `width` and `depth`, or lower `min_spacing` or `count`"),
            ));
        }
        let candidate = (stream.signed() * width * 0.5, stream.signed() * depth * 0.5);
        let key = |v: f64| {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "coordinates are bounded by the parameter extent"
            )]
            let cell_index = (v / cell).floor() as i64;
            cell_index
        };
        let (cx, cz) = (key(candidate.0), key(candidate.1));
        let crowded = spacing > 0.0
            && (cx - 1..=cx + 1).any(|x| {
                (cz - 1..=cz + 1).any(|z| {
                    grid.get(&(x, z)).is_some_and(|near| {
                        near.iter().any(|&i| {
                            let (dx, dz) = (points[i].0 - candidate.0, points[i].1 - candidate.1);
                            dx * dx + dz * dz < spacing * spacing
                        })
                    })
                })
            });
        if crowded {
            continue;
        }
        grid.entry((cx, cz)).or_default().push(points.len());
        points.push(candidate);
        transforms.push(
            DMat4::from_translation(DVec3::new(candidate.0, 0.0, candidate.1))
                * local_variation(&mut stream, yaw, scale),
        );
    }
    Ok(transforms)
}

/// Varies every existing placement by a bounded random shift, yaw and uniform scale.
///
/// Each element draws from its own stream keyed by its index, so the variation of element `i` does not
/// depend on how many others exist.
pub(crate) fn jitter(node: &Node, base: &InstanceSet) -> Result<Vec<DMat4>, Fail> {
    let shift = vector(node, "shift").map_err(|_| fail("graph.invalid_param", "shift"))?;
    let measure = |name| number(node, name).map_err(|_| fail("graph.invalid_param", name));
    let (yaw, scale) = (measure("yaw_jitter")?, measure("scale_jitter")?);
    let seed = seed(node)?;
    Ok(base
        .transforms
        .iter()
        .enumerate()
        .map(|(index, matrix)| {
            let mut stream = Stream::new(seed, 2 + index as u64);
            let offset = DVec3::new(
                stream.signed() * shift[0],
                stream.signed() * shift[1],
                stream.signed() * shift[2],
            );
            DMat4::from_translation(offset) * *matrix * local_variation(&mut stream, yaw, scale)
        })
        .collect())
}
