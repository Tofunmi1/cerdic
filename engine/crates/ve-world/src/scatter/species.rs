//! Species and biome rules: which plant grows where.
use std::collections::HashMap;

use super::place::random::{cell_hash, id_value, mix64, unit};
use super::place::{bad_parameter, bounds_count, canonical_area, cell_bounds};
use super::{Field2, PointSet, ScatterError};

/// One rule: the species likes `channel` between `lo` and `hi`, softly (`soft` is the smoothstep width on both ends).
#[derive(Debug, Clone, PartialEq)]
pub struct Rule {
    /// Name of the field (from the `channels` given to [`place_species`]).
    pub channel: String,
    /// Lowest value it likes.
    pub lo: f32,
    /// Highest value it likes.
    pub hi: f32,
    /// Width of the soft edge on both ends.
    pub soft: f32,
    /// Strength of this rule.
    pub weight: f32,
}

/// A kind of plant with its spacing and the rules that decide where it grows.
#[derive(Debug, Clone, PartialEq)]
pub struct Species {
    /// Written to the `id` channel.
    pub id: u32,
    /// Seed of this species' placement.
    pub seed: u32,
    /// Minimum spacing in metres.
    pub radius: f32,
    /// Overall density 0..=1.
    pub density: f32,
    /// The rules; the species' weight at a point is the product of the rule memberships times each rule's weight.
    pub rules: Vec<Rule>,
}

