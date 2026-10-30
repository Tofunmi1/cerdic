//! Rivers, roads, shores and climate on a height grid (ADR 0056). Written before the bodies; do not weaken.
#![expect(
    clippy::cast_precision_loss,
    clippy::float_cmp,
    reason = "fixtures build grids from small indices and compare exact copies of the same computation"
)]
use ve_world::river::{
    Climate, Ground, GroundRef, Path, RiverError, RiverProfile, RoadProfile, carve_river,
    carve_road, moisture, shore_masks, temperature,
};

const NEVER: &dyn Fn() -> bool = &|| false;
const W: usize = 129;
const D: usize = 129;

/// A plateau at 50 m that falls 0.04 m per metre toward +x, with a gentle ripple across z, on a 1 m grid whose cell (0,0) is at `origin`.
fn land(origin: [f32; 2], width: usize, depth: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(width * depth);
    for j in 0..depth {
        for i in 0..width {
            let (x, z) = (origin[0] + i as f32, origin[1] + j as f32);
            out.push(50.0 - 0.04 * x + 0.3 * (z * 0.2).sin());
        }
    }
    out
}

fn ground(heights: &mut [f32], width: usize, depth: usize, origin: [f32; 2]) -> Ground<'_> {
    Ground {
        heights,
        width,
        depth,
        cell: 1.0,
        origin,
    }
}

fn view(heights: &[f32], width: usize, depth: usize) -> GroundRef<'_> {
    GroundRef {
        heights,
        width,
        depth,
        cell: 1.0,
        origin: [0.0, 0.0],
    }
}

fn river() -> RiverProfile {
    RiverProfile {
        width: 8.0,
        depth: 3.0,
        bank_slope_deg: 35.0,
        bank_width: 6.0,
        meander_flatten: 0.3,
        water_level_offset: 0.5,
    }
}

fn straight() -> Path {
    Path {
        points: vec![[10.0, 64.0], [60.0, 64.0], [118.0, 64.0]],
    }
}

// ---------- rivers ----------

#[test]
fn a_river_cuts_a_channel_and_reports_water_and_flow() {
    let original = land([0.0, 0.0], W, D);
    let mut heights = original.clone();
    let out = carve_river(
        &mut ground(&mut heights, W, D, [0.0, 0.0]),
        &straight(),
        &river(),
        NEVER,
    )
    .unwrap();
    let at = |x: usize, z: usize| z * W + x;
    // The bed at the centre is about `depth` below the ground it replaced.
    for x in [20, 40, 60, 80, 100] {
        let cut = original[at(x, 64)] - heights[at(x, 64)];
        assert!(
            (2.7..4.5).contains(&cut),
            "bed {cut} m below the old ground at x = {x}"
        );
    }
    // The bed never rises going downstream here, because the ground falls that way.
    let bed: Vec<f32> = (12..=116).map(|x| heights[at(x, 64)]).collect();
    assert!(
        bed.windows(2).all(|pair| pair[1] <= pair[0] + 0.05),
        "the bed runs downhill"
    );
    // Far from the channel (beyond half the width and the banks) nothing changed.
    for (x, z) in [(60, 64 + 12), (60, 64 - 12), (60, 5), (5, 5)] {
        assert_eq!(
            heights[at(x, z)],
            original[at(x, z)],
            "untouched at {x},{z}"
        );
    }
    // Water sits `water_level_offset` below the old bank top, above the bed, and is the ground elsewhere.
    let wet = out.water_height[at(60, 64)];
    assert!(
        (wet - (original[at(60, 64)] - 0.5)).abs() < 0.35,
        "water at {wet}"
    );
    assert!(wet > heights[at(60, 64)] + 1.5);
    assert_eq!(out.water_height[at(60, 5)], heights[at(60, 5)]);
    // Flow follows the path, only inside the channel.
    assert!(out.flow_dir[at(60, 64)][0] > 0.95 && out.flow_dir[at(60, 64)][1].abs() < 0.3);
    assert!(out.flow_speed[at(60, 64)] > 0.0 && out.flow_speed[at(60, 5)] == 0.0);
    assert_eq!(out.flow_dir[at(60, 5)], [0.0, 0.0]);
    // Masks: the channel is 1 at the centre, 0 far away; the bank mask peaks beside the channel.
    assert!(out.channel_mask[at(60, 64)] > 0.95 && out.channel_mask[at(60, 5)] == 0.0);
    assert!(out.bank_mask[at(60, 64 + 6)] > out.bank_mask[at(60, 64 + 20)]);
    assert!(
        [&out.channel_mask, &out.bank_mask, &out.flow_speed]
            .iter()
            .all(|m| m.iter().all(|v| v.is_finite() && *v >= 0.0))
    );
}

