//! Game cameras (F1 of `plans/game-foundations-plan.md`): `camera NAME mode=... ` statements describe how the view follows a target. The rig is
//! pure logic: given where the target is and where the player is looking, it returns an eye, a yaw, a pitch and a field of view. Third person
//! orbits with a shoulder offset and a spring arm that shortens against walls; top-down, side-scroll and isometric look along a fixed angle with
//! smoothing, a dead zone and bounds; a fixed camera stands still and watches; switching blends between cameras. Yaw is a compass angle (the
//! camera looks along `(sin yaw, cos yaw)` on the ground) and pitch is up-positive. Written before the body; do not weaken.
#![expect(clippy::unwrap_used, clippy::float_cmp, reason = "fixtures are known to be valid and compare exact rig values")]
use ve_game::{CameraInput, CameraRig, GameDef};

const DT: f64 = 1.0 / 60.0;

fn rig(cameras: &str) -> CameraRig {
    let text = format!("game name=cam\nkind hero size=0.4 tags=hero\n{cameras}\n");
    CameraRig::new(GameDef::parse(&text).unwrap().cameras)
}

fn look(target: [f64; 3], yaw: f64, pitch: f64) -> CameraInput {
    CameraInput { target, heading: 0.0, orbit_yaw: yaw, orbit_pitch: pitch, dt: DT, zoom: 1.0, yaw_offset: 0.0, pitch_offset: 0.0 }
}

fn adjusted(zoom: f64, yaw_offset: f64, pitch_offset: f64) -> CameraInput {
    CameraInput { zoom, yaw_offset, pitch_offset, ..look([0.0, 0.0, 0.0], 0.0, 0.0) }
}

#[test]
fn next_and_prev_cycle_through_the_cameras_and_wrap() {
    let mut camera = rig("camera side mode=side\ncamera behind mode=third\ncamera eyes mode=first");
    assert_eq!(camera.active(), "side");
    camera.switch("next", 0.0).unwrap();
    assert_eq!(camera.active(), "behind");
    camera.switch("next", 0.0).unwrap();
    assert_eq!(camera.active(), "eyes");
    camera.switch("next", 0.0).unwrap();
    assert_eq!(camera.active(), "side", "wraps past the last");
    camera.switch("prev", 0.0).unwrap();
    assert_eq!(camera.active(), "eyes", "and back before the first");
    assert!(rig("").switch("next", 0.0).is_err(), "nothing to cycle through without cameras");
}

#[test]
fn the_player_can_zoom_every_camera_that_has_a_distance_within_limits() {
    for (camera, from_eye) in [
        ("camera main mode=third distance=4 height=0 smoothing=0", 4.0),
        ("camera main mode=side distance=10 height=0 smoothing=0 yaw=180", 10.0),
        ("camera main mode=top_down distance=12 smoothing=0", 12.0),
        ("camera main mode=iso distance=16 smoothing=0", 16.0),
    ] {
        let mut rig_in = rig(camera);
        let far = rig_in.pose(&adjusted(2.0, 0.0, 0.0), &no_walls).eye;
        let mut rig_out = rig(camera);
        let near = rig_out.pose(&adjusted(0.5, 0.0, 0.0), &no_walls).eye;
        let reach = |eye: [f64; 3]| (eye[0].powi(2) + eye[1].powi(2) + eye[2].powi(2)).sqrt();
        near_ratio(reach(far) / reach(near), 4.0, camera);
        assert!(from_eye > 0.0);
    }
    let mut way_out = rig("camera main mode=top_down distance=12 smoothing=0");
    let clamped = way_out.pose(&adjusted(50.0, 0.0, 0.0), &no_walls).eye[1];
    assert!(clamped <= 12.0 * 3.0 + 0.1, "zoom is limited to three times the distance: {clamped}");
    let mut way_in = rig("camera main mode=top_down distance=12 smoothing=0");
    assert!(way_in.pose(&adjusted(0.001, 0.0, 0.0), &no_walls).eye[1] >= 12.0 * 0.3 - 0.1, "and to a third of it");
}

