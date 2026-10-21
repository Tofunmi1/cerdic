//! Climate channels for species rules.
use super::GroundRef;

/// Climate settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Climate {
    /// Height of sea level in metres (temperature is `base_temperature` there).
    pub sea_level: f32,
    /// Degrees Celsius the temperature falls per 1000 m of height above sea level.
    pub lapse_per_km: f32,
    /// Temperature at sea level at z = 0, in degrees Celsius.
    pub base_temperature: f32,
    /// Degrees Celsius the temperature falls per 1000 m toward +z (latitude).
    pub latitude_gradient: f32,
    /// Distance over which closeness to water adds moisture, in metres.
    pub moisture_distance: f32,
}

/// Temperature in degrees Celsius per cell: `base - lapse * (height - sea_level) / 1000 - latitude * z / 1000`.
#[must_use]
pub fn temperature(ground: &GroundRef<'_>, climate: &Climate) -> Vec<f32> {
    ground
        .heights
        .iter()
        .enumerate()
        .map(|(index, height)| {
            let z = index / ground.width;
            let world_z = ground.origin[1] + crate::num::cell_f32(z) * ground.cell;
            climate.base_temperature
                - climate.lapse_per_km * (height - climate.sea_level) / 1000.0
                - climate.latitude_gradient * world_z / 1000.0
        })
        .collect()
}

/// Moisture 0..=1 per cell from the distance to the nearest wet cell (`water` holds a depth per cell; above 0.01 m is wet): 1 on
/// water, falling smoothly to 0 at `moisture_distance`; all 0 with no water.
#[must_use]
pub fn moisture(ground: &GroundRef<'_>, water: &[f32], climate: &Climate) -> Vec<f32> {
    let count = ground.width.saturating_mul(ground.depth);
    if water.len() != count
        || climate.moisture_distance <= 0.0
        || !climate.moisture_distance.is_finite()
    {
        return vec![0.0; count];
    }
    let mut distances = vec![f32::INFINITY; count];
    for (index, value) in water.iter().enumerate() {
        if *value > 0.01 {
            distances[index] = 0.0;
        }
    }
    if distances.iter().all(|distance| !distance.is_finite()) {
        return vec![0.0; count];
    }
    let diagonal = std::f32::consts::SQRT_2 * ground.cell;
    for z in 0..ground.depth {
        for x in 0..ground.width {
            relax(&mut distances, ground.width, x, z, -1, 0, ground.cell);
            relax(&mut distances, ground.width, x, z, 0, -1, ground.cell);
            relax(&mut distances, ground.width, x, z, -1, -1, diagonal);
            relax(&mut distances, ground.width, x, z, 1, -1, diagonal);
        }
    }
    for z in (0..ground.depth).rev() {
        for x in (0..ground.width).rev() {
            relax(&mut distances, ground.width, x, z, 1, 0, ground.cell);
            relax(&mut distances, ground.width, x, z, 0, 1, ground.cell);
            relax(&mut distances, ground.width, x, z, 1, 1, diagonal);
            relax(&mut distances, ground.width, x, z, -1, 1, diagonal);
        }
    }
    distances
        .iter()
        .map(|distance| 1.0 - smoothstep(0.0, climate.moisture_distance, *distance))
        .collect()
}

fn relax(distances: &mut [f32], width: usize, x: usize, z: usize, dx: isize, dz: isize, cost: f32) {
    let nx = x.checked_add_signed(dx);
    let nz = z.checked_add_signed(dz);
    if let (Some(nx), Some(nz)) = (nx.filter(|value| *value < width), nz) {
        let next = nz * width + nx;
        if next < distances.len() {
            let index = z * width + x;
            distances[index] = distances[index].min(distances[next] + cost);
        }
    }
}

fn smoothstep(start: f32, end: f32, value: f32) -> f32 {
    let t = ((value - start) / (end - start).max(f32::EPSILON)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
