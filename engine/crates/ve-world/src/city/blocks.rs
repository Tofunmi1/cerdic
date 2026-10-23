//! Street-bounded block polygons and recursive lot splitting.
use super::{CityError, CityParams, Lot, Random, streets::StreetGrid};

/// A convex street-inset block polygon.
pub(super) struct Block {
    /// Stable row-major block identifier.
    pub(super) id: u32,
    polygon: Vec<[f32; 2]>,
}

/// Build inset blocks between every neighbouring pair of street lines.
pub(super) fn make_blocks(params: &CityParams, grid: &StreetGrid) -> Vec<Block> {
    let mut blocks = Vec::with_capacity(grid.x_lines * grid.z_lines);
    let mut block_id = 0_u32;
    for z in 0..grid.z_lines {
        for x in 0..grid.x_lines {
            let quad = vec![
                grid.junction(x, z),
                grid.junction(x + 1, z),
                grid.junction(x + 1, z + 1),
                grid.junction(x, z + 1),
            ];
            let polygon = inset(&quad, params.street_width * 0.5);
            if polygon.len() < 3 || area(&polygon) <= 1.0 {
                continue;
            }
            blocks.push(Block {
                id: block_id,
                polygon,
            });
            block_id += 1;
        }
    }
    blocks
}

impl Block {
    /// Split this block into lots while keeping a stable block ID and frontage.
    pub(super) fn split_into(
        &self,
        params: &CityParams,
        block_id: u32,
        grid: &StreetGrid,
        output: &mut Vec<Lot>,
    ) -> Result<(), CityError> {
        let rng = Random::new(params.seed);
        split_polygon(&self.polygon, params, block_id, grid, &rng, 0, output)
    }
}

fn split_polygon(
    poly: &[[f32; 2]],
    params: &CityParams,
    block: u32,
    grid: &StreetGrid,
    rng: &Random,
    path: u64,
    output: &mut Vec<Lot>,
) -> Result<(), CityError> {
    let (min, max) = bounds(poly);
    let width = max[0] - min[0];
    let depth = max[1] - min[1];
    if width.max(depth) <= params.lot_max || poly.len() >= 8 {
        if output.len() >= 20_000 {
            return Err(CityError::LotBudget { name: "lot_max" });
        }
        let front = front_edge(poly, grid);
        output.push(Lot {
            footprint: poly.to_vec(),
            block,
            height: 0.0,
            front,
        });
        return Ok(());
    }
    let split_x = width >= depth;
    let low = if split_x { min[0] } else { min[1] };
    let high = if split_x { max[0] } else { max[1] };
    let ratio = 0.35 + rng.unit(path ^ u64::from(block).rotate_left(19), 0x0053_504c_4954) * 0.30;
    let cut = low + (high - low) * ratio;
    let (a, b) = if split_x {
        (clip(poly, 0, cut, true), clip(poly, 0, cut, false))
    } else {
        (clip(poly, 1, cut, true), clip(poly, 1, cut, false))
    };
    if a.len() < 3
        || b.len() < 3
        || min_side(&a) < params.lot_min * 0.5
        || min_side(&b) < params.lot_min * 0.5
    {
        if output.len() >= 20_000 {
            return Err(CityError::LotBudget { name: "lot_max" });
        }
        let front = front_edge(poly, grid);
        output.push(Lot {
            footprint: poly.to_vec(),
            block,
            height: 0.0,
            front,
        });
        return Ok(());
    }
    split_polygon(
        &a,
        params,
        block,
        grid,
        rng,
        path.wrapping_mul(2).wrapping_add(1),
        output,
    )?;
    split_polygon(
        &b,
        params,
        block,
        grid,
        rng,
        path.wrapping_mul(2).wrapping_add(2),
        output,
    )
}

fn clip(poly: &[[f32; 2]], axis: usize, cut: f32, keep_low: bool) -> Vec<[f32; 2]> {
    let mut result = Vec::new();
    for index in 0..poly.len() {
        let current = poly[index];
        let next = poly[(index + 1) % poly.len()];
        let current_inside = if keep_low {
            current[axis] <= cut
        } else {
            current[axis] >= cut
        };
        let next_inside = if keep_low {
            next[axis] <= cut
        } else {
            next[axis] >= cut
        };
        if current_inside {
            result.push(current);
        }
        if current_inside != next_inside {
            let t = (cut - current[axis]) / (next[axis] - current[axis]);
            let other = 1 - axis;
            let mut point = [0.0, 0.0];
            point[axis] = cut;
            point[other] = current[other] + (next[other] - current[other]) * t;
            result.push(point);
        }
    }
    result
}