fn near_ratio(a: f64, b: f64, what: &str) {
    assert!((a - b).abs() < 0.05, "{what}: ratio {a} against {b}");
}

#[test]
fn the_player_can_turn_an_isometric_or_top_down_view_but_not_a_side_scroller() {
    let mut iso = rig("camera main mode=iso distance=16 smoothing=0 yaw=45 pitch=-35");
    let turned = iso.pose(&adjusted(1.0, 90.0_f64.to_radians(), 0.0), &no_walls);
    near(turned.yaw, 135.0_f64.to_radians(), 1e-9, "45 plus the 90 the player turned");
    let mut tilted = rig("camera main mode=iso distance=16 smoothing=0 yaw=45 pitch=-35");
    near(tilted.pose(&adjusted(1.0, 0.0, 20.0_f64.to_radians()), &no_walls).pitch, (-15.0_f64).to_radians(), 1e-9, "tilted up by 20 degrees");
    let mut capped = rig("camera main mode=iso distance=16 smoothing=0 yaw=45 pitch=-35");
    let pitch = capped.pose(&adjusted(1.0, 0.0, 3.0), &no_walls).pitch;
    assert!(pitch <= (-10.0_f64).to_radians() + 1e-9, "never looks above ten degrees below the horizon: {pitch}");
    let mut top = rig("camera main mode=top_down distance=12 smoothing=0");
    near(top.pose(&adjusted(1.0, 1.0, 0.0), &no_walls).yaw, 1.0, 1e-9, "a top-down view turns about the vertical");
    let mut side = rig("camera main mode=side distance=10 smoothing=0 yaw=180");
    near(side.pose(&adjusted(1.0, 1.0, 0.3), &no_walls).yaw, std::f64::consts::PI, 1e-9, "a side-scroller keeps its plane");
}

fn no_walls(_from: [f64; 3], _to: [f64; 3]) -> Option<f64> {
    None
}

fn near(a: f64, b: f64, tolerance: f64, what: &str) {
    assert!((a - b).abs() <= tolerance, "{what}: {a} against {b}");
}

#[test]
fn third_person_stands_behind_the_target_at_the_set_distance_and_height() {
    let mut camera = rig("camera main mode=third distance=4 height=1.5 smoothing=0");
    let pose = camera.pose(&look([10.0, 0.0, 5.0], 0.0, 0.0), &no_walls);
    near(pose.eye[0], 10.0, 1e-9, "x");
    near(pose.eye[1], 1.5, 1e-9, "height of the pivot");
    near(pose.eye[2], 1.0, 1e-9, "four metres behind the target (it looks along +z)");
    near(pose.yaw, 0.0, 1e-9, "looks along +z");
    near(pose.pitch, 0.0, 1e-9, "level");
}

#[test]
fn looking_down_raises_the_third_person_camera_on_its_arm() {
    let mut camera = rig("camera main mode=third distance=4 height=1.5 smoothing=0");
    let pose = camera.pose(&look([0.0, 0.0, 0.0], 0.0, (-30.0_f64).to_radians()), &no_walls);
    near(pose.eye[1], 1.5 + 4.0 * 0.5, 1e-9, "the arm swings up by distance times sin 30");
    near(pose.eye[2], -4.0 * (30.0_f64).to_radians().cos(), 1e-9, "and in by distance times cos 30");
    near(pose.pitch, (-30.0_f64).to_radians(), 1e-9, "the camera looks down at the target");
}

#[test]
fn the_orbit_follows_the_look_yaw() {
    let mut camera = rig("camera main mode=third distance=4 height=0 smoothing=0");
    let pose = camera.pose(&look([0.0, 0.0, 0.0], std::f64::consts::FRAC_PI_2, 0.0), &no_walls);
    near(pose.eye[0], -4.0, 1e-9, "looking along +x puts the camera at -x");
    near(pose.eye[2].abs(), 0.0, 1e-9, "z");
}

