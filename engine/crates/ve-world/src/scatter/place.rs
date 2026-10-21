//! Placement and shaping of point sets.
use super::{Field2, PointSet, ScatterError};
use random::{cell_hash, id_value, lerp, mix64, unit};
pub(in crate::scatter) mod random;

/// Locally stable blue-noise placement settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Poisson {
    /// Seed.
    pub seed: u32,
    /// Minimum spacing in metres at density 1.
    pub radius: f32,
    /// Overall density 0..=1.
    pub density: f32,
    /// World rectangle `[min_x, min_z, max_x, max_z]`, half-open.
    pub area: [f32; 4],
    /// Candidates tried per cell.
    pub tries: u32,
}

/// Standard per-point variation written by [`vary`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Variation {
    /// Seed.
    pub seed: u32,
    /// Range of the `scale` channel.
    pub scale: [f32; 2],
    /// Value written to the `id` channel.
    pub species: u32,
    /// Range of the `age` channel.
    pub age: [f32; 2],
}

const MAX_POINTS: usize = 1_000_000;

/// Blue-noise points over the rectangle, optionally sampled from a mask and height field.
///
/// # Errors
/// Returns `Cancelled`, `TooMany`, or `BadParameter` for invalid dimensions.
#[expect(
    clippy::too_many_lines,
    reason = "placement stages validation, global grid evaluation, and output clipping"
)]
pub fn poisson(
    params: &Poisson,
    mask: Option<&Field2<'_>>,
    heights: Option<&Field2<'_>>,
    cancel: &dyn Fn() -> bool,
) -> Result<PointSet, ScatterError> {
    validate_area(params.radius, params.area)?;
    if !params.density.is_finite() || params.density < 0.0 {
        return Err(bad_parameter("density", "must be finite and non-negative"));
    }
    let cell = params.radius / std::f32::consts::SQRT_2;
    let bounds = cell_bounds(canonical_area(params.area), cell)?;
    let cell_count = bounds_count(bounds)?;
    if cell_count > MAX_POINTS.saturating_mul(64) {
        return Err(ScatterError::TooMany {
            requested: cell_count,
            max: MAX_POINTS,
        });
    }
    let mut cells = Vec::with_capacity(cell_count);
    for z in bounds.2..=bounds.3 {
        if cancel() {
            return Err(ScatterError::Cancelled);
        }
        for x in bounds.0..=bounds.1 {
            cells.push((cell_hash(params.seed, x, z, 0), x, z));
        }
    }
    cells.sort_unstable();
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(cell_count / 3);
    let mut point_keys = Vec::new();
    let mut grid: Vec<Option<usize>> = vec![None; cell_count];
    let grid_width = usize::try_from(i64::from(bounds.1) - i64::from(bounds.0) + 1)
        .map_err(|_| bad_parameter("area", "grid is too large"))?;
    let tries = params.tries.max(1);
    for (_, cx, cz) in cells {
        let mut chosen = None;
        for attempt in 0..tries {
            let key = cell_hash(params.seed, cx, cz, attempt);
            let x = (crate::num::lattice_f32(cx) + unit(key)) * cell;
            let z = (crate::num::lattice_f32(cz) + unit(mix64(key)) * 0.999_999) * cell;
            let spacing_squared = params.radius * params.radius;
            let mut clear = true;
            for dz in -2..=2 {
                for dx in -2..=2 {
                    let Some(nx) = cx.checked_add(dx) else {
                        continue;
                    };
                    let Some(nz) = cz.checked_add(dz) else {
                        continue;
                    };
                    if nx < bounds.0 || nx > bounds.1 || nz < bounds.2 || nz > bounds.3 {
                        continue;
                    }
                    let grid_index = usize::try_from(i64::from(nz) - i64::from(bounds.2))
                        .unwrap_or(0)
                        .saturating_mul(grid_width)
                        .saturating_add(
                            usize::try_from(i64::from(nx) - i64::from(bounds.0)).unwrap_or(0),
                        );
                    if let Some(index) = grid[grid_index] {
                        let prior = positions[index];
                        let delta_x = prior[0] - x;
                        let delta_z = prior[2] - z;
                        if delta_x * delta_x + delta_z * delta_z < spacing_squared {
                            clear = false;
                        }
                    }
                    if !clear {
                        break;
                    }
                }
                if !clear {
                    break;
                }
            }
            if clear {
                let y = heights.map_or(0.0, |field| field.sample(x, z));
                chosen = Some([x, y, z]);
                break;
            }
        }
        if let Some(point) = chosen {
            if positions.len() == MAX_POINTS {
                return Err(ScatterError::TooMany {
                    requested: positions.len() + 1,
                    max: MAX_POINTS,
                });
            }
            let index = positions.len();
            positions.push(point);
            point_keys.push(cell_hash(params.seed, cx, cz, 0));
            let grid_index = usize::try_from(i64::from(cz) - i64::from(bounds.2))
                .unwrap_or(0)
                .saturating_mul(grid_width)
                .saturating_add(usize::try_from(i64::from(cx) - i64::from(bounds.0)).unwrap_or(0));
            grid[grid_index] = Some(index);
        }
    }
    let keep: Vec<bool> = positions
        .iter()
        .zip(point_keys)
        .map(|(point, key)| {
            let in_area = (params.area[0]..params.area[2]).contains(&point[0])
                && (params.area[1]..params.area[3]).contains(&point[2]);
            let mask_value = mask
                .map_or(1.0, |field| field.sample(point[0], point[2]))
                .clamp(0.0, 1.0);
            let probability = (params.density * mask_value).clamp(0.0, 1.0);
            in_area && unit(mix64(key ^ 0xa076_1d64_78bd_642f)) < probability
        })
        .collect();
    let mut result = PointSet::new(positions);
    result.retain(&keep);
    Ok(result)
}

