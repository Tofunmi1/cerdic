//! Procedural plant skeletons (ADR 0056). Written before the bodies; do not weaken.
use ve_world::flora::{
    Colonize, Crown, FloraError, Level, NAMES, Preset, Skeleton, Tree, Tuft, colonize, grow_tree,
    grow_tuft, preset,
};

const NEVER: &dyn Fn() -> bool = &|| false;

fn oak(seed: u32) -> Tree {
    let level = |count, length, down| Level {
        count,
        length,
        length_variation: 0.2,
        radius_ratio: 0.65,
        taper: 0.8,
        down_angle_deg: down,
        down_variation_deg: 10.0,
        rotate_deg: 137.5,
        rotate_variation_deg: 15.0,
        curve_deg: 25.0,
        segments: 6,
        start: 0.3,
        gravity: 0.1,
    };
    Tree {
        seed,
        height: 10.0,
        trunk_radius: 0.4,
        levels: vec![
            level(1, 1.0, 0.0),
            level(7, 0.55, 55.0),
            level(5, 0.4, 45.0),
        ],
        leaf_size: 0.12,
        leaves_per_tip: 3,
        leaf_depth_from: 2,
    }
}

fn crown(points: u32) -> Colonize {
    Colonize {
        seed: 3,
        trunk_height: 2.5,
        trunk_radius: 0.25,
        crown: Crown::Ellipsoid {
            center: [0.0, 6.0, 0.0],
            radii: [3.5, 3.0, 3.5],
        },
        points,
        attraction_radius: 1.8,
        kill_distance: 0.4,
        step: 0.25,
        radius_exponent: 2.0,
        max_iterations: 200,
    }
}

fn children(skeleton: &Skeleton) -> Vec<Vec<usize>> {
    let mut out = vec![Vec::new(); skeleton.node_count()];
    for (node, parent) in skeleton.parent.iter().enumerate() {
        if *parent != u32::MAX {
            out[*parent as usize].push(node);
        }
    }
    out
}

/// The structural rules every skeleton must follow.
fn assert_valid(skeleton: &Skeleton, what: &str) {
    let n = skeleton.node_count();
    assert!(n > 10, "{what}: {n} nodes");
    for len in [
        skeleton.radius.len(),
        skeleton.parent.len(),
        skeleton.depth.len(),
        skeleton.wind.len(),
        skeleton.phase.len(),
        skeleton.stiffness.len(),
        skeleton.branch.len(),
    ] {
        assert_eq!(len, n, "{what}: parallel arrays");
    }
    assert_eq!(
        skeleton.parent.iter().filter(|p| **p == u32::MAX).count(),
        1,
        "{what}: exactly one root"
    );
    assert_eq!(skeleton.parent[0], u32::MAX, "{what}: node 0 is the root");
    assert!(
        skeleton.position[0].iter().all(|v| v.abs() < 1e-4),
        "{what}: the root stands at the origin"
    );
    for node in 1..n {
        assert!(
            (skeleton.parent[node] as usize) < node,
            "{what}: a parent comes before its children (node {node})"
        );
        assert!(
            skeleton.radius[node] <= skeleton.radius[skeleton.parent[node] as usize] + 1e-5,
            "{what}: radius never grows away from the base (node {node})"
        );
    }
    for node in 0..n {
        assert!(
            skeleton.radius[node] > 0.0 && skeleton.radius[node].is_finite(),
            "{what}: positive radius"
        );
        assert!(
            skeleton.position[node].iter().all(|v| v.is_finite()),
            "{what}: finite position"
        );
        for value in [
            skeleton.wind[node],
            skeleton.phase[node],
            skeleton.stiffness[node],
        ] {
            assert!(
                (0.0..=1.0).contains(&value),
                "{what}: wind data in 0..=1, got {value}"
            );
        }
    }
    for leaf in &skeleton.leaves {
        assert!(
            (leaf.node as usize) < n,
            "{what}: a leaf grows from a real node"
        );
        let length = leaf.direction.iter().map(|v| v * v).sum::<f32>().sqrt();
        assert!(
            (length - 1.0).abs() < 1e-3 && leaf.size > 0.0,
            "{what}: a unit direction and a size"
        );
    }
    // Every node of one branch shares its gust phase.
    for chain in skeleton.polylines() {
        let first = chain.iter().find(|&&node| {
            skeleton.branch[node as usize] == skeleton.branch[chain[chain.len() - 1] as usize]
        });
        if let Some(&first) = first {
            let id = skeleton.branch[first as usize];
            let phases: Vec<f32> = (0..n)
                .filter(|&k| skeleton.branch[k] == id)
                .map(|k| skeleton.phase[k])
                .collect();
            assert!(
                phases.iter().all(|p| (*p - phases[0]).abs() < 1e-6),
                "{what}: one phase per branch"
            );
        }
    }
}

