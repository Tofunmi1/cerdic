//! Deterministic street, block, lot, and height generation for a city district.
mod blocks;
mod random;
mod streets;

use blocks::make_blocks;
use random::Random;
use streets::StreetGrid;

/// Inputs for generating a city district in metres.
#[derive(Clone, Debug, PartialEq)]
pub struct CityParams {
    /// District x and z extent, centred on the origin.
    pub size: [f32; 2],
    /// Downtown centre in x and z coordinates.
    pub centre: [f32; 2],
    /// Target distance between streets along X in metres.
    pub block: f32,
    /// Target distance between streets along Z; 0 means the same as `block` (a square grid), 15 or more gives long thin blocks.
    pub block_depth: f32,
    /// Width of the street gap between blocks in metres.
    pub street_width: f32,
    /// Desired minimum lot side in metres.
    pub lot_min: f32,
    /// Maximum desired long side of a lot in metres.
    pub lot_max: f32,
    /// Street displacement from a square grid, from zero to one.
    pub irregularity: f32,
    /// Seed for deterministic layout variation.
    pub seed: u64,
}

impl CityParams {
    /// The spacing between streets along Z: `block_depth`, or `block` when that is 0.
    pub(crate) fn depth(&self) -> f32 {
        if self.block_depth == 0.0 {
            self.block
        } else {
            self.block_depth
        }
    }
}

impl Default for CityParams {
    fn default() -> Self {
        Self {
            size: [300.0, 300.0],
            centre: [0.0, 0.0],
            block: 40.0,
            block_depth: 0.0,
            street_width: 9.0,
            lot_min: 6.0,
            lot_max: 14.0,
            irregularity: 0.5,
            seed: 1,
        }
    }
}

/// A lot footprint, its parent block, height, and street-facing edge.
#[derive(Clone, Debug, PartialEq)]
pub struct Lot {
    /// Counter-clockwise footprint corners in x and z coordinates.
    pub footprint: Vec<[f32; 2]>,
    /// Contiguous index of the parent block.
    pub block: u32,
    /// Generated building height in metres.
    pub height: f32,
    /// Index into `footprint` of the street-facing edge.
    pub front: usize,
}

/// Generated street graph and its lots.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    /// Street graph vertices in x and z coordinates.
    pub street_nodes: Vec<[f32; 2]>,
    /// Street graph segments as vertex indices.
    pub street_edges: Vec<(u32, u32)>,
    /// Lots in deterministic block and split order.
    pub lots: Vec<Lot>,
}

/// An invalid city parameter or a layout that exceeds its bounded cost.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum CityError {
    /// A parameter is outside its documented range.
    #[error("parameter {name} is invalid: allowed range {range}")]
    InvalidParameter {
        /// Parameter name.
        name: &'static str,
        /// Allowed range or constraint.
        range: &'static str,
    },
    /// The requested district would exceed the lot budget.
    #[error("parameter {name} would exceed the maximum of 20000 lots")]
    LotBudget {
        /// Parameter responsible for the excessive layout.
        name: &'static str,
    },
}

