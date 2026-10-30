//! Point sets and locally stable placement (ADR 0056). Written before the bodies; do not weaken.
#![expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::float_cmp,
    clippy::imprecise_flops,
    reason = "fixtures build fields from small indices and compare exact copies of the same computation"
)]
use ve_world::scatter::{
    Field2, PointSet, Poisson, Rule, ScatterError, Species, Variation, align_to_surface, cluster,
    filter, place_species, poisson, relax, vary,
};

const NEVER: &dyn Fn() -> bool = &|| false;

fn settings(area: [f32; 4]) -> Poisson {
    Poisson {
        seed: 11,
        radius: 5.0,
        density: 1.0,
        area,
        tries: 8,
    }
}

fn min_spacing(points: &PointSet) -> f32 {
    let p = points.positions();
    let mut best = f32::MAX;
    for i in 0..p.len() {
        for j in i + 1..p.len() {
            best = best.min(((p[i][0] - p[j][0]).powi(2) + (p[i][2] - p[j][2]).powi(2)).sqrt());
        }
    }
    best
}

fn sorted(points: &PointSet) -> Vec<[u32; 2]> {
    let mut keys: Vec<[u32; 2]> = points
        .positions()
        .iter()
        .map(|p| [p[0].to_bits(), p[2].to_bits()])
        .collect();
    keys.sort_unstable();
    keys
}

// ---------- the point set (implemented by the owner-side session) ----------

#[test]
fn channels_stay_aligned_through_retain_merge_and_names() {
    let mut set = PointSet::new(vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]]);
    set.set_channel("scale", vec![1.0, 2.0, 3.0]).unwrap();
    set.set_vector_channel("normal", vec![[0.0, 1.0, 0.0]; 3])
        .unwrap();
    assert_eq!(
        set.channel_names(),
        vec!["normal".to_string(), "scale".to_string()]
    );
    assert!(matches!(
        set.set_channel("bad", vec![1.0]),
        Err(ScatterError::LengthMismatch {
            expected: 3,
            got: 1,
            ..
        })
    ));
    assert!(matches!(
        set.set_channel("", vec![0.0; 3]),
        Err(ScatterError::BadChannel { .. })
    ));
    let mut kept = set.clone();
    kept.retain(&[true, false, true]);
    assert_eq!(kept.len(), 2);
    assert_eq!(kept.channel("scale").unwrap(), &[1.0, 3.0]);
    assert_eq!(kept.vector_channel("normal").unwrap().len(), 2);
    let mut joined = set.clone();
    joined.merge(&kept).unwrap();
    assert_eq!(joined.len(), 5);
    assert_eq!(joined.channel("scale").unwrap().len(), 5);
    let mut other = PointSet::new(vec![[9.0, 0.0, 0.0]]);
    other.set_channel("different", vec![0.0]).unwrap();
    assert!(set.merge(&other).is_err(), "channels must match");
}

#[test]
fn a_field_samples_bilinearly_in_world_coordinates() {
    let values = [0.0, 1.0, 0.0, 1.0];
    let field = Field2 {
        values: &values,
        width: 2,
        depth: 2,
        cell: 10.0,
        origin: [100.0, 50.0],
    };
    assert!((field.sample(105.0, 50.0) - 0.5).abs() < 1e-6);
    assert!(
        (field.sample(0.0, 0.0) - 0.0).abs() < 1e-6,
        "clamps at the edge"
    );
    assert!((field.sample(1000.0, 1000.0) - 1.0).abs() < 1e-6);
}

// ---------- poisson ----------

#[test]
fn points_keep_their_spacing_and_fill_the_area() {
    let points = poisson(&settings([0.0, 0.0, 100.0, 100.0]), None, None, NEVER).unwrap();
    assert!(
        min_spacing(&points) >= 5.0 * 0.999,
        "{}",
        min_spacing(&points)
    );
    // A hexagonal packing of radius 5 holds about 462 points in 100 x 100 m; blue noise reaches well over half of that.
    assert!(
        (250..=470).contains(&points.len()),
        "{} points",
        points.len()
    );
    assert!(
        points
            .positions()
            .iter()
            .all(|p| (0.0..100.0).contains(&p[0]) && (0.0..100.0).contains(&p[2]))
    );
}

#[test]
fn placement_is_deterministic_and_the_seed_matters() {
    let area = [0.0, 0.0, 60.0, 60.0];
    let a = poisson(&settings(area), None, None, NEVER).unwrap();
    assert_eq!(a, poisson(&settings(area), None, None, NEVER).unwrap());
    let mut other = settings(area);
    other.seed = 12;
    assert_ne!(
        sorted(&a),
        sorted(&poisson(&other, None, None, NEVER).unwrap())
    );
}

