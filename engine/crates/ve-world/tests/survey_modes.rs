//! Additional mode coverage without changing the handoff's survey fixtures.
use ve_world::survey::{SurveyMode, SurveyOptions, Triangle, plan_cameras};

fn floor(x: f64, y: f64, size: f64) -> Vec<Triangle> {
    let a = [x, y, 0.0];
    let b = [x, y, size];
    let c = [x + size, y, size];
    let d = [x + size, y, 0.0];
    vec![[a, b, c], [a, c, d]]
}

#[test]
fn auto_drops_roof_regions_while_retaining_the_floor_at_any_world_height() {
    for base in [0.0, -20.0, 37.0] {
        let mut scene = floor(0.0, base, 8.0);
        scene.extend(floor(0.0, base + 6.0, 8.0));
        let o = SurveyOptions {
            spacing: 2.0,
            ..SurveyOptions::default()
        };
        let auto = plan_cameras(&scene, &o);
        let walk = plan_cameras(
            &scene,
            &SurveyOptions {
                mode: SurveyMode::Walk,
                ..o.clone()
            },
        );
        assert_eq!(auto.mode, SurveyMode::Walk);
        assert!(!auto.cameras.is_empty());
        assert!(
            auto.cameras
                .iter()
                .all(|c| (c.position[1] - base - o.eye_height).abs() < 1e-6)
        );
        assert!(walk.standable_points > auto.standable_points);
    }
}

#[test]
fn auto_drops_a_small_disconnected_base_region() {
    let mut scene = floor(0.0, 0.0, 8.0);
    scene.extend(floor(30.0, 0.0, 2.0));
    let o = SurveyOptions {
        spacing: 1.0,
        samples_per_m2: 10.0,
        ..SurveyOptions::default()
    };
    let auto = plan_cameras(&scene, &o);
    let walk = plan_cameras(
        &scene,
        &SurveyOptions {
            mode: SurveyMode::Walk,
            ..o
        },
    );
    assert_eq!(auto.mode, SurveyMode::Walk);
    assert!(auto.cameras.iter().all(|c| c.position[0] <= 8.0));
    assert!(walk.standable_points > auto.standable_points);
}

#[test]
fn auto_orbits_when_every_connected_base_region_is_too_small() {
    let o = SurveyOptions {
        spacing: 1.0,
        samples_per_m2: 10.0,
        ..SurveyOptions::default()
    };
    let auto = plan_cameras(&floor(0.0, 0.0, 2.0), &o);
    let walk = plan_cameras(
        &floor(0.0, 0.0, 2.0),
        &SurveyOptions {
            mode: SurveyMode::Walk,
            ..o
        },
    );
    assert_eq!(auto.mode, SurveyMode::Orbit);
    assert_eq!(auto.standable_points, 0);
    assert!(walk.standable_points > 0);
    assert_eq!(walk.mode, SurveyMode::Walk);
}

#[test]
fn orbit_uses_the_full_budget_at_two_heights_and_looks_at_the_centre() {
    let scene = floor(0.0, 0.0, 8.0);
    let centre = [4.0, 0.0, 4.0];
    for budget in [0, 1, 2, 3, 8, 13, 24] {
        let o = SurveyOptions {
            mode: SurveyMode::Orbit,
            max_views: budget,
            ..SurveyOptions::default()
        };
        let plan = plan_cameras(&scene, &o);
        assert_eq!(plan.mode, SurveyMode::Orbit);
        assert_eq!(plan.cameras.len(), budget);
        assert_eq!(plan.predicted.len(), budget);
        assert!(plan.predicted.windows(2).all(|w| w[1] >= w[0]));
        assert!((plan.predicted.last().copied().unwrap_or_default() - plan.coverage).abs() < 1e-9);
        for (i, camera) in plan.cameras.iter().enumerate() {
            assert_eq!(camera.label, format!("aerial_{}", i + 1));
            let delta: [f64; 3] = std::array::from_fn(|axis| centre[axis] - camera.position[axis]);
            let distance = delta.iter().map(|v| v * v).sum::<f64>().sqrt();
            for (axis, component) in delta.iter().enumerate() {
                assert!(
                    (camera.target[axis] - camera.position[axis] - component / distance).abs()
                        < 1e-9
                );
            }
        }
        if budget >= 2 {
            assert!(plan.cameras.iter().any(|c| c.position[1].abs() < 1e-9));
            assert!(plan.cameras.iter().any(|c| c.position[1] > 0.0));
        }
    }
}

#[test]
fn walk_falls_back_to_orbit_when_geometry_has_no_upward_faces() {
    let scene = vec![[[0.0, 0.0, 0.0], [8.0, 0.0, 0.0], [0.0, 3.0, 0.0]]];
    let plan = plan_cameras(
        &scene,
        &SurveyOptions {
            mode: SurveyMode::Walk,
            ..SurveyOptions::default()
        },
    );
    assert_eq!(plan.mode, SurveyMode::Orbit);
    assert_eq!(plan.standable_points, 0);
    assert!(!plan.cameras.is_empty());
    assert!(plan.cameras.iter().all(|c| c.label.starts_with("aerial_")));
    assert_eq!(
        plan_cameras(&[], &SurveyOptions::default()).mode,
        SurveyMode::Orbit
    );
}