/// Generate a complete deterministic city layout.
///
/// # Errors
/// Returns an error when a parameter is outside its documented range or the layout would exceed the lot budget.
pub fn layout(params: &CityParams) -> Result<Layout, CityError> {
    validate(params)?;
    let grid = StreetGrid::new(params);
    let blocks = make_blocks(params, &grid);
    let mut lots = Vec::new();
    for block in &blocks {
        block.split_into(params, block.id, &grid, &mut lots)?;
    }
    if lots.len() > 20_000 {
        return Err(CityError::LotBudget { name: "lot_max" });
    }
    let rng = Random::new(params.seed);
    let mut heights: Vec<f32> = lots
        .iter()
        .enumerate()
        .map(|(index, lot)| height(params, lot, index, &rng))
        .collect();
    add_towers(params, &lots, &mut heights, &rng);
    for (lot, value) in lots.iter_mut().zip(heights) {
        lot.height = value;
    }
    Ok(Layout {
        street_nodes: grid.nodes,
        street_edges: grid.edges,
        lots,
    })
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "validated district and block ranges bound the cell counts below 67"
)]
fn validate(p: &CityParams) -> Result<(), CityError> {
    let bad = |name, range| Err(CityError::InvalidParameter { name, range });
    if p.size
        .iter()
        .any(|v| !v.is_finite() || !(60.0..=1000.0).contains(v))
    {
        return bad(
            "size",
            "finite values from 60 through 1000 metres on each axis",
        );
    }
    if p.centre.iter().any(|v| !v.is_finite()) {
        return bad("size", "a finite centre");
    }
    if !p.block.is_finite() || !(15.0..=200.0).contains(&p.block) {
        return bad("block", "15 through 200 metres");
    }
    if !p.block_depth.is_finite()
        || (p.block_depth != 0.0 && !(15.0..=400.0).contains(&p.block_depth))
    {
        return bad(
            "block_depth",
            "0 (the same as block) or 15 through 400 metres",
        );
    }
    if !p.street_width.is_finite() || !(2.0..=40.0).contains(&p.street_width) {
        return bad("street_width", "2 through 40 metres");
    }
    if !p.lot_min.is_finite() || !(3.0..=40.0).contains(&p.lot_min) {
        return bad("lot_min", "3 through 40 metres");
    }
    if !p.lot_max.is_finite() || p.lot_max < p.lot_min * 1.5 || p.lot_max > 80.0 {
        return bad(
            "lot_max",
            "at least 1.5 times lot_min and at most 80 metres",
        );
    }
    if !p.irregularity.is_finite() || !(0.0..=1.0).contains(&p.irregularity) {
        return bad("irregularity", "0 through 1");
    }
    let across_x = (p.size[0] / p.block).ceil() as usize;
    let across_z = (p.size[1] / p.depth()).ceil() as usize;
    let rough_lots = across_x.saturating_mul(across_z).saturating_mul(64);
    if rough_lots > 20_000 {
        return Err(CityError::LotBudget { name: "block" });
    }
    Ok(())
}

#[expect(
    clippy::cast_precision_loss,
    reason = "lot footprints have at most eight corners"
)]
fn height(p: &CityParams, lot: &Lot, index: usize, rng: &Random) -> f32 {
    let centre = lot.footprint.iter().fold([0.0, 0.0], |mut sum, point| {
        sum[0] += point[0] / lot.footprint.len() as f32;
        sum[1] += point[1] / lot.footprint.len() as f32;
        sum
    });
    let radius = p.size[0].min(p.size[1]) * 0.5;
    let dx = (centre[0] - p.centre[0]) / radius;
    let dz = (centre[1] - p.centre[1]) / radius;
    let distance = (dx * dx + dz * dz).sqrt().min(1.5);
    let base = if distance < 0.35 {
        47.0 - distance * 18.0
    } else {
        40.7 - (distance - 0.35).min(0.8) * 39.0
    };
    (base + rng.signed(index as u64, 0x4845_4947_4854) * 5.0).clamp(6.0, 58.0)
}

#[expect(
    clippy::cast_precision_loss,
    reason = "lot footprints have at most eight corners"
)]
fn add_towers(p: &CityParams, lots: &[Lot], heights: &mut [f32], rng: &Random) {
    if lots.is_empty() {
        return;
    }
    let desired = (lots.len() * 4 / 100).max(3).min(lots.len() / 12);
    if desired == 0 {
        return;
    }
    let radius = p.size[0].min(p.size[1]) * 0.5;
    let mut candidates: Vec<(u64, usize)> = lots
        .iter()
        .enumerate()
        .filter_map(|(index, lot)| {
            let centre = lot.footprint.iter().fold([0.0, 0.0], |mut sum, point| {
                sum[0] += point[0] / lot.footprint.len() as f32;
                sum[1] += point[1] / lot.footprint.len() as f32;
                sum
            });
            let dx = centre[0] - p.centre[0];
            let dz = centre[1] - p.centre[1];
            let distance = (dx * dx + dz * dz).sqrt();
            (distance < radius * 0.62).then_some((rng.bits(index as u64, 0x544f_5745_5253), index))
        })
        .collect();
    candidates.sort_unstable();
    for (_, index) in candidates.into_iter().take(desired) {
        heights[index] = 90.0 + rng.unit(index as u64, 0x544f_5745_5253) * 90.0;
    }
}