#[test]
fn a_shoulder_offset_moves_the_camera_to_the_right() {
    let mut camera = rig("camera main mode=third distance=4 height=1.5 shoulder=0.5 smoothing=0");
    let pose = camera.pose(&look([0.0, 0.0, 0.0], 0.0, 0.0), &no_walls);
    near(pose.eye[0], -0.5, 1e-9, "the right of a camera looking along +z is -x");
    // facing +x the right is +z
    let pose = camera.pose(&look([0.0, 0.0, 0.0], std::f64::consts::FRAC_PI_2, 0.0), &no_walls);
    near(pose.eye[2], 0.5, 1e-9, "the right of a camera looking along +x is +z");
}

#[test]
fn the_spring_arm_shortens_against_a_wall_and_leaves_a_margin() {
    let mut camera = rig("camera main mode=third distance=4 height=1.5 smoothing=0");
    let hit_at_two = |_from: [f64; 3], _to: [f64; 3]| Some(2.0);
    let pose = camera.pose(&look([0.0, 0.0, 0.0], 0.0, 0.0), &hit_at_two);
    let arm = (pose.eye[2].powi(2) + (pose.eye[1] - 1.5).powi(2)).sqrt();
    assert!(arm < 2.0 && arm > 1.5, "the arm stops short of the wall at 2 m: {arm}");
    // a wall further away than the arm changes nothing
    let far_wall = |_from: [f64; 3], _to: [f64; 3]| Some(9.0);
    let pose = camera.pose(&look([0.0, 0.0, 0.0], 0.0, 0.0), &far_wall);
    near(pose.eye[2], -4.0, 1e-9, "unchanged");
    // collide=0 ignores walls
    let mut ghost = rig("camera main mode=third distance=4 height=1.5 smoothing=0 collide=0");
    near(ghost.pose(&look([0.0, 0.0, 0.0], 0.0, 0.0), &hit_at_two).eye[2], -4.0, 1e-9, "no collision");
}

#[test]
fn smoothing_lets_the_camera_trail_a_jump_of_the_target_and_catch_up() {
    let mut camera = rig("camera main mode=third distance=4 height=0 smoothing=0.2");
    camera.pose(&look([0.0, 0.0, 0.0], 0.0, 0.0), &no_walls);
    let first = camera.pose(&look([10.0, 0.0, 0.0], 0.0, 0.0), &no_walls);
    assert!(first.eye[0] > 0.3 && first.eye[0] < 1.5, "one step moves it a little way: {}", first.eye[0]);
    let mut last = first;
    for _ in 0..180 {
        last = camera.pose(&look([10.0, 0.0, 0.0], 0.0, 0.0), &no_walls);
    }
    near(last.eye[0], 10.0, 0.01, "three seconds later it has caught up");
    // no smoothing is rigid
    let mut rigid = rig("camera main mode=third distance=4 height=0 smoothing=0");
    rigid.pose(&look([0.0, 0.0, 0.0], 0.0, 0.0), &no_walls);
    near(rigid.pose(&look([10.0, 0.0, 0.0], 0.0, 0.0), &no_walls).eye[0], 10.0, 1e-9, "rigid");
}

#[test]
fn the_third_person_pitch_is_limited() {
    let mut camera = rig("camera main mode=third distance=4 height=1 smoothing=0 min_pitch=-40 max_pitch=20");
    let low = camera.pose(&look([0.0, 0.0, 0.0], 0.0, (-80.0_f64).to_radians()), &no_walls);
    near(low.pitch, (-40.0_f64).to_radians(), 1e-9, "no further down than the limit");
    let high = camera.pose(&look([0.0, 0.0, 0.0], 0.0, (60.0_f64).to_radians()), &no_walls);
    near(high.pitch, (20.0_f64).to_radians(), 1e-9, "no further up than the limit");
}