/// Writes deterministic `scale`, `yaw`, `age`, and `id` channels.
pub fn vary(points: &mut PointSet, params: &Variation) {
    let mut scale = Vec::with_capacity(points.len());
    let mut yaw = Vec::with_capacity(points.len());
    let mut age = Vec::with_capacity(points.len());
    let mut id = Vec::with_capacity(points.len());
    for point in points.positions() {
        let key = point_hash(params.seed, point);
        scale.push(lerp(params.scale[0], params.scale[1], unit(key)));
        yaw.push(unit(mix64(key)) * std::f32::consts::TAU);
        age.push(lerp(
            params.age[0],
            params.age[1],
            unit(mix64(key ^ 0xe703_7ed1_a0b4_28db)),
        ));
        id.push(id_value(params.species));
    }
    let _ = points.set_channel("scale", scale);
    let _ = points.set_channel("yaw", yaw);
    let _ = points.set_channel("age", age);
    let _ = points.set_channel("id", id);
}

/// Adds a unit `normal` channel and a normalized `tilt` channel from a height field.
pub fn align_to_surface(points: &mut PointSet, heights: &Field2<'_>, amount: f32) {
    let mut normals = Vec::with_capacity(points.len());
    let mut tilts = Vec::with_capacity(points.len());
    for point in points.positions() {
        let step = heights.cell.max(f32::EPSILON);
        let dx = (heights.sample(point[0] + step, point[2])
            - heights.sample(point[0] - step, point[2]))
            / (2.0 * step);
        let dz = (heights.sample(point[0], point[2] + step)
            - heights.sample(point[0], point[2] - step))
            / (2.0 * step);
        let length = (dx * dx + 1.0 + dz * dz).sqrt();
        let normal = [-dx / length, 1.0 / length, -dz / length];
        let slope = (dx * dx + dz * dz).sqrt();
        let angle = slope.atan();
        normals.push(normal);
        tilts.push((amount * angle / std::f32::consts::FRAC_PI_2).clamp(0.0, 1.0));
    }
    let _ = points.set_vector_channel("normal", normals);
    let _ = points.set_channel("tilt", tilts);
}

/// Keeps points whose scalar channel lies within `lo..=hi`.
///
/// # Errors
/// Returns `BadChannel` if missing and `BadParameter` when `lo > hi`.
pub fn filter(points: &mut PointSet, channel: &str, lo: f32, hi: f32) -> Result<(), ScatterError> {
    if lo > hi {
        return Err(bad_parameter("range", "lo must not exceed hi"));
    }
    let values = points
        .channel(channel)
        .ok_or_else(|| ScatterError::BadChannel {
            name: channel.to_string(),
        })?;
    let keep: Vec<bool> = values
        .iter()
        .map(|value| (lo..=hi).contains(value))
        .collect();
    points.retain(&keep);
    Ok(())
}

/// Pushes nearby points apart while keeping them within their starting bounds.
///
/// # Errors
/// Returns `Cancelled` or `BadParameter` for a non-positive radius.
pub fn relax(
    points: &mut PointSet,
    radius: f32,
    iterations: u32,
    cancel: &dyn Fn() -> bool,
) -> Result<(), ScatterError> {
    if !radius.is_finite() || radius <= 0.0 {
        return Err(bad_parameter("radius", "must be finite and positive"));
    }
    if points.is_empty() {
        return Ok(());
    }
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_z = f32::INFINITY;
    let mut max_z = f32::NEG_INFINITY;
    for point in points.positions() {
        min_x = min_x.min(point[0]);
        max_x = max_x.max(point[0]);
        min_z = min_z.min(point[2]);
        max_z = max_z.max(point[2]);
    }
    for _ in 0..iterations {
        if cancel() {
            return Err(ScatterError::Cancelled);
        }
        let mut moved = points.positions().to_vec();
        for i in 0..moved.len() {
            for j in 0..i {
                let dx = moved[i][0] - moved[j][0];
                let dz = moved[i][2] - moved[j][2];
                let distance_squared = dx * dx + dz * dz;
                if distance_squared > 0.0 && distance_squared < radius * radius {
                    let distance = distance_squared.sqrt();
                    let push = (radius - distance) * 0.5 / distance;
                    moved[i][0] += dx * push;
                    moved[i][2] += dz * push;
                    moved[j][0] -= dx * push;
                    moved[j][2] -= dz * push;
                }
            }
        }
        for point in &mut moved {
            point[0] = point[0].clamp(min_x, max_x);
            point[2] = point[2].clamp(min_z, max_z);
        }
        points.positions_mut().copy_from_slice(&moved);
    }
    Ok(())
}

