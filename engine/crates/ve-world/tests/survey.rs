//! `ve_world::survey`: where to stand and look to see a whole level (the camera planner idea of Patch's Trompe, Apache-2.0, redone on plain geometry).
//! Sample where a person can stand, build candidate cameras there, and keep greedily the cameras that see the most of the surface well.
#![expect(
    clippy::unwrap_used,
    clippy::float_cmp,
    reason = "fixtures are known to be valid; determinism is compared exactly"
)]
use ve_world::survey::{SurveyMode, SurveyOptions, Triangle, plan_cameras};

/// Two triangles for the quad a-b-c-d (counter-clockwise seen from the side its normal points to).
fn quad(a: [f64; 3], b: [f64; 3], c: [f64; 3], d: [f64; 3]) -> Vec<Triangle> {
    vec![[a, b, c], [a, c, d]]
}

/// A floor at y = 0 (normal up) over x0..x1, z0..z1.
fn floor(x0: f64, x1: f64, z0: f64, z1: f64) -> Vec<Triangle> {
    quad([x0, 0.0, z0], [x0, 0.0, z1], [x1, 0.0, z1], [x1, 0.0, z0])
}

/// A vertical wall from (x0, z0) to (x1, z1), height h, split into a grid so rays and samples see small pieces.
fn wall(x0: f64, z0: f64, x1: f64, z1: f64, h: f64) -> Vec<Triangle> {
    let mut out = Vec::new();
    let n = 6;
    for i in 0..n {
        let (t0, t1) = (f64::from(i) / f64::from(n), f64::from(i + 1) / f64::from(n));
        let (ax, az) = (x0 + (x1 - x0) * t0, z0 + (z1 - z0) * t0);
        let (bx, bz) = (x0 + (x1 - x0) * t1, z0 + (z1 - z0) * t1);
        out.extend(quad([ax, 0.0, az], [bx, 0.0, bz], [bx, h, bz], [ax, h, az]));
    }
    out
}

/// A room without a ceiling: floor and four walls.
fn room(x0: f64, x1: f64, z0: f64, z1: f64, h: f64) -> Vec<Triangle> {
    let mut t = floor(x0, x1, z0, z1);
    t.extend(wall(x0, z0, x1, z0, h));
    t.extend(wall(x1, z0, x1, z1, h));
    t.extend(wall(x1, z1, x0, z1, h));
    t.extend(wall(x0, z1, x0, z0, h));
    t
}

fn options(max_views: usize, target: f64) -> SurveyOptions {
    SurveyOptions {
        max_views,
        target_coverage: target,
        ..SurveyOptions::default()
    }
}

#[test]
fn an_empty_level_gives_no_cameras_and_no_coverage() {
    let plan = plan_cameras(&[], &SurveyOptions::default());
    assert!(plan.cameras.is_empty());
    assert_eq!(plan.coverage, 0.0);
    assert_eq!(plan.standable_points, 0);
}

#[test]
fn a_room_is_covered_by_a_few_cameras_standing_on_its_floor_at_eye_height() {
    let room = room(0.0, 8.0, 0.0, 8.0, 3.0);
    let o = options(12, 0.9);
    let plan = plan_cameras(&room, &o);
    assert!(
        !plan.cameras.is_empty() && plan.cameras.len() <= 12,
        "{}",
        plan.cameras.len()
    );
    assert!(plan.coverage >= 0.9, "coverage {}", plan.coverage);
    for c in &plan.cameras {
        assert!(
            (c.position[1] - o.eye_height).abs() < 1e-6,
            "{:?}",
            c.position
        );
        assert!(
            (0.0..=8.0).contains(&c.position[0]) && (0.0..=8.0).contains(&c.position[2]),
            "{:?}",
            c.position
        );
        let look = [
            c.target[0] - c.position[0],
            c.target[1] - c.position[1],
            c.target[2] - c.position[2],
        ];
        assert!(
            look.iter().all(|v| v.is_finite()) && (look[0].hypot(look[2]) + look[1].abs()) > 0.1,
            "{look:?}"
        );
    }
    for (i, a) in plan.cameras.iter().enumerate() {
        for b in &plan.cameras[i + 1..] {
            assert!(
                a.position != b.position || a.target != b.target,
                "two identical cameras"
            );
        }
    }
}

#[test]
fn the_predicted_coverage_never_falls_and_ends_at_the_reported_coverage() {
    let plan = plan_cameras(&room(0.0, 8.0, 0.0, 8.0, 3.0), &options(12, 0.9));
    assert_eq!(plan.predicted.len(), plan.cameras.len());
    assert!(
        plan.predicted.windows(2).all(|w| w[1] >= w[0]),
        "{:?}",
        plan.predicted
    );
    assert!((plan.predicted.last().copied().unwrap() - plan.coverage).abs() < 1e-9);
}

#[test]
fn a_higher_target_takes_more_cameras() {
    let room = room(0.0, 8.0, 0.0, 8.0, 3.0);
    let few = plan_cameras(&room, &options(30, 0.5));
    let many = plan_cameras(&room, &options(30, 0.97));
    assert!(
        few.cameras.len() < many.cameras.len(),
        "{} against {}",
        few.cameras.len(),
        many.cameras.len()
    );
}

