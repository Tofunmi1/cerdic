//! A reusable facade grammar for a close-view building: floor and bay splits, real window
//! openings, frames, mullions, belt courses, a stepped crown and rooftop equipment.

use super::{BuildingError, Mesh, Rng, area};
mod mesh;
use mesh::{block_on_edge, outline, roof_tank};
mod openings;
use openings::window_opening;
mod styles;
use styles::{STYLES, profile};

/// Independent material groups for an authored building asset.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HeroAsset {
    /// Stone, brick or concrete wall panels around window openings.
    pub masonry: Mesh,
    /// Recessed window panes.
    pub glass: Mesh,
    /// Window frames, pilasters, floor bands and crown accents.
    pub trim: Mesh,
    /// The roof slab, parapet and mechanical equipment.
    pub roof: Mesh,
    /// Ground-floor door leaves, surrounds, handles and stoops.
    pub doors: Mesh,
}

/// Parameters for [`build_hero_asset`]. The result is deterministic for a given seed.
#[derive(Clone, Debug, PartialEq)]
pub struct HeroAssetParams {
    /// Building origin in metres (x, y, z).
    pub at: [f32; 3],
    /// Front width in metres.
    pub width: f32,
    /// Building depth in metres.
    pub depth: f32,
    /// Storey count.
    pub floors: u8,
    /// Window bays on each facade edge.
    pub bays: u8,
    /// Storey height in metres.
    pub floor_height: f32,
    /// Detail tier from 0 (clean asset) to 3 (dense mullions and rooftop pieces).
    pub detail: u8,
    /// Facade grammar: art deco, classical, tenement, brownstone, precast, curtainwall, industrial, brutalist, modernist, mediterranean, victorian or warehouse.
    pub style: String,
    /// Stable procedural variation seed.
    pub seed: u64,
}

fn range(name: &'static str, allowed: &'static str, value: f32) -> BuildingError {
    BuildingError::Range {
        name,
        range: allowed,
        value,
    }
}

fn check(params: &HeroAssetParams) -> Result<(), BuildingError> {
    if !(8.0..=80.0).contains(&params.width) {
        return Err(range("width", "8 to 80 metres", params.width));
    }
    if !(8.0..=80.0).contains(&params.depth) {
        return Err(range("depth", "8 to 80 metres", params.depth));
    }
    if !(2..=40).contains(&params.floors) {
        return Err(range("floors", "2 to 40 storeys", f32::from(params.floors)));
    }
    if !(2..=16).contains(&params.bays) {
        return Err(range("bays", "2 to 16", f32::from(params.bays)));
    }
    if !(2.8..=5.5).contains(&params.floor_height) {
        return Err(range(
            "floor_height",
            "2.8 to 5.5 metres",
            params.floor_height,
        ));
    }
    if params.detail > 3 {
        return Err(range("detail", "0 to 3", f32::from(params.detail)));
    }
    if !STYLES.contains(&params.style.as_str()) {
        return Err(BuildingError::Footprint(
            "unknown facade style; choose art_deco, classical, tenement, brownstone, precast, curtainwall, industrial, brutalist, modernist, mediterranean, victorian or warehouse",
        ));
    }
    Ok(())
}

/// Builds a detailed building from floor, bay and facade rules.
///
/// # Errors
/// Refuses dimensions, styles or detail tiers outside the documented limits.
pub fn build_hero_asset(params: &HeroAssetParams) -> Result<HeroAsset, BuildingError> {
    check(params)?;
    let mut asset = HeroAsset::default();
    let mut rng = Rng(params.seed ^ 0xD1B5_4A32_D192_ED03);
    let height = f32::from(params.floors) * params.floor_height;
    let style = profile(&params.style);
    let mut previous = outline(
        params.width,
        params.depth,
        0,
        params.floors,
        style.stepped_crown,
    );

    for floor in 0..params.floors {
        let y = f32::from(floor) * params.floor_height;
        let current = outline(
            params.width,
            params.depth,
            floor,
            params.floors,
            style.stepped_crown,
        );
        if current != previous {
            asset.roof.ring(&previous, &current, y);
            previous = current.clone();
        }
        for edge in 0..4 {
            let a = current[edge];
            let b = current[(edge + 1) % 4];
            let length = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
            for bay in 0..params.bays {
                window_opening(&mut asset, params, a, b, floor, bay, edge, y, &mut rng);
            }
            let band_y = y + params.floor_height - 0.13;
            if (floor + 1) % style.band_every == 0 {
                block_on_edge(
                    &mut asset.trim,
                    a,
                    b,
                    0.5,
                    0.045,
                    [length, 0.13, 0.16],
                    band_y,
                );
            }
            if style.pilaster_every > 0 {
                for bay in 0..=params.bays {
                    if bay % style.pilaster_every == 0 {
                        let along = f32::from(bay) / f32::from(params.bays);
                        block_on_edge(
                            &mut asset.trim,
                            a,
                            b,
                            along,
                            0.08,
                            [style.pilaster_width, params.floor_height, 0.22],
                            y,
                        );
                    }
                }
            }
        }
    }

    let top = outline(
        params.width,
        params.depth,
        params.floors - 1,
        params.floors,
        style.stepped_crown,
    );
    asset.roof.cap(&top, height);
    asset
        .roof
        .walls(&top, height, height + 0.95, params.floor_height);
    asset.roof.cap(&super::inset(&top, 0.12), height + 0.95);
    if area(&top) > 25.0 {
        for x in [-params.width * 0.2, params.width * 0.2] {
            for z in [-params.depth * 0.2, params.depth * 0.2] {
                let jitter = (rng.next() - 0.5) * 0.4;
                asset.roof.block(
                    [x + jitter, z - jitter],
                    [2.2, 1.2 + rng.next() * 0.8, 1.45],
                    0.0,
                    height + 0.95,
                );
            }
        }
        if style.roof_tank {
            roof_tank(&mut asset.roof, [0.0, 0.0], 0.85, 1.9, height + 0.95);
        }
    }
    for mesh in [
        &mut asset.masonry,
        &mut asset.glass,
        &mut asset.trim,
        &mut asset.roof,
        &mut asset.doors,
    ] {
        for point in &mut mesh.positions {
            point[0] += params.at[0];
            point[1] += params.at[1];
            point[2] += params.at[2];
        }
    }
    Ok(asset)
}