#[test]
fn a_river_depends_only_on_the_ground_near_it_so_tiles_agree() {
    let whole_ground = land([0.0, 0.0], W, D);
    let mut whole = whole_ground.clone();
    carve_river(
        &mut ground(&mut whole, W, D, [0.0, 0.0]),
        &straight(),
        &river(),
        NEVER,
    )
    .unwrap();
    // A window 70 cells wide starting at x = 30 carves the same cells away from its own edge (the channel reaches 4 + 6 = 10 m).
    let (x0, width) = (30usize, 70usize);
    let mut window = Vec::new();
    for j in 0..D {
        window.extend_from_slice(&whole_ground[j * W + x0..j * W + x0 + width]);
    }
    carve_river(
        &mut ground(&mut window, width, D, [x0 as f32, 0.0]),
        &straight(),
        &river(),
        NEVER,
    )
    .unwrap();
    for j in 0..D {
        for i in 12..width - 12 {
            let (a, b) = (whole[j * W + x0 + i], window[j * width + i]);
            assert!((a - b).abs() < 1e-4, "cell {} {j}: {a} against {b}", x0 + i);
        }
    }
}

#[test]
fn a_river_refuses_bad_paths_and_parameters() {
    let mut heights = land([0.0, 0.0], W, D);
    let mut g = ground(&mut heights, W, D, [0.0, 0.0]);
    assert!(matches!(
        carve_river(
            &mut g,
            &Path {
                points: vec![[5.0, 5.0]]
            },
            &river(),
            NEVER
        ),
        Err(RiverError::BadPath { .. })
    ));
    assert!(matches!(
        carve_river(
            &mut g,
            &Path {
                points: vec![[5.0, 5.0], [5.0, 5.0]]
            },
            &river(),
            NEVER
        ),
        Err(RiverError::BadPath { .. })
    ));
    assert!(matches!(
        carve_river(
            &mut g,
            &Path {
                points: vec![[500.0, 500.0], [600.0, 500.0]]
            },
            &river(),
            NEVER
        ),
        Err(RiverError::BadPath { .. })
    ));
    let mut narrow = river();
    narrow.width = 0.0;
    assert!(matches!(
        carve_river(&mut g, &straight(), &narrow, NEVER),
        Err(RiverError::BadParameter { name: "width", .. })
    ));
    let mut steep = river();
    steep.bank_slope_deg = 95.0;
    assert!(matches!(
        carve_river(&mut g, &straight(), &steep, NEVER),
        Err(RiverError::BadParameter {
            name: "bank_slope_deg",
            ..
        })
    ));
    assert!(matches!(
        carve_river(&mut g, &straight(), &river(), &|| true),
        Err(RiverError::Cancelled)
    ));
    let mut short = vec![0.0; 10];
    assert!(matches!(
        carve_river(
            &mut ground(&mut short, W, D, [0.0, 0.0]),
            &straight(),
            &river(),
            NEVER
        ),
        Err(RiverError::BadShape { .. })
    ));
}

// ---------- roads ----------

fn road() -> RoadProfile {
    RoadProfile {
        width: 5.0,
        shoulder: 2.0,
        max_grade_percent: 6.0,
        crown: 0.1,
        embank_slope_deg: 30.0,
    }
}

/// A ramp rising `percent` per 100 m along x, from 20 m.
fn ramp(percent: f32) -> Vec<f32> {
    (0..W * D)
        .map(|k| 20.0 + percent / 100.0 * (k % W) as f32)
        .collect()
}

#[test]
fn a_road_follows_gentle_ground_and_limits_its_grade_on_steep_ground() {
    let road_path = Path {
        points: vec![[8.0, 64.0], [120.0, 64.0]],
    };
    // Gentle: 3 percent, under the limit; the road lies on the ground.
    let mut gentle = ramp(3.0);
    let original = gentle.clone();
    let out = carve_road(
        &mut ground(&mut gentle, W, D, [0.0, 0.0]),
        &road_path,
        &road(),
        NEVER,
    )
    .unwrap();
    for x in [20, 60, 100] {
        assert!(
            (gentle[64 * W + x] - original[64 * W + x]).abs() < 0.35,
            "on gentle ground the road stays at {x}"
        );
    }
    assert!(
        out.road_mask[64 * W + 60] > 0.95 && out.road_mask[64 * W + 5] == 0.0
            || out.road_mask[5 * W + 60] == 0.0
    );
    // Steep: 25 percent. The road climbs no more than 6 percent per metre of path, cutting into the hill.
    let mut steep = ramp(25.0);
    let before = steep.clone();
    let out = carve_road(
        &mut ground(&mut steep, W, D, [0.0, 0.0]),
        &road_path,
        &road(),
        NEVER,
    )
    .unwrap();
    let profile: Vec<f32> = (10..=118).map(|x| steep[64 * W + x]).collect();
    assert!(
        profile
            .windows(2)
            .all(|pair| (pair[1] - pair[0]).abs() <= 0.06 + 1e-3),
        "grade over 6 percent"
    );
    assert!(out.cut_fill > 50.0, "earth was moved: {} m3", out.cut_fill);
    assert!(steep.iter().zip(&before).any(|(a, b)| (a - b).abs() > 1.0));
    assert!(out.shoulder_mask.iter().any(|v| *v > 0.5) && steep.iter().all(|v| v.is_finite()));
    assert!(
        [&out.road_mask, &out.shoulder_mask]
            .iter()
            .all(|m| m.iter().all(|v| (0.0..=1.0).contains(v)))
    );
}