/// Creates deterministic children around each parent with a `parent` channel.
///
/// # Errors
/// Returns `Cancelled`, `TooMany`, or `BadParameter` for invalid ranges.
pub fn cluster(
    seed: u32,
    parents: &PointSet,
    children_per_parent: [u32; 2],
    spread: f32,
    cancel: &dyn Fn() -> bool,
) -> Result<PointSet, ScatterError> {
    if children_per_parent[0] > children_per_parent[1] {
        return Err(bad_parameter(
            "children_per_parent",
            "minimum exceeds maximum",
        ));
    }
    if !spread.is_finite() || spread < 0.0 {
        return Err(bad_parameter("spread", "must be finite and non-negative"));
    }
    let mut positions = Vec::new();
    let mut owners = Vec::new();
    for (parent_index, parent) in parents.positions().iter().enumerate() {
        if cancel() {
            return Err(ScatterError::Cancelled);
        }
        let parent_index_u32 = u32::try_from(parent_index).map_err(|_| ScatterError::TooMany {
            requested: parent_index,
            max: MAX_POINTS,
        })?;
        let key = point_hash(seed, parent);
        let range = u64::from(children_per_parent[1]) - u64::from(children_per_parent[0]) + 1;
        let offset = u32::try_from(mix64(key) % range).unwrap_or(0);
        let count = children_per_parent[0] + offset;
        for child in 0..count {
            if positions.len() == MAX_POINTS {
                return Err(ScatterError::TooMany {
                    requested: positions.len() + 1,
                    max: MAX_POINTS,
                });
            }
            let child_key = mix64(key ^ u64::from(child));
            let dx = unit(child_key) * 2.0 - 1.0;
            let dz = unit(mix64(child_key)) * 2.0 - 1.0;
            let length = (dx * dx + dz * dz).sqrt().max(f32::EPSILON);
            let distance = unit(mix64(child_key ^ 0x8ebc_6af0_9c88_c6e3)) * spread;
            positions.push([
                parent[0] + dx / length * distance,
                parent[1],
                parent[2] + dz / length * distance,
            ]);
            owners.push(id_value(parent_index_u32));
        }
    }
    let mut result = PointSet::new(positions);
    result.set_channel("parent", owners)?;
    Ok(result)
}

fn point_hash(seed: u32, point: &[f32; 3]) -> u64 {
    let x = u64::from(point[0].to_bits());
    let z = u64::from(point[2].to_bits());
    mix64((u64::from(seed) << 32) ^ x.rotate_left(17) ^ z.rotate_left(41))
}

fn validate_area(radius: f32, area: [f32; 4]) -> Result<(), ScatterError> {
    if !radius.is_finite() || radius <= 0.0 {
        return Err(bad_parameter("radius", "must be finite and positive"));
    }
    if !area.iter().all(|value| value.is_finite()) || area[2] <= area[0] || area[3] <= area[1] {
        return Err(bad_parameter(
            "area",
            "must have finite, positive width and depth",
        ));
    }
    Ok(())
}

pub(super) fn bad_parameter(name: &'static str, why: &str) -> ScatterError {
    ScatterError::BadParameter {
        name,
        why: why.to_string(),
    }
}

pub(super) fn cell_bounds(area: [f32; 4], cell: f32) -> Result<(i32, i32, i32, i32), ScatterError> {
    let min_x = crate::num::lattice_index(area[0] / cell).saturating_sub(2);
    let max_x = crate::num::lattice_index(area[2] / cell).saturating_add(2);
    let min_z = crate::num::lattice_index(area[1] / cell).saturating_sub(2);
    let max_z = crate::num::lattice_index(area[3] / cell).saturating_add(2);
    if max_x < min_x || max_z < min_z {
        return Err(bad_parameter(
            "area",
            "grid coordinates exceed supported bounds",
        ));
    }
    Ok((min_x, max_x, min_z, max_z))
}

pub(super) fn canonical_area(area: [f32; 4]) -> [f32; 4] {
    let block = 256.0;
    [
        (area[0] / block).floor() * block,
        (area[1] / block).floor() * block,
        (area[2] / block).ceil() * block,
        (area[3] / block).ceil() * block,
    ]
}

pub(super) fn bounds_count(bounds: (i32, i32, i32, i32)) -> Result<usize, ScatterError> {
    let width = usize::try_from(i64::from(bounds.1) - i64::from(bounds.0) + 1)
        .map_err(|_| bad_parameter("area", "grid is too large"))?;
    let depth = usize::try_from(i64::from(bounds.3) - i64::from(bounds.2) + 1)
        .map_err(|_| bad_parameter("area", "grid is too large"))?;
    width
        .checked_mul(depth)
        .ok_or_else(|| bad_parameter("area", "grid is too large"))
}