#[test]
fn tiles_cut_a_rectangle_without_duplicates_or_gaps() {
    let whole = poisson(&settings([0.0, 0.0, 100.0, 100.0]), None, None, NEVER).unwrap();
    let mut union = PointSet::default();
    for (x, z) in [(0.0, 0.0), (50.0, 0.0), (0.0, 50.0), (50.0, 50.0)] {
        union
            .merge(&poisson(&settings([x, z, x + 50.0, z + 50.0]), None, None, NEVER).unwrap())
            .unwrap();
    }
    assert_eq!(union.len(), whole.len());
    assert_eq!(
        sorted(&union),
        sorted(&whole),
        "the same points, bit for bit"
    );
    let mut keys = sorted(&union);
    keys.dedup();
    assert_eq!(keys.len(), union.len(), "no duplicates");
}

#[test]
fn shifting_by_whole_cells_keeps_the_shared_points() {
    let a = poisson(&settings([0.0, 0.0, 100.0, 100.0]), None, None, NEVER).unwrap();
    let b = poisson(&settings([25.0, 25.0, 125.0, 125.0]), None, None, NEVER).unwrap();
    let inside = |p: &&[f32; 3]| (25.0..100.0).contains(&p[0]) && (25.0..100.0).contains(&p[2]);
    let shared = |set: &PointSet| {
        let mut keys: Vec<[u32; 2]> = set
            .positions()
            .iter()
            .filter(inside)
            .map(|p| [p[0].to_bits(), p[2].to_bits()])
            .collect();
        keys.sort_unstable();
        keys
    };
    assert_eq!(shared(&a), shared(&b));
}

#[test]
fn a_mask_thins_the_points_where_it_is_low_and_a_change_stays_local() {
    // Left half of the world has full density, the right half a tenth.
    let mut values = vec![1.0_f32; 21 * 21];
    for j in 0..21 {
        for i in 11..21 {
            values[j * 21 + i] = 0.1;
        }
    }
    let mask = Field2 {
        values: &values,
        width: 21,
        depth: 21,
        cell: 5.0,
        origin: [0.0, 0.0],
    };
    let points = poisson(
        &settings([0.0, 0.0, 100.0, 100.0]),
        Some(&mask),
        None,
        NEVER,
    )
    .unwrap();
    let left = points.positions().iter().filter(|p| p[0] < 50.0).count();
    let right = points.len() - left;
    assert!(left > 3 * right.max(1), "left {left}, right {right}");
    // Changing the mask only in one corner moves no point farther than one radius outside it.
    let mut edited = values.clone();
    for j in 0..5 {
        for i in 0..5 {
            edited[j * 21 + i] = 0.0;
        }
    }
    let changed = Field2 {
        values: &edited,
        width: 21,
        depth: 21,
        cell: 5.0,
        origin: [0.0, 0.0],
    };
    let after = poisson(
        &settings([0.0, 0.0, 100.0, 100.0]),
        Some(&changed),
        None,
        NEVER,
    )
    .unwrap();
    let far = |p: &&[f32; 3]| p[0] > 20.0 + 5.0 || p[2] > 20.0 + 5.0;
    let keep = |set: &PointSet| {
        let mut keys: Vec<[u32; 2]> = set
            .positions()
            .iter()
            .filter(far)
            .map(|p| [p[0].to_bits(), p[2].to_bits()])
            .collect();
        keys.sort_unstable();
        keys
    };
    assert_eq!(
        keep(&points),
        keep(&after),
        "points away from the edit are untouched"
    );
}

#[test]
fn heights_set_the_y_of_each_point() {
    let values: Vec<f32> = (0..11 * 11).map(|k| (k % 11) as f32 * 2.0).collect();
    let heights = Field2 {
        values: &values,
        width: 11,
        depth: 11,
        cell: 10.0,
        origin: [0.0, 0.0],
    };
    let points = poisson(
        &settings([0.0, 0.0, 100.0, 100.0]),
        None,
        Some(&heights),
        NEVER,
    )
    .unwrap();
    assert!(
        points
            .positions()
            .iter()
            .all(|p| (p[1] - heights.sample(p[0], p[2])).abs() < 1e-3)
    );
    assert!(points.positions().iter().any(|p| p[1] > 10.0));
}

#[test]
fn bad_requests_and_cancellation_are_reported() {
    let mut bad = settings([0.0, 0.0, 10.0, 10.0]);
    bad.radius = 0.0;
    assert!(matches!(
        poisson(&bad, None, None, NEVER),
        Err(ScatterError::BadParameter { name: "radius", .. })
    ));
    let empty = settings([10.0, 10.0, 10.0, 20.0]);
    assert!(matches!(
        poisson(&empty, None, None, NEVER),
        Err(ScatterError::BadParameter { .. })
    ));
    let mut huge = settings([0.0, 0.0, 100_000.0, 100_000.0]);
    huge.radius = 1.0;
    let error = poisson(&huge, None, None, NEVER).unwrap_err();
    assert!(matches!(error, ScatterError::TooMany { .. }) && error.to_string().contains("spacing"));
    assert!(matches!(
        poisson(&settings([0.0, 0.0, 50.0, 50.0]), None, None, &|| true),
        Err(ScatterError::Cancelled)
    ));
}