#[test]
fn a_road_refuses_bad_parameters() {
    let mut heights = ramp(3.0);
    let mut g = ground(&mut heights, W, D, [0.0, 0.0]);
    let path = Path {
        points: vec![[8.0, 64.0], [120.0, 64.0]],
    };
    let mut too_steep = road();
    too_steep.max_grade_percent = 55.0;
    assert!(matches!(
        carve_road(&mut g, &path, &too_steep, NEVER),
        Err(RiverError::BadParameter {
            name: "max_grade_percent",
            ..
        })
    ));
    let mut thin = road();
    thin.width = -1.0;
    assert!(matches!(
        carve_road(&mut g, &path, &thin, NEVER),
        Err(RiverError::BadParameter { name: "width", .. })
    ));
    assert!(matches!(
        carve_road(&mut g, &Path::default(), &road(), NEVER),
        Err(RiverError::BadPath { .. })
    ));
    assert!(matches!(
        carve_road(&mut g, &path, &road(), &|| true),
        Err(RiverError::Cancelled)
    ));
}

// ---------- shores ----------

#[test]
fn shore_masks_follow_the_water_level() {
    // Heights rise 0.1 m per cell along x: 0 at x = 0 to 12.8 at x = 128. Water at 5 m is at x = 50.
    let heights: Vec<f32> = (0..W * D).map(|k| 0.1 * (k % W) as f32).collect();
    let out = shore_masks(&view(&heights, W, D), 5.0, 2.0, NEVER).unwrap();
    let at = |x: usize| 64 * W + x;
    assert_eq!(out.underwater[at(10)], 1.0);
    assert_eq!(out.underwater[at(90)], 0.0);
    assert!((out.depth_below[at(0)] - 5.0).abs() < 1e-4 && out.depth_below[at(90)] == 0.0);
    assert!(
        out.beach[at(52)] > 0.8,
        "sand at the water line: {}",
        out.beach[at(52)]
    );
    assert!(
        out.beach[at(60)] < out.beach[at(52)] && out.beach[at(80)] == 0.0,
        "sand fades with height"
    );
    assert!(
        out.wet_rock.iter().all(|v| *v == 0.0),
        "a gentle ramp has no wet rock"
    );
    // A cliff that meets the water has wet rock at its foot and not far above it.
    let cliff: Vec<f32> = (0..W * D)
        .map(|k| if k % W < 64 { 0.0 } else { 20.0 })
        .collect();
    let rock = shore_masks(&view(&cliff, W, D), 1.0, 2.0, NEVER).unwrap();
    assert!(
        rock.wet_rock[64 * W + 64] > 0.5,
        "wet rock at the cliff foot"
    );
    assert_eq!(rock.wet_rock[64 * W + 10], 0.0);
    assert!(shore_masks(&view(&heights, W, D), 5.0, -1.0, NEVER).is_err());
    assert!(matches!(
        shore_masks(&view(&heights, W, D), 5.0, 2.0, &|| true),
        Err(RiverError::Cancelled)
    ));
}

// ---------- climate ----------

fn climate() -> Climate {
    Climate {
        sea_level: 0.0,
        lapse_per_km: 6.5,
        base_temperature: 15.0,
        latitude_gradient: 2.0,
        moisture_distance: 20.0,
    }
}

#[test]
fn temperature_falls_with_height_and_toward_the_pole() {
    let heights: Vec<f32> = (0..W * D)
        .map(|k| if (k % W) < 64 { 0.0 } else { 1000.0 })
        .collect();
    let t = temperature(&view(&heights, W, D), &climate());
    assert_eq!(t.len(), W * D);
    assert!((t[0] - 15.0).abs() < 1e-3, "sea level at z = 0: {}", t[0]);
    assert!(
        (t[100] - 8.5).abs() < 1e-3,
        "1000 m up loses 6.5 degrees: {}",
        t[100]
    );
    // 100 m toward +z at 2 degrees per 1000 m.
    assert!((t[100 * W] - (15.0 - 0.2)).abs() < 1e-3, "{}", t[100 * W]);
}

#[test]
fn moisture_is_one_on_water_and_fades_to_zero_with_distance() {
    let heights = vec![0.0; W * D];
    let mut water = vec![0.0; W * D];
    for j in 0..D {
        water[j * W + 10] = 1.0;
    }
    let m = moisture(&view(&heights, W, D), &water, &climate());
    let at = |x: usize| 64 * W + x;
    assert!(m[at(10)] > 0.99);
    assert!(
        (0..W - 11)
            .map(|d| m[at(10 + d)])
            .collect::<Vec<_>>()
            .windows(2)
            .all(|pair| pair[1] <= pair[0] + 1e-6),
        "never rises with distance"
    );
    assert!(
        m[at(10 + 20)] < 0.01 && m[at(10 + 8)] > 0.1,
        "{} {}",
        m[at(30)],
        m[at(18)]
    );
    assert!(
        moisture(&view(&heights, W, D), &vec![0.0; W * D], &climate())
            .iter()
            .all(|v| *v == 0.0),
        "no water, no moisture"
    );
    assert!(m.iter().all(|v| (0.0..=1.0).contains(v)));
}
