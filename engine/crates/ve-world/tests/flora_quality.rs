//! Whether the named species read as the trees they are named for: measurable crown and trunk proportions at 10 m (added after the first
//! render of the presets looked like saplings with a few leaves). Written before the fix; do not weaken.
#![expect(
    clippy::unwrap_used,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    reason = "fixtures unwrap valid presets and count points into small grid cells and azimuth sectors"
)]
use ve_world::flora::{NAMES, Preset, Skeleton, colonize, grow_tree, preset};

const NEVER: &dyn Fn() -> bool = &|| false;

fn plant(name: &str) -> Skeleton {
    match preset(name, 10.0, 4).unwrap() {
        Preset::Tree(tree) => grow_tree(&tree, NEVER).unwrap(),
        Preset::Colonize(setup) => colonize(&setup, NEVER).unwrap(),
    }
}

struct Crown {
    leaves: usize,
    base: f32,
    radius: f32,
    top: f32,
    fill: f32,
}

/// Leaf count, where the leaves start above the ground, their widest horizontal reach, their top, and how much of the crown's box
/// (an 8 by 8 by 8 grid over it) holds at least one leaf point.
fn crown(skeleton: &Skeleton) -> Crown {
    let points: Vec<[f32; 3]> = skeleton.leaves.iter().map(|l| l.position).collect();
    let lo = points.iter().fold([f32::MAX; 3], |m, p| {
        [m[0].min(p[0]), m[1].min(p[1]), m[2].min(p[2])]
    });
    let hi = points.iter().fold([f32::MIN; 3], |m, p| {
        [m[0].max(p[0]), m[1].max(p[1]), m[2].max(p[2])]
    });
    let mut cells = std::collections::HashSet::new();
    for p in &points {
        let cell =
            |axis: usize| (((p[axis] - lo[axis]) / (hi[axis] - lo[axis]).max(1e-3)) * 7.999) as i32;
        cells.insert((cell(0), cell(1), cell(2)));
    }
    Crown {
        leaves: points.len(),
        base: lo[1],
        radius: points
            .iter()
            .map(|p| (p[0] * p[0] + p[2] * p[2]).sqrt())
            .fold(0.0, f32::max),
        top: hi[1],
        fill: cells.len() as f32 / 512.0,
    }
}

#[test]
fn broadleaf_trees_have_full_crowns_above_a_clear_trunk() {
    for (name, min_leaves, min_radius, min_base) in [
        ("oak", 900, 3.0, 2.0),
        ("birch", 500, 1.8, 2.5),
        ("willow", 700, 2.8, 1.5),
    ] {
        let c = crown(&plant(name));
        let summary = format!(
            "{name}: {} leaf points, base {:.1} m, radius {:.1} m, top {:.1} m, fill {:.2}",
            c.leaves, c.base, c.radius, c.top, c.fill
        );
        assert!(c.leaves >= min_leaves, "too few leaf points: {summary}");
        assert!(c.radius >= min_radius, "the crown is too narrow: {summary}");
        assert!(
            c.base >= min_base,
            "the leaves start too low (no trunk): {summary}"
        );
        assert!(
            c.top >= 7.5 && c.top <= 11.5,
            "the crown tops out at the tree's height: {summary}"
        );
        assert!(c.fill >= 0.15, "the crown is hollow: {summary}");
    }
}

#[test]
fn a_pine_is_a_tall_narrow_cone_and_a_shrub_is_low_and_bushy() {
    let pine = crown(&plant("pine"));
    let summary = format!(
        "pine: {} leaf points, base {:.1} m, radius {:.1} m, top {:.1} m, fill {:.2}",
        pine.leaves, pine.base, pine.radius, pine.top, pine.fill
    );
    assert!(
        pine.leaves >= 600 && pine.base >= 1.5 && pine.top >= 8.5,
        "{summary}"
    );
    assert!(
        pine.radius >= 1.4 && pine.radius <= 3.8,
        "a pine is narrow: {summary}"
    );
    let shrub = crown(&preset_shrub());
    let summary = format!(
        "shrub at 2 m: {} leaf points, base {:.2} m, radius {:.2} m, top {:.2} m, fill {:.2}",
        shrub.leaves, shrub.base, shrub.radius, shrub.top, shrub.fill
    );
    assert!(
        shrub.leaves >= 120 && shrub.base <= 0.5 && shrub.radius >= 0.6,
        "{summary}"
    );
}

fn preset_shrub() -> Skeleton {
    match preset("shrub", 2.0, 4).unwrap() {
        Preset::Tree(tree) => grow_tree(&tree, NEVER).unwrap(),
        Preset::Colonize(setup) => colonize(&setup, NEVER).unwrap(),
    }
}