#[test]
#[ignore = "timing: run with `cargo test -p ve-world --release -- --ignored`"]
fn two_hundred_thousand_points_take_seconds_at_most() {
    // Release builds: `cargo test -p ve-world --release -- --ignored`. 200,000 points in under 2 s.
    let mut big = settings([0.0, 0.0, 2000.0, 2000.0]);
    big.radius = 3.5;
    let start = std::time::Instant::now();
    let points = poisson(&big, None, None, NEVER).unwrap();
    let seconds = start.elapsed().as_secs_f32();
    assert!(
        points.len() > 150_000 && seconds < 2.0,
        "{} points in {seconds} s",
        points.len()
    );
}

// ---------- shaping ----------

#[test]
fn vary_fills_the_standard_channels_within_their_ranges() {
    let mut points = poisson(&settings([0.0, 0.0, 60.0, 60.0]), None, None, NEVER).unwrap();
    let params = Variation {
        seed: 3,
        scale: [0.8, 1.4],
        species: 7,
        age: [0.1, 0.9],
    };
    vary(&mut points, &params);
    for (name, lo, hi) in [
        ("scale", 0.8, 1.4),
        ("age", 0.1, 0.9),
        ("yaw", 0.0, std::f32::consts::TAU),
    ] {
        let values = points
            .channel(name)
            .unwrap_or_else(|| panic!("channel {name}"));
        assert!(values.iter().all(|v| (lo..=hi).contains(v)), "{name}");
        assert!(
            values.iter().any(|v| (v - values[0]).abs() > 1e-4),
            "{name} varies"
        );
    }
    assert!(
        points
            .channel("id")
            .unwrap()
            .iter()
            .all(|v| (*v - 7.0).abs() < 1e-6)
    );
    let mut again = poisson(&settings([0.0, 0.0, 60.0, 60.0]), None, None, NEVER).unwrap();
    vary(&mut again, &params);
    assert_eq!(points, again, "deterministic");
}

#[test]
fn alignment_reads_the_normal_and_tilt_from_the_height_field() {
    let flat = vec![0.0; 11 * 11];
    let field = Field2 {
        values: &flat,
        width: 11,
        depth: 11,
        cell: 10.0,
        origin: [0.0, 0.0],
    };
    let mut points = PointSet::new(vec![[50.0, 0.0, 50.0]]);
    align_to_surface(&mut points, &field, 1.0);
    let n = points.vector_channel("normal").unwrap()[0];
    assert!((n[1] - 1.0).abs() < 1e-5 && n[0].abs() < 1e-5);
    assert!(points.channel("tilt").unwrap()[0].abs() < 1e-5);
    let ramp: Vec<f32> = (0..11 * 11).map(|k| (k % 11) as f32 * 10.0).collect();
    let slope = Field2 {
        values: &ramp,
        width: 11,
        depth: 11,
        cell: 10.0,
        origin: [0.0, 0.0],
    };
    let mut tilted = PointSet::new(vec![[50.0, 50.0, 50.0]]);
    align_to_surface(&mut tilted, &slope, 1.0);
    let tilt = tilted.channel("tilt").unwrap()[0];
    assert!(
        (tilt - 0.5).abs() < 0.02,
        "a 45 degree slope tilts by half of 90: {tilt}"
    );
}

#[test]
fn filter_keeps_a_band_of_a_channel_and_refuses_bad_input() {
    let mut points = PointSet::new((0..10).map(|k| [k as f32, 0.0, 0.0]).collect());
    points
        .set_channel("h", (0..10).map(|k| k as f32).collect())
        .unwrap();
    filter(&mut points, "h", 3.0, 6.0).unwrap();
    assert_eq!(points.len(), 4);
    assert_eq!(points.channel("h").unwrap(), &[3.0, 4.0, 5.0, 6.0]);
    assert!(matches!(
        filter(&mut points, "nope", 0.0, 1.0),
        Err(ScatterError::BadChannel { .. })
    ));
    assert!(matches!(
        filter(&mut points, "h", 5.0, 1.0),
        Err(ScatterError::BadParameter { .. })
    ));
}