/// Places several species over a rectangle. Where species overlap, a lattice cell goes to the species with the highest weight there
/// (ties by id), so adding a species changes only the cells it wins. Locally stable like [`super::poisson`].
///
/// # Errors
/// `Cancelled`, `TooMany`, `BadChannel` (a rule names a field that was not given), `BadParameter`.
#[expect(
    clippy::too_many_lines,
    reason = "species placement validates rules, resolves winners, and clips stable output"
)]
pub fn place_species(
    species: &[Species],
    area: [f32; 4],
    channels: &[(&str, Field2<'_>)],
    heights: Option<&Field2<'_>>,
    cancel: &dyn Fn() -> bool,
) -> Result<PointSet, ScatterError> {
    if !area.iter().all(|value| value.is_finite()) || area[2] <= area[0] || area[3] <= area[1] {
        return Err(bad_parameter(
            "area",
            "must have finite, positive width and depth",
        ));
    }
    if species.is_empty() {
        return Ok(PointSet::default());
    }
    let mut by_name = HashMap::with_capacity(channels.len());
    for (name, field) in channels {
        by_name.insert(*name, field);
    }
    for kind in species {
        if !kind.radius.is_finite() || kind.radius <= 0.0 {
            return Err(bad_parameter(
                "radius",
                "species radius must be finite and positive",
            ));
        }
        if !kind.density.is_finite() || kind.density < 0.0 {
            return Err(bad_parameter(
                "density",
                "species density must be finite and non-negative",
            ));
        }
        for rule in &kind.rules {
            if !by_name.contains_key(rule.channel.as_str()) {
                return Err(ScatterError::BadChannel {
                    name: rule.channel.clone(),
                });
            }
            if rule.lo > rule.hi
                || !rule.lo.is_finite()
                || !rule.hi.is_finite()
                || !rule.soft.is_finite()
                || rule.soft < 0.0
            {
                return Err(bad_parameter(
                    "rule",
                    "bounds must be finite and soft width non-negative",
                ));
            }
            if !rule.weight.is_finite() || rule.weight < 0.0 {
                return Err(bad_parameter("weight", "must be finite and non-negative"));
            }
        }
    }
    let base_seed = 0x6a09_e667_u32;
    let base_radius = species
        .iter()
        .map(|kind| kind.radius)
        .fold(f32::INFINITY, f32::min);
    let cell = base_radius / std::f32::consts::SQRT_2;
    let bounds = cell_bounds(canonical_area(area), cell)?;
    let cell_count = bounds_count(bounds)?;
    if cell_count > 64_000_000 {
        return Err(ScatterError::TooMany {
            requested: cell_count,
            max: 1_000_000,
        });
    }
    let mut cells = Vec::with_capacity(cell_count);
    for z in bounds.2..=bounds.3 {
        if cancel() {
            return Err(ScatterError::Cancelled);
        }
        for x in bounds.0..=bounds.1 {
            cells.push((cell_hash(base_seed, x, z, 0), x, z));
        }
    }
    cells.sort_unstable();
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut ids: Vec<f32> = Vec::new();
    let mut spacings = Vec::new();
    let mut grid: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
    for (_, cx, cz) in cells {
        let cell_key = cell_hash(base_seed, cx, cz, 1);
        let x = (crate::num::lattice_f32(cx) + unit(cell_key)) * cell;
        let z = (crate::num::lattice_f32(cz) + unit(mix64(cell_key)) * 0.999_999) * cell;
        let mut winner: Option<(&Species, f32)> = None;
        for kind in species {
            let weight = species_weight(kind, x, z, &by_name) * kind.density;
            if weight <= 0.0 {
                continue;
            }
            let wins =
                winner.is_none_or(
                    |(prior, prior_weight)| match weight.total_cmp(&prior_weight) {
                        std::cmp::Ordering::Greater => true,
                        std::cmp::Ordering::Equal => kind.id < prior.id,
                        std::cmp::Ordering::Less => false,
                    },
                );
            if wins {
                winner = Some((kind, weight));
            }
        }
        let Some((kind, weight)) = winner else {
            continue;
        };
        let chance_key = mix64(cell_key ^ u64::from(kind.seed));
        if unit(chance_key) >= weight.clamp(0.0, 1.0) {
            continue;
        }
        let spacing = kind.radius / weight.max(0.05).sqrt();
        let mut clear = true;
        for dz in -2..=2 {
            for dx in -2..=2 {
                let Some(nx) = cx.checked_add(dx) else {
                    continue;
                };
                let Some(nz) = cz.checked_add(dz) else {
                    continue;
                };
                if let Some(indices) = grid.get(&(nx, nz)) {
                    for index in indices {
                        let prior = positions[*index];
                        let delta_x = prior[0] - x;
                        let delta_z = prior[2] - z;
                        let required = spacing.max(spacings[*index]);
                        if delta_x * delta_x + delta_z * delta_z < required * required {
                            clear = false;
                            break;
                        }
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
            if positions.len() == 1_000_000 {
                return Err(ScatterError::TooMany {
                    requested: positions.len() + 1,
                    max: 1_000_000,
                });
            }
            let y = heights.map_or(0.0, |field| field.sample(x, z));
            let index = positions.len();
            positions.push([x, y, z]);
            ids.push(id_value(kind.id));
            spacings.push(spacing);
            grid.entry((cx, cz)).or_default().push(index);
        }
    }
    let keep: Vec<bool> = positions
        .iter()
        .map(|point| {
            (area[0]..area[2]).contains(&point[0]) && (area[1]..area[3]).contains(&point[2])
        })
        .collect();
    let mut points = PointSet::new(positions);
    points.retain(&keep);
    let ids: Vec<f32> = ids
        .into_iter()
        .zip(keep)
        .filter_map(|(id, keep)| keep.then_some(id))
        .collect();
    points.set_channel("id", ids)?;
    Ok(points)
}

fn species_weight(kind: &Species, x: f32, z: f32, channels: &HashMap<&str, &Field2<'_>>) -> f32 {
    kind.rules.iter().fold(1.0, |weight, rule| {
        let Some(field) = channels.get(rule.channel.as_str()) else {
            return 0.0;
        };
        weight * membership(field.sample(x, z), rule) * rule.weight
    })
}

fn membership(value: f32, rule: &Rule) -> f32 {
    if value < rule.lo - rule.soft || value > rule.hi + rule.soft {
        return 0.0;
    }
    if value < rule.lo && rule.soft > 0.0 {
        let t = ((value - (rule.lo - rule.soft)) / rule.soft).clamp(0.0, 1.0);
        return t * t * (3.0 - 2.0 * t);
    }
    if value > rule.hi && rule.soft > 0.0 {
        let t = (((rule.hi + rule.soft) - value) / rule.soft).clamp(0.0, 1.0);
        return t * t * (3.0 - 2.0 * t);
    }
    1.0
}