fn inset(poly: &[[f32; 2]], distance: f32) -> Vec<[f32; 2]> {
    let mut shifted = Vec::with_capacity(poly.len());
    for index in 0..poly.len() {
        let a = poly[index];
        let b = poly[(index + 1) % poly.len()];
        let dx = b[0] - a[0];
        let dz = b[1] - a[1];
        let length = (dx * dx + dz * dz).sqrt().max(1.0e-5);
        let outward = [dz / length, -dx / length];
        shifted.push((
            [a[0] - outward[0] * distance, a[1] - outward[1] * distance],
            [dx, dz],
        ));
    }
    (0..poly.len())
        .map(|index| {
            let previous = (index + poly.len() - 1) % poly.len();
            line_intersection(shifted[previous], shifted[index]).unwrap_or(poly[index])
        })
        .collect()
}

fn line_intersection(a: ([f32; 2], [f32; 2]), b: ([f32; 2], [f32; 2])) -> Option<[f32; 2]> {
    let cross = a.1[0] * b.1[1] - a.1[1] * b.1[0];
    if cross.abs() < 1.0e-5 {
        return None;
    }
    let delta = [b.0[0] - a.0[0], b.0[1] - a.0[1]];
    let t = (delta[0] * b.1[1] - delta[1] * b.1[0]) / cross;
    Some([a.0[0] + a.1[0] * t, a.0[1] + a.1[1] * t])
}

fn front_edge(poly: &[[f32; 2]], grid: &StreetGrid) -> usize {
    let mut best = (f32::MAX, 0);
    for index in 0..poly.len() {
        let a = poly[index];
        let b = poly[(index + 1) % poly.len()];
        let midpoint = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5];
        let mut distance = f32::MAX;
        for &(from, to) in &grid.edges {
            let p = grid.nodes[from as usize];
            let q = grid.nodes[to as usize];
            distance = distance.min(segment_distance(midpoint, p, q));
        }
        if distance < best.0 {
            best = (distance, index);
        }
    }
    best.1
}

fn segment_distance(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let ab = [b[0] - a[0], b[1] - a[1]];
    let ap = [p[0] - a[0], p[1] - a[1]];
    let length2 = (ab[0] * ab[0] + ab[1] * ab[1]).max(1.0e-8);
    let t = ((ap[0] * ab[0] + ap[1] * ab[1]) / length2).clamp(0.0, 1.0);
    let dx = p[0] - (a[0] + t * ab[0]);
    let dz = p[1] - (a[1] + t * ab[1]);
    (dx * dx + dz * dz).sqrt()
}

fn bounds(poly: &[[f32; 2]]) -> ([f32; 2], [f32; 2]) {
    poly.iter().fold(
        ([f32::MAX; 2], [f32::MIN; 2]),
        |(mut low, mut high), point| {
            for axis in 0..2 {
                low[axis] = low[axis].min(point[axis]);
                high[axis] = high[axis].max(point[axis]);
            }
            (low, high)
        },
    )
}

fn min_side(poly: &[[f32; 2]]) -> f32 {
    let (low, high) = bounds(poly);
    (high[0] - low[0]).min(high[1] - low[1])
}

fn area(poly: &[[f32; 2]]) -> f32 {
    (0..poly.len())
        .map(|i| {
            let a = poly[i];
            let b = poly[(i + 1) % poly.len()];
            a[0] * b[1] - b[0] * a[1]
        })
        .sum::<f32>()
        * 0.5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recursive_split_preserves_area_and_creates_building_sized_lots() {
        let params = CityParams {
            lot_max: 14.0,
            ..CityParams::default()
        };
        let polygon = vec![[0.0, 0.0], [38.0, 0.0], [38.0, 32.0], [0.0, 32.0]];
        let grid = StreetGrid::new(&params);
        let block = Block {
            id: 0,
            polygon: polygon.clone(),
        };
        let mut output = Vec::new();
        block
            .split_into(&params, 0, &grid, &mut output)
            .expect("split fits budget");
        assert!(output.len() >= 4);
        assert!(
            (output.iter().map(|lot| area(&lot.footprint)).sum::<f32>() - area(&polygon)).abs()
                < 0.01
        );
        assert!(
            output
                .iter()
                .all(|lot| min_side(&lot.footprint) >= params.lot_min * 0.5)
        );
    }
}