#[test]
fn relaxing_does_not_squeeze_points_together() {
    let mut points = poisson(&settings([0.0, 0.0, 60.0, 60.0]), None, None, NEVER).unwrap();
    let before = min_spacing(&points);
    let count = points.len();
    relax(&mut points, 6.0, 5, NEVER).unwrap();
    assert_eq!(points.len(), count);
    assert!(
        min_spacing(&points) >= before - 1e-3,
        "{} against {before}",
        min_spacing(&points)
    );
    assert!(points.positions().iter().all(|p| (-1e-3..=60.0 + 1e-3).contains(&p[0]) && (-1e-3..=60.0 + 1e-3).contains(&p[2])));
    assert!(matches!(
        relax(&mut points, 6.0, 5, &|| true),
        Err(ScatterError::Cancelled)
    ));
}

#[test]
fn clusters_gather_children_around_each_parent() {
    let parents = PointSet::new(vec![
        [10.0, 0.0, 10.0],
        [50.0, 0.0, 50.0],
        [90.0, 0.0, 20.0],
    ]);
    let children = cluster(5, &parents, [3, 6], 4.0, NEVER).unwrap();
    assert!((9..=18).contains(&children.len()), "{}", children.len());
    let owner = children.channel("parent").unwrap();
    for (child, parent) in children.positions().iter().zip(owner) {
        let p = parents.positions()[*parent as usize];
        assert!(((child[0] - p[0]).powi(2) + (child[2] - p[2]).powi(2)).sqrt() <= 4.0 + 1e-3);
    }
    assert_eq!(children, cluster(5, &parents, [3, 6], 4.0, NEVER).unwrap());
    assert!(matches!(
        cluster(5, &parents, [6, 3], 4.0, NEVER),
        Err(ScatterError::BadParameter { .. })
    ));
}

// ---------- species ----------

fn moisture() -> Vec<f32> {
    // Dry on the left (x = 0), wet on the right (x = 100).
    (0..21 * 21).map(|k| (k % 21) as f32 / 20.0).collect()
}

fn species(id: u32, lo: f32, hi: f32) -> Species {
    Species {
        id,
        seed: 100 + id,
        radius: 4.0,
        density: 1.0,
        rules: vec![Rule {
            channel: "moisture".into(),
            lo,
            hi,
            soft: 0.1,
            weight: 1.0,
        }],
    }
}

#[test]
fn species_grow_where_their_rules_say() {
    let values = moisture();
    let field = Field2 {
        values: &values,
        width: 21,
        depth: 21,
        cell: 5.0,
        origin: [0.0, 0.0],
    };
    let channels = [("moisture", field)];
    let both = [species(1, 0.0, 0.45), species(2, 0.55, 1.0)];
    let points = place_species(&both, [0.0, 0.0, 100.0, 100.0], &channels, None, NEVER).unwrap();
    let ids = points.channel("id").unwrap();
    assert!(points.len() > 100);
    for (p, id) in points.positions().iter().zip(ids) {
        let m = field.sample(p[0], p[2]);
        if *id == 1.0 {
            assert!(m < 0.6, "dry species at moisture {m}");
        } else {
            assert!(m > 0.4, "wet species at moisture {m}");
        }
    }
    assert!(ids.contains(&1.0) && ids.contains(&2.0));
    let wrong = [Species {
        rules: vec![Rule {
            channel: "nope".into(),
            lo: 0.0,
            hi: 1.0,
            soft: 0.0,
            weight: 1.0,
        }],
        ..species(3, 0.0, 1.0)
    }];
    assert!(matches!(
        place_species(&wrong, [0.0, 0.0, 50.0, 50.0], &channels, None, NEVER),
        Err(ScatterError::BadChannel { .. })
    ));
}

#[test]
fn adding_a_species_that_never_wins_changes_nothing_and_tiles_agree() {
    let values = moisture();
    let field = Field2 {
        values: &values,
        width: 21,
        depth: 21,
        cell: 5.0,
        origin: [0.0, 0.0],
    };
    let channels = [("moisture", field)];
    let base = [species(1, 0.0, 0.45), species(2, 0.55, 1.0)];
    let mut with_extra = base.to_vec();
    with_extra.push(Species {
        rules: vec![Rule {
            channel: "moisture".into(),
            lo: 2.0,
            hi: 3.0,
            soft: 0.0,
            weight: 1.0,
        }],
        ..species(9, 2.0, 3.0)
    });
    let area = [0.0, 0.0, 100.0, 100.0];
    let a = place_species(&base, area, &channels, None, NEVER).unwrap();
    let b = place_species(&with_extra, area, &channels, None, NEVER).unwrap();
    assert_eq!(sorted(&a), sorted(&b));
    let mut union = PointSet::default();
    for (x, z) in [(0.0, 0.0), (50.0, 0.0), (0.0, 50.0), (50.0, 50.0)] {
        union
            .merge(
                &place_species(&base, [x, z, x + 50.0, z + 50.0], &channels, None, NEVER).unwrap(),
            )
            .unwrap();
    }
    assert_eq!(
        sorted(&union),
        sorted(&a),
        "tiles of a species placement agree with the whole"
    );
}