#[test]
fn the_view_budget_is_respected() {
    let plan = plan_cameras(&room(0.0, 14.0, 0.0, 6.0, 3.0), &options(2, 0.99));
    assert!(plan.cameras.len() <= 2);
}

#[test]
fn two_rooms_joined_by_a_door_both_get_cameras() {
    // One 14 x 6 hall with a partition at x = 7 that has a door gap between z = 2.5 and z = 3.5.
    let mut level = room(0.0, 14.0, 0.0, 6.0, 3.0);
    level.extend(wall(7.0, 0.0, 7.0, 2.5, 3.0));
    level.extend(wall(7.0, 3.5, 7.0, 6.0, 3.0));
    let plan = plan_cameras(&level, &options(24, 0.9));
    let left = plan.cameras.iter().filter(|c| c.position[0] < 7.0).count();
    let right = plan.cameras.iter().filter(|c| c.position[0] > 7.0).count();
    assert!(left > 0 && right > 0, "left {left} right {right}");
    assert!(plan.coverage >= 0.85, "coverage {}", plan.coverage);
}

#[test]
fn a_pillar_in_the_middle_is_seen_from_several_sides() {
    let mut level = room(0.0, 10.0, 0.0, 10.0, 3.0);
    // 1 x 1 pillar, 3 m tall, around (5, 5), faces outward.
    level.extend(wall(4.5, 4.5, 5.5, 4.5, 3.0));
    level.extend(wall(5.5, 4.5, 5.5, 5.5, 3.0));
    level.extend(wall(5.5, 5.5, 4.5, 5.5, 3.0));
    level.extend(wall(4.5, 5.5, 4.5, 4.5, 3.0));
    let plan = plan_cameras(&level, &options(20, 0.9));
    assert!(plan.coverage >= 0.85, "coverage {}", plan.coverage);
    let mut sides = std::collections::BTreeSet::new();
    for c in &plan.cameras {
        sides.insert((c.position[0] >= 5.0, c.position[2] >= 5.0));
    }
    assert!(
        sides.len() >= 3,
        "cameras on {} sides of the pillar: {sides:?}",
        sides.len()
    );
}

#[test]
fn the_same_options_give_the_same_plan_and_another_seed_still_reaches_the_target() {
    let level = room(0.0, 8.0, 0.0, 8.0, 3.0);
    let a = plan_cameras(&level, &options(12, 0.9));
    let b = plan_cameras(&level, &options(12, 0.9));
    assert_eq!(format!("{a:?}"), format!("{b:?}"));
    let other = plan_cameras(
        &level,
        &SurveyOptions {
            seed: 99,
            ..options(12, 0.9)
        },
    );
    assert!(other.coverage >= 0.85, "coverage {}", other.coverage);
}

#[test]
fn a_level_with_nowhere_to_stand_gets_aerial_cameras_around_it() {
    // One vertical wall and no floor: nobody can stand here.
    let level = wall(0.0, 0.0, 10.0, 0.0, 3.0);
    let plan = plan_cameras(&level, &options(8, 0.9));
    assert_eq!(plan.standable_points, 0);
    assert!(plan.cameras.len() >= 4, "{}", plan.cameras.len());
    assert!(
        plan.cameras.iter().all(|c| c.label.starts_with("aerial")),
        "{:?}",
        plan.cameras.iter().map(|c| &c.label).collect::<Vec<_>>()
    );
    assert!(plan.cameras.iter().all(|c| c.position[1] > 1.0));
}

#[test]
fn standing_cameras_are_labelled_stand_and_numbered_in_order() {
    let plan = plan_cameras(&room(0.0, 8.0, 0.0, 8.0, 3.0), &options(12, 0.9));
    for (i, c) in plan.cameras.iter().enumerate() {
        assert_eq!(c.label, format!("stand_{}", i + 1));
    }
}

#[test]
fn a_roof_over_the_player_blocks_standing_there() {
    // The room has a ceiling 1.9 m up: eye height 1.65 plus 0.3 headroom does not fit, so nobody can stand.
    let mut level = room(0.0, 8.0, 0.0, 8.0, 1.9);
    level.extend(quad(
        [0.0, 1.9, 0.0],
        [8.0, 1.9, 0.0],
        [8.0, 1.9, 8.0],
        [0.0, 1.9, 8.0],
    ));
    let plan = plan_cameras(&level, &options(8, 0.9));
    assert_eq!(plan.standable_points, 0);
}