// ---------- the skeleton type (implemented by the owner-side session) ----------

#[test]
fn polylines_run_from_the_fork_to_the_tip_trunk_first() {
    let skeleton = Skeleton {
        position: vec![
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 2.0, 0.0],
            [1.0, 1.5, 0.0],
            [2.0, 2.0, 0.0],
        ],
        radius: vec![0.2, 0.15, 0.1, 0.05, 0.02],
        parent: vec![u32::MAX, 0, 1, 1, 3],
        depth: vec![0, 0, 0, 1, 1],
        wind: vec![0.0; 5],
        phase: vec![0.0; 5],
        stiffness: vec![1.0; 5],
        branch: vec![0, 0, 0, 1, 1],
        leaves: Vec::new(),
    };
    assert_eq!(skeleton.polylines(), vec![vec![0, 1, 2], vec![1, 3, 4]]);
    assert_eq!(skeleton.node_count(), 5);
    assert!((skeleton.height() - 2.0).abs() < 1e-6);
}

// ---------- parametric trees ----------

#[test]
fn an_oak_is_a_valid_tree_of_about_the_asked_height() {
    let skeleton = grow_tree(&oak(1), NEVER).unwrap();
    assert_valid(&skeleton, "oak");
    assert!(
        (skeleton.height() - 10.0).abs() < 1.5,
        "height {}",
        skeleton.height()
    );
    assert!(
        skeleton.depth.iter().max().copied().unwrap_or(0) >= 2,
        "three levels of branching"
    );
    assert!(!skeleton.leaves.is_empty(), "leaves on the twigs");
    assert!(
        skeleton
            .leaves
            .iter()
            .all(|leaf| skeleton.depth[leaf.node as usize] >= 2),
        "leaves only from the depth asked"
    );
}

#[test]
fn growth_is_deterministic_and_the_seed_changes_the_shape() {
    assert_eq!(
        grow_tree(&oak(1), NEVER).unwrap(),
        grow_tree(&oak(1), NEVER).unwrap()
    );
    assert_ne!(
        grow_tree(&oak(1), NEVER).unwrap().position,
        grow_tree(&oak(2), NEVER).unwrap().position
    );
}

#[test]
fn wind_grows_outward_and_with_depth_and_stiffness_falls() {
    let skeleton = grow_tree(&oak(4), NEVER).unwrap();
    let max_at = |depth: u8| {
        skeleton
            .depth
            .iter()
            .zip(&skeleton.wind)
            .filter(|(d, _)| **d == depth)
            .map(|(_, w)| *w)
            .fold(0.0_f32, f32::max)
    };
    assert!(
        max_at(2) > max_at(0),
        "twigs sway more than the trunk: {} against {}",
        max_at(2),
        max_at(0)
    );
    assert!(
        skeleton.wind[0] < 0.2,
        "the base hardly moves: {}",
        skeleton.wind[0]
    );
    assert!(
        skeleton.stiffness[0] > 0.8,
        "the base is stiff: {}",
        skeleton.stiffness[0]
    );
    let tip = (0..skeleton.node_count())
        .max_by(|a, b| skeleton.position[*a][1].total_cmp(&skeleton.position[*b][1]))
        .unwrap();
    assert!(
        skeleton.stiffness[tip] < 0.5,
        "the top is supple: {}",
        skeleton.stiffness[tip]
    );
}