#[test]
fn first_person_sits_at_eye_height_and_looks_where_the_player_looks() {
    let mut camera = rig("camera main mode=first height=1.7");
    let pose = camera.pose(&look([3.0, 1.0, 4.0], 0.7, -0.2), &no_walls);
    assert_eq!(pose.eye, [3.0, 2.7, 4.0]);
    assert_eq!((pose.yaw, pose.pitch), (0.7, -0.2));
}

#[test]
fn top_down_hangs_above_the_target_and_looks_straight_down() {
    let mut camera = rig("camera main mode=top_down distance=12 smoothing=0");
    let pose = camera.pose(&look([4.0, 0.0, -3.0], 1.234, 0.5), &no_walls);
    near(pose.eye[0], 4.0, 0.3, "above the target");
    near(pose.eye[1], 12.0, 0.3, "twelve metres up");
    near(pose.eye[2], -3.0, 0.3, "above the target");
    assert!(pose.pitch < -1.5, "looking down: {}", pose.pitch);
    // the player's look does not move a fixed-angle camera
    let again = camera.pose(&look([4.0, 0.0, -3.0], -2.0, 0.0), &no_walls);
    assert_eq!(pose.yaw, again.yaw);
}

#[test]
fn isometric_looks_along_a_fixed_angle() {
    let mut camera = rig("camera main mode=iso distance=16 smoothing=0 yaw=45 pitch=-35");
    let pose = camera.pose(&look([0.0, 0.0, 0.0], 0.0, 0.0), &no_walls);
    near(pose.yaw, 45.0_f64.to_radians(), 1e-9, "yaw");
    near(pose.pitch, (-35.0_f64).to_radians(), 1e-9, "pitch");
    near(pose.eye[1], 16.0 * 35.0_f64.to_radians().sin(), 1e-9, "height from the angle");
    assert!(pose.eye[0] < 0.0 && pose.eye[2] < 0.0, "it stands back toward -x and -z: {:?}", pose.eye);
}

#[test]
fn a_side_scroller_follows_only_outside_its_dead_zone() {
    let mut camera = rig("camera main mode=side distance=10 smoothing=0 deadzone=1.5");
    let start = camera.pose(&look([0.0, 0.0, 0.0], 0.0, 0.0), &no_walls);
    let small = camera.pose(&look([1.0, 0.0, 0.0], 0.0, 0.0), &no_walls);
    near(small.eye[0], start.eye[0], 1e-9, "the target moved 1 m inside a 1.5 m dead zone: the camera stays");
    let far = camera.pose(&look([4.0, 0.0, 0.0], 0.0, 0.0), &no_walls);
    near(far.eye[0], 4.0 - 1.5, 1e-9, "past the zone it follows, keeping the target at the edge");
    near(far.eye[2], -10.0, 1e-9, "ten metres to the side of the plane");
}

#[test]
fn a_side_scroller_does_not_bob_with_every_jump() {
    let mut camera = rig("camera main mode=side distance=10 smoothing=0 deadzone=1.5 height=0");
    let start = camera.pose(&look([0.0, 0.0, 0.0], 0.0, 0.0), &no_walls);
    let jumping = camera.pose(&look([0.0, 1.2, 0.0], 0.0, 0.0), &no_walls);
    near(jumping.eye[1], start.eye[1], 1e-9, "a 1.2 m jump inside the 1.5 m band leaves the camera where it is");
    let climbing = camera.pose(&look([0.0, 4.0, 0.0], 0.0, 0.0), &no_walls);
    near(climbing.eye[1], 4.0 - 1.5, 1e-9, "a real climb drags it up, keeping the hero at the edge of the band");
    let mut top = rig("camera main mode=top_down distance=10 smoothing=0 deadzone=1.5");
    top.pose(&look([0.0, 0.0, 0.0], 0.0, 0.0), &no_walls);
    near(top.pose(&look([0.0, 1.2, 0.0], 0.0, 0.0), &no_walls).eye[1], 1.2 + 10.0, 0.01, "other modes follow height exactly (a top-down view is 89 degrees down)");
}