/// An object with no ground under it: four walls and a hipped roof (a cottage floating in space), nothing else.
fn cottage_without_ground() -> Vec<Triangle> {
    let mut t = Vec::new();
    t.extend(wall(0.0, 0.0, 5.0, 0.0, 2.6));
    t.extend(wall(5.0, 0.0, 5.0, 4.0, 2.6));
    t.extend(wall(5.0, 4.0, 0.0, 4.0, 2.6));
    t.extend(wall(0.0, 4.0, 0.0, 0.0, 2.6));
    let apex = [2.5, 4.2, 2.0];
    // Wound so every roof face looks up and out (normal y about 0.8): the case where a planner could wrongly stand on the roof.
    t.push([[0.0, 2.6, 0.0], apex, [5.0, 2.6, 0.0]]);
    t.push([[5.0, 2.6, 0.0], apex, [5.0, 2.6, 4.0]]);
    t.push([[5.0, 2.6, 4.0], apex, [0.0, 2.6, 4.0]]);
    t.push([[0.0, 2.6, 4.0], apex, [0.0, 2.6, 0.0]]);
    t
}

#[test]
fn an_object_with_no_ground_is_surveyed_from_around_it_not_from_standing_on_its_roof() {
    let plan = plan_cameras(&cottage_without_ground(), &options(8, 0.9));
    assert!(plan.cameras.len() >= 4, "{}", plan.cameras.len());
    assert!(
        plan.cameras.iter().all(|c| c.label.starts_with("aerial")),
        "{:?}",
        plan.cameras.iter().map(|c| &c.label).collect::<Vec<_>>()
    );
    // Every side of the object is looked at: cameras on at least three of the four sides of its centre.
    let mut sides = std::collections::BTreeSet::new();
    for c in &plan.cameras {
        sides.insert((c.position[0] >= 2.5, c.position[2] >= 2.0));
    }
    assert!(sides.len() >= 3, "{sides:?}");
    assert!(plan.coverage >= 0.6, "coverage {}", plan.coverage);
}

#[test]
fn walk_mode_still_stands_wherever_it_can_and_orbit_mode_always_goes_around() {
    let ground_and_pillar = {
        let mut level = room(0.0, 10.0, 0.0, 10.0, 3.0);
        level.extend(wall(4.5, 4.5, 5.5, 4.5, 3.0));
        level.extend(wall(5.5, 4.5, 5.5, 5.5, 3.0));
        level.extend(wall(5.5, 5.5, 4.5, 5.5, 3.0));
        level.extend(wall(4.5, 5.5, 4.5, 4.5, 3.0));
        level
    };
    let walk = plan_cameras(
        &ground_and_pillar,
        &SurveyOptions {
            mode: SurveyMode::Walk,
            ..options(12, 0.9)
        },
    );
    assert!(
        walk.cameras.iter().all(|c| c.label.starts_with("stand")),
        "{:?}",
        walk.cameras.iter().map(|c| &c.label).collect::<Vec<_>>()
    );
    let orbit = plan_cameras(
        &ground_and_pillar,
        &SurveyOptions {
            mode: SurveyMode::Orbit,
            ..options(12, 0.9)
        },
    );
    assert!(
        orbit.cameras.len() >= 4 && orbit.cameras.iter().all(|c| c.label.starts_with("aerial")),
        "{:?}",
        orbit.cameras.iter().map(|c| &c.label).collect::<Vec<_>>()
    );
    // Auto picks standing for a level with a real floor.
    let auto = plan_cameras(&ground_and_pillar, &options(12, 0.9));
    assert!(
        auto.cameras.iter().all(|c| c.label.starts_with("stand")),
        "{:?}",
        auto.cameras.iter().map(|c| &c.label).collect::<Vec<_>>()
    );
}

#[test]
fn walk_mode_stands_wherever_it_can_even_on_the_roof_of_a_floating_object() {
    // Walk keeps the original behaviour: the roof faces look up and have headroom, so they are places to stand. Auto is what refuses them.
    let plan = plan_cameras(
        &cottage_without_ground(),
        &SurveyOptions {
            mode: SurveyMode::Walk,
            ..options(8, 0.9)
        },
    );
    assert!(!plan.cameras.is_empty());
    assert!(
        plan.cameras.iter().all(|c| c.label.starts_with("stand")),
        "{:?}",
        plan.cameras.iter().map(|c| &c.label).collect::<Vec<_>>()
    );
    assert!(plan.standable_points > 0);
}

#[test]
fn a_small_standable_patch_on_top_of_a_big_object_does_not_count_as_a_place_to_stand() {
    // A 12 x 12 x 6 block seen from outside, with a 1 x 1 flat patch on its roof: not a level to walk in.
    let mut t = Vec::new();
    t.extend(wall(0.0, 0.0, 12.0, 0.0, 6.0));
    t.extend(wall(12.0, 0.0, 12.0, 12.0, 6.0));
    t.extend(wall(12.0, 12.0, 0.0, 12.0, 6.0));
    t.extend(wall(0.0, 12.0, 0.0, 0.0, 6.0));
    t.extend(quad(
        [5.5, 6.0, 5.5],
        [5.5, 6.0, 6.5],
        [6.5, 6.0, 6.5],
        [6.5, 6.0, 5.5],
    ));
    let plan = plan_cameras(&t, &options(8, 0.9));
    assert!(
        plan.cameras.iter().all(|c| c.label.starts_with("aerial")),
        "{:?}",
        plan.cameras.iter().map(|c| &c.label).collect::<Vec<_>>()
    );
}