#[test]
fn a_tree_with_more_branches_has_more_nodes() {
    let mut sparse = oak(1);
    sparse.levels[1].count = 3;
    let mut dense = oak(1);
    dense.levels[1].count = 12;
    assert!(
        grow_tree(&dense, NEVER).unwrap().node_count()
            > grow_tree(&sparse, NEVER).unwrap().node_count()
    );
}

#[test]
fn trees_refuse_bad_parameters_and_can_be_cancelled() {
    let mut none = oak(1);
    none.levels.clear();
    assert!(matches!(
        grow_tree(&none, NEVER),
        Err(FloraError::BadParameter { name: "levels", .. })
    ));
    let mut many = oak(1);
    many.levels = vec![many.levels[0]; 5];
    assert!(matches!(
        grow_tree(&many, NEVER),
        Err(FloraError::BadParameter { name: "levels", .. })
    ));
    let mut flat = oak(1);
    flat.height = 0.0;
    assert!(matches!(
        grow_tree(&flat, NEVER),
        Err(FloraError::BadParameter { .. })
    ));
    assert!(matches!(
        grow_tree(&oak(1), &|| true),
        Err(FloraError::Cancelled)
    ));
    let mut huge = oak(1);
    for level in &mut huge.levels {
        level.count = 400;
        level.segments = 40;
    }
    let error = grow_tree(&huge, NEVER).unwrap_err();
    assert!(
        matches!(error, FloraError::TooLarge { .. }) && error.to_string().contains("lower"),
        "{error}"
    );
}

// ---------- space colonization ----------

#[test]
fn a_colonized_crown_is_valid_obeys_the_pipe_model_and_stays_in_its_volume() {
    let params = crown(1500);
    let skeleton = colonize(&params, NEVER).unwrap();
    assert_valid(&skeleton, "crown");
    let kids = children(&skeleton);
    for (node, branches) in kids.iter().enumerate() {
        if branches.len() >= 2 {
            let parent = skeleton.radius[node].powf(2.0);
            let sum: f32 = branches.iter().map(|k| skeleton.radius[*k].powf(2.0)).sum();
            assert!(
                (parent - sum).abs() / parent < 0.02,
                "pipe model at node {node}: {parent} against {sum}"
            );
        }
    }
    // Everything above the trunk lies inside the crown ellipsoid, to within one growth step.
    let Crown::Ellipsoid { center, radii } = params.crown else {
        unreachable!()
    };
    for p in skeleton
        .position
        .iter()
        .filter(|p| p[1] > params.trunk_height + 0.5)
    {
        let d = ((p[0] - center[0]) / (radii[0] + params.step)).powi(2)
            + ((p[1] - center[1]) / (radii[1] + params.step)).powi(2)
            + ((p[2] - center[2]) / (radii[2] + params.step)).powi(2);
        assert!(d <= 1.0, "a node outside the crown at {p:?}");
    }
}

#[test]
fn more_attraction_points_make_a_denser_crown() {
    let few = colonize(&crown(300), NEVER).unwrap().node_count();
    let many = colonize(&crown(3000), NEVER).unwrap().node_count();
    assert!(many > few, "{many} against {few}");
}

#[test]
fn every_crown_shape_grows() {
    for shape in [
        Crown::Sphere {
            center: [0.0, 5.0, 0.0],
            radius: 3.0,
        },
        Crown::Cone {
            base: [0.0, 2.0, 0.0],
            height: 7.0,
            radius: 3.0,
        },
    ] {
        let mut params = crown(1200);
        params.crown = shape;
        assert_valid(&colonize(&params, NEVER).unwrap(), "crown shape");
    }
}