#[test]
fn bounds_keep_the_camera_inside_the_level() {
    let mut camera = rig("camera main mode=side distance=10 smoothing=0 bounds=-5,5,-1,1");
    let pose = camera.pose(&look([30.0, 0.0, 0.0], 0.0, 0.0), &no_walls);
    near(pose.eye[0], 5.0, 1e-9, "stops at the right edge");
    let pose = camera.pose(&look([-30.0, 0.0, 0.0], 0.0, 0.0), &no_walls);
    near(pose.eye[0], -5.0, 1e-9, "and the left");
}

#[test]
fn a_fixed_camera_stands_still_and_watches_the_target() {
    let mut camera = rig("camera cctv mode=fixed at=0,5,-10");
    let a = camera.pose(&look([0.0, 0.0, 0.0], 0.0, 0.0), &no_walls);
    let b = camera.pose(&look([6.0, 0.0, 0.0], 0.0, 0.0), &no_walls);
    assert_eq!(a.eye, [0.0, 5.0, -10.0]);
    assert_eq!(a.eye, b.eye, "it does not move");
    near(a.yaw, 0.0, 1e-9, "the target is straight ahead");
    near(a.pitch, (-4.0_f64).atan2(10.0), 1e-9, "tilted down toward the middle of the target, a metre above its feet");
    assert!(b.yaw > 0.3, "and it turns to follow the target: {}", b.yaw);
}

#[test]
fn switching_cameras_blends_without_a_jump() {
    let mut camera = rig("camera behind mode=third distance=4 height=1 smoothing=0\ncamera above mode=top_down distance=20 smoothing=0");
    assert_eq!(camera.active(), "behind", "the first camera starts");
    let before = camera.pose(&look([0.0, 0.0, 0.0], 0.0, 0.0), &no_walls);
    camera.switch("above", 1.0).unwrap();
    assert_eq!(camera.active(), "above");
    let first = camera.pose(&look([0.0, 0.0, 0.0], 0.0, 0.0), &no_walls);
    assert!(
        (first.eye[1] - before.eye[1]).abs() < 1.0 && (first.eye[2] - before.eye[2]).abs() < 1.0,
        "the first frame after the switch is still near the old view: {:?} against {:?}",
        first.eye,
        before.eye
    );
    let mut halfway = first;
    for _ in 0..30 {
        halfway = camera.pose(&look([0.0, 0.0, 0.0], 0.0, 0.0), &no_walls);
    }
    assert!(halfway.eye[1] > before.eye[1] + 3.0 && halfway.eye[1] < 20.0 - 3.0, "halfway is between the two views: {}", halfway.eye[1]);
    let mut end = halfway;
    for _ in 0..60 {
        end = camera.pose(&look([0.0, 0.0, 0.0], 0.0, 0.0), &no_walls);
    }
    near(end.eye[1], 20.0, 0.01, "a second later it is the new camera (at 89 degrees down, a hair under 20 m)");
    assert!(camera.switch("nowhere", 0.5).is_err(), "an unknown camera is refused");
}

#[test]
fn the_same_inputs_give_the_same_poses() {
    let play = || {
        let mut camera = rig("camera main mode=third distance=4 height=1.5 smoothing=0.15 shoulder=0.4");
        let mut poses = Vec::new();
        for step in 0..120 {
            let t = f64::from(step) * 0.05;
            poses.push(camera.pose(&look([t.sin() * 5.0, 0.0, t.cos() * 5.0], t * 0.3, -0.2), &no_walls));
        }
        poses
    };
    assert_eq!(play(), play());
}

