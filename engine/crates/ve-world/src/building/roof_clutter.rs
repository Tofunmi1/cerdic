//! Seeded rooftop equipment for the district-scale building generator.

use super::{BuildingParams, Mesh, Rng, area, contains, convex, inset};
use std::f32::consts::TAU;

pub(super) fn add(
    roof: &mut Mesh,
    outline: &[[f32; 2]],
    y: f32,
    params: &BuildingParams,
    rng: &mut Rng,
) {
    if params.clutter <= 0.0 {
        return;
    }
    let region = inset(outline, 1.3);
    if area(&region) < 4.0 || !convex(&region) {
        return;
    }
    let (lo, hi) = region
        .iter()
        .fold(([f32::MAX; 2], [f32::MIN; 2]), |(lo, hi), p| {
            (
                [lo[0].min(p[0]), lo[1].min(p[1])],
                [hi[0].max(p[0]), hi[1].max(p[1])],
            )
        });
    let place = |rng: &mut Rng| {
        for _ in 0..24 {
            let p = [
                lo[0] + (hi[0] - lo[0]) * rng.next(),
                lo[1] + (hi[1] - lo[1]) * rng.next(),
            ];
            if contains(&region, p) {
                return Some(p);
            }
        }
        None
    };
    let roof_area = area(outline);
    #[expect(clippy::cast_possible_truncation, reason = "a handful of units")]
    #[expect(clippy::cast_sign_loss, reason = "clutter and area are not negative")]
    let units = ((params.clutter * (2.0 + roof_area / 50.0)).floor() as usize).min(10);
    for k in 0..units {
        let Some(at) = place(rng) else { continue };
        match k % 3 {
            0 => roof.block(at, [1.1, 0.9, 0.9], rng.next() * TAU, y),
            1 => roof.tank(at, 0.9, 2.2, y),
            _ => roof.block(at, [1.6, 0.5, 2.4], rng.next() * TAU, y),
        }
    }
    if roof_area >= 120.0 && params.clutter > 0.3 {
        if let Some(at) = place(rng) {
            roof.block(at, [3.0, 2.8, 3.0], 0.0, y);
        }
    }
    if params.height >= 60.0 && params.clutter > 0.3 {
        let c = outline.iter().fold([0.0, 0.0], |s, p| {
            [
                s[0] + p[0] / outline.len() as f32,
                s[1] + p[1] / outline.len() as f32,
            ]
        });
        roof.block(c, [0.2, 6.5, 0.2], 0.0, y);
    }
}