#[test]
fn colonization_refuses_bad_parameters_and_can_be_cancelled() {
    let mut reach = crown(500);
    reach.kill_distance = 2.0;
    assert!(matches!(
        colonize(&reach, NEVER),
        Err(FloraError::BadParameter {
            name: "kill_distance",
            ..
        })
    ));
    assert!(matches!(
        colonize(&crown(60_000), NEVER),
        Err(FloraError::BadParameter { name: "points", .. })
    ));
    let mut exponent = crown(500);
    exponent.radius_exponent = 5.0;
    assert!(matches!(
        colonize(&exponent, NEVER),
        Err(FloraError::BadParameter {
            name: "radius_exponent",
            ..
        })
    ));
    assert!(matches!(
        colonize(&crown(500), &|| true),
        Err(FloraError::Cancelled)
    ));
}

// ---------- tufts and presets ----------

#[test]
fn a_tuft_is_one_root_with_a_chain_per_blade() {
    let params = Tuft {
        seed: 2,
        blades: 12,
        height: [0.15, 0.4],
        width: 0.012,
        bend_deg: 35.0,
        spread: 0.05,
    };
    let skeleton = grow_tuft(&params, NEVER).unwrap();
    assert_eq!(
        skeleton.parent.iter().filter(|p| **p == u32::MAX).count(),
        1
    );
    let blades = skeleton.polylines().len();
    assert!(blades >= 12, "{blades} chains for 12 blades");
    assert!(
        skeleton.height() >= 0.14 && skeleton.height() <= 0.41,
        "height {}",
        skeleton.height()
    );
    assert!(
        skeleton.radius.iter().all(|r| *r <= 0.006 + 1e-5),
        "radius is half the width"
    );
    assert_eq!(
        skeleton,
        grow_tuft(&params, NEVER).unwrap(),
        "deterministic"
    );
    assert!(matches!(
        grow_tuft(
            &Tuft {
                blades: 0,
                ..params
            },
            NEVER
        ),
        Err(FloraError::BadParameter { .. })
    ));
}

#[test]
fn every_named_preset_grows_a_distinct_valid_plant() {
    let mut shapes = Vec::new();
    for name in NAMES {
        let skeleton = match preset(name, 8.0, 5).unwrap() {
            Preset::Tree(tree) => grow_tree(&tree, NEVER).unwrap(),
            Preset::Colonize(setup) => colonize(&setup, NEVER).unwrap(),
        };
        assert_valid(&skeleton, name);
        assert!(
            (skeleton.height() - 8.0).abs() < 2.0,
            "{name} is {} m tall",
            skeleton.height()
        );
        shapes.push((skeleton.node_count(), skeleton.leaves.len()));
    }
    let mut unique = shapes.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(
        unique.len(),
        NAMES.len(),
        "each preset looks different: {shapes:?}"
    );
}

#[test]
fn an_unknown_preset_names_the_valid_ones() {
    let error = preset("baobab", 8.0, 1).unwrap_err();
    assert!(
        NAMES.iter().all(|name| error.to_string().contains(name)),
        "{error}"
    );
    assert!(matches!(
        preset("oak", 0.0, 1),
        Err(FloraError::BadParameter { .. })
    ));
}

#[test]
#[ignore = "timing: run with `cargo test -p ve-world --release -- --ignored`"]
fn plants_grow_in_well_under_a_second() {
    let start = std::time::Instant::now();
    grow_tree(&oak(1), NEVER).unwrap();
    let tree = start.elapsed().as_secs_f32();
    let start = std::time::Instant::now();
    colonize(&crown(5000), NEVER).unwrap();
    let crown_time = start.elapsed().as_secs_f32();
    assert!(
        tree < 1.0 && crown_time < 1.0,
        "oak {tree} s, crown {crown_time} s"
    );
}