#[test]
fn bad_cameras_are_refused_with_a_reason() {
    let refused = |line: &str| {
        let text = format!("game name=cam\nkind hero size=0.4 tags=hero\n{line}\n");
        GameDef::parse(&text).unwrap_err().into_iter().map(|d| d.message).collect::<Vec<_>>().join(" | ")
    };
    assert!(refused("camera main mode=swim").contains("mode"));
    assert!(refused("camera main mode=third distance=-1").contains("distance"));
    assert!(refused("camera main mode=third smoothing=-1").contains("smoothing"));
    assert!(refused("camera main mode=fixed").contains("at"), "a fixed camera needs a place");
    assert!(refused("camera main mode=side bounds=1,2,3").contains("bounds"));
    assert!(refused("camera main mode=third min_pitch=30 max_pitch=-30").contains("pitch"));
    assert!(refused("camera main mode=third\ncamera main mode=first").contains("duplicate"));
    assert!(refused("camera main mode=third target=kind:").contains("target"));
}

#[test]
fn a_ray_finds_the_first_solid_box_along_it() {
    use ve_game::World;
    let text = "game name=cam\nkind hero size=0.4 tags=hero\nkind wall size=1 solid=4,3,1\nspawn hero at=0,0,0\nspawn wall at=0,1.5,-6\nspawn wall at=0,1.5,-10\n";
    let world = World::new(&GameDef::parse(text).unwrap(), 1);
    let hit = world.solid_hit([0.0, 1.0, 0.0], [0.0, 1.0, -12.0]).expect("the near wall is hit");
    near(hit, 5.5, 1e-9, "its face is at z = -5.5, five and a half metres along");
    assert!(world.solid_hit([0.0, 1.0, 0.0], [0.0, 1.0, -4.0]).is_none(), "a segment that stops short misses");
    assert!(world.solid_hit([0.0, 5.0, 0.0], [0.0, 5.0, -12.0]).is_none(), "above the wall misses");
    assert!(world.solid_hit([3.0, 1.0, 0.0], [3.0, 1.0, -12.0]).is_none(), "beside the wall misses");
}

#[test]
fn a_rule_can_cut_or_blend_to_a_camera() {
    use ve_game::{EffectKind, Inputs, World};
    let text = "game name=cam\nkind hero size=0.4 tags=hero\nspawn hero at=0,0,0\ncamera behind mode=third\ncamera above mode=top_down\nrule on=start do=effect:camera:above:1.5\nrule on=tick every=2 do=effect:camera:behind\n";
    let mut world = World::new(&GameDef::parse(text).unwrap(), 1);
    let first = world.step(&Inputs::default()).unwrap();
    let switch = first.effects.iter().find(|e| e.kind == EffectKind::Camera).expect("the start rule asks for a camera");
    assert_eq!((switch.name.as_str(), switch.value), ("above", Some(1.5)), "a blend of 1.5 seconds");
    let mut cut = None;
    for _ in 0..180 {
        if let Some(e) = world.step(&Inputs::default()).unwrap().effects.into_iter().find(|e| e.kind == EffectKind::Camera) {
            cut = Some(e);
            break;
        }
    }
    let cut = cut.expect("the timed rule switches back");
    assert_eq!((cut.name.as_str(), cut.value), ("behind", Some(0.0)), "no time given means a cut");
}

#[test]
fn a_camera_effect_without_a_name_or_with_a_bad_time_is_refused() {
    let refused = |action: &str| {
        let text = format!("game name=cam\nkind hero size=0.4 tags=hero\nrule on=start do={action}\n");
        GameDef::parse(&text).is_err()
    };
    assert!(refused("effect:camera:"));
    assert!(refused("effect:camera:above:soon"));
    assert!(!refused("effect:camera:above"));
    assert!(!refused("effect:camera:above:2"));
}

#[test]
fn a_camera_survives_a_text_round_trip() {
    let text = "game name=cam\nkind hero size=0.4 tags=hero\ncamera main mode=third distance=5 shoulder=0.5 smoothing=0.1\ncamera cctv mode=fixed at=0,5,-10\n";
    let def = GameDef::parse(text).unwrap();
    assert_eq!(def.cameras.len(), 2);
    let again = GameDef::parse(&def.to_text()).unwrap();
    assert_eq!(def.to_text(), again.to_text());
}