/// The share of leaf points in each of eight azimuth sectors around the trunk axis, and how far the crown's centre sits from the trunk.
fn balance(skeleton: &Skeleton) -> (Vec<f32>, f32) {
    let points: Vec<[f32; 3]> = skeleton.leaves.iter().map(|l| l.position).collect();
    let mut sectors = vec![0.0_f32; 8];
    for p in &points {
        let angle = p[2].atan2(p[0]).rem_euclid(std::f32::consts::TAU);
        sectors[((angle / std::f32::consts::TAU) * 8.0) as usize % 8] += 1.0;
    }
    let total = points.len() as f32;
    let centre = [
        points.iter().map(|p| p[0]).sum::<f32>() / total,
        points.iter().map(|p| p[2]).sum::<f32>() / total,
    ];
    let reach = points
        .iter()
        .map(|p| (p[0] * p[0] + p[2] * p[2]).sqrt())
        .fold(0.0, f32::max);
    (
        sectors.into_iter().map(|s| s / total).collect(),
        (centre[0] * centre[0] + centre[1] * centre[1]).sqrt() / reach.max(0.1),
    )
}

#[test]
fn crowns_are_balanced_around_the_trunk_not_thrown_to_one_side() {
    for name in ["oak", "pine", "birch", "willow", "shrub"] {
        let skeleton = if name == "shrub" {
            preset_shrub()
        } else {
            plant(name)
        };
        let (sectors, offset) = balance(&skeleton);
        let lightest = sectors.iter().copied().fold(f32::MAX, f32::min);
        assert!(
            lightest >= 0.05,
            "{name}: a side of the crown is nearly empty, sector shares {sectors:?}"
        );
        assert!(
            offset <= 0.25,
            "{name}: the crown's centre sits {offset:.2} of its reach away from the trunk"
        );
    }
}

#[test]
fn a_pine_is_a_cone_with_branches_all_the_way_up() {
    let skeleton = plant("pine");
    let points: Vec<[f32; 3]> = skeleton.leaves.iter().map(|l| l.position).collect();
    let (lo, hi) = (
        points.iter().fold(f32::MAX, |m, p| m.min(p[1])),
        points.iter().fold(f32::MIN, |m, p| m.max(p[1])),
    );
    let band = |i: usize| -> (usize, f32) {
        let (from, to) = (
            lo + (hi - lo) * i as f32 / 4.0,
            lo + (hi - lo) * (i + 1) as f32 / 4.0,
        );
        let inside: Vec<&[f32; 3]> = points
            .iter()
            .filter(|p| p[1] >= from && (p[1] < to || i == 3))
            .collect();
        let reach = inside
            .iter()
            .map(|p| (p[0] * p[0] + p[2] * p[2]).sqrt())
            .sum::<f32>()
            / inside.len().max(1) as f32;
        (inside.len(), reach)
    };
    let bands: Vec<(usize, f32)> = (0..4).map(band).collect();
    assert!(
        bands
            .iter()
            .all(|(count, _)| *count as f32 >= points.len() as f32 * 0.10),
        "every height has branches: {bands:?}"
    );
    assert!(
        bands[0].1 > bands[3].1 * 1.5,
        "the lowest branches are much longer than the top ones (a cone): {bands:?}"
    );
}

#[test]
fn trunks_are_thick_enough_and_taper_toward_the_top() {
    for name in NAMES {
        let skeleton = plant(name);
        let base = skeleton.radius[0];
        // The trunk base is between 1.5 and 6 percent of the height, except the shrub (many thin stems) and the palm.
        let (lo, hi) = match name {
            "shrub" => (0.003, 0.05),
            "birch" => (0.012, 0.05),
            _ => (0.015, 0.06),
        };
        assert!(
            (lo * 10.0..hi * 10.0).contains(&base),
            "{name} trunk radius {base} m on a 10 m plant"
        );
        let top = skeleton.radius[skeleton
            .position
            .iter()
            .enumerate()
            .max_by(|a, b| a.1[1].total_cmp(&b.1[1]))
            .unwrap()
            .0];
        assert!(
            top < base * 0.8,
            "{name} tapers: {top} at the top against {base}"
        );
    }
}

#[test]
fn the_species_have_distinct_crown_shapes() {
    let shapes: Vec<(f32, f32)> = ["oak", "pine", "willow", "birch"]
        .iter()
        .map(|n| {
            let c = crown(&plant(n));
            (c.radius / (c.top - c.base).max(0.1), c.base)
        })
        .collect();
    // The oak is broad (radius over crown height high), the pine and the birch are narrow.
    assert!(
        shapes[0].0 > shapes[1].0 * 1.4,
        "an oak is broader than a pine: {shapes:?}"
    );
    assert!(
        shapes[0].0 > shapes[3].0 * 1.2,
        "an oak is broader than a birch: {shapes:?}"
    );
}
