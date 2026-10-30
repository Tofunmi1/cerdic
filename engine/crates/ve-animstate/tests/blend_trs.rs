//! `blend_trs`: the weighted blend of poses the runtime applies to a part when a state machine plays several clips at once. A pose is a
//! translation, a rotation (unit quaternion x, y, z, w) and a scale; blending lerps translation and scale and takes the normalised weighted
//! sum of the quaternions on one hemisphere. Pure arithmetic (square root allowed), so native and wasm agree. Written before the body.
#![expect(
    clippy::cast_precision_loss,
    reason = "fixtures build poses from small indices"
)]
use ve_animstate::{Trs, blend_trs};

const IDENTITY: Trs = Trs {
    translation: [0.0; 3],
    rotation: [0.0, 0.0, 0.0, 1.0],
    scale: [1.0; 3],
};

/// A rotation of `degrees` about y.
fn about_y(degrees: f32) -> [f32; 4] {
    let half = degrees.to_radians() * 0.5;
    [0.0, half.sin(), 0.0, half.cos()]
}

/// The angle in degrees of a unit quaternion's rotation.
fn angle(q: [f32; 4]) -> f32 {
    2.0 * q[3].clamp(-1.0, 1.0).acos().to_degrees()
}

fn close(a: [f32; 3], b: [f32; 3]) -> bool {
    a.iter().zip(&b).all(|(x, y)| (x - y).abs() < 1e-5)
}

fn length(q: [f32; 4]) -> f32 {
    q.iter().map(|c| c * c).sum::<f32>().sqrt()
}

#[test]
fn one_pose_comes_back_unchanged() {
    let pose = Trs {
        translation: [1.0, 2.0, 3.0],
        rotation: about_y(30.0),
        scale: [1.0, 2.0, 1.0],
    };
    let out = blend_trs(&[(1.0, pose)]).unwrap();
    assert!(close(out.translation, pose.translation));
    assert!(close(out.scale, pose.scale));
    assert!((angle(out.rotation) - 30.0).abs() < 0.01);
}

#[test]
fn an_equal_blend_is_the_midpoint() {
    let a = IDENTITY;
    let b = Trs {
        translation: [2.0, 0.0, -4.0],
        rotation: about_y(90.0),
        scale: [3.0, 1.0, 1.0],
    };
    let out = blend_trs(&[(0.5, a), (0.5, b)]).unwrap();
    assert!(close(out.translation, [1.0, 0.0, -2.0]));
    assert!(close(out.scale, [2.0, 1.0, 1.0]));
    assert!(
        (angle(out.rotation) - 45.0).abs() < 0.02,
        "angle {}",
        angle(out.rotation)
    );
    assert!(
        (length(out.rotation) - 1.0).abs() < 1e-5,
        "the rotation stays a unit quaternion"
    );
}

#[test]
fn weights_pull_toward_the_heavier_pose() {
    let b = Trs {
        translation: [4.0, 0.0, 0.0],
        ..IDENTITY
    };
    let out = blend_trs(&[(0.75, IDENTITY), (0.25, b)]).unwrap();
    assert!(close(out.translation, [1.0, 0.0, 0.0]));
}

#[test]
fn weights_are_normalised_so_they_need_not_sum_to_one() {
    let b = Trs {
        translation: [2.0, 0.0, 0.0],
        ..IDENTITY
    };
    let halves = blend_trs(&[(0.5, IDENTITY), (0.5, b)]).unwrap();
    let twos = blend_trs(&[(2.0, IDENTITY), (2.0, b)]).unwrap();
    assert!(close(halves.translation, twos.translation));
}

#[test]
fn a_quaternion_and_its_negative_blend_to_the_same_rotation() {
    // q and -q are one rotation; a naive average of them would be the zero quaternion.
    let q = about_y(60.0);
    let neg = [-q[0], -q[1], -q[2], -q[3]];
    let a = Trs {
        rotation: q,
        ..IDENTITY
    };
    let b = Trs {
        rotation: neg,
        ..IDENTITY
    };
    let out = blend_trs(&[(0.5, a), (0.5, b)]).unwrap();
    assert!((length(out.rotation) - 1.0).abs() < 1e-5);
    assert!(
        (angle(out.rotation) - 60.0).abs() < 0.02,
        "angle {}",
        angle(out.rotation)
    );
}

#[test]
fn the_order_of_the_poses_does_not_matter() {
    let a = Trs {
        translation: [1.0, 0.0, 0.0],
        rotation: about_y(20.0),
        scale: [1.0; 3],
    };
    let b = Trs {
        translation: [0.0, 3.0, 0.0],
        rotation: about_y(110.0),
        scale: [2.0; 3],
    };
    let c = Trs {
        translation: [0.0, 0.0, -2.0],
        rotation: about_y(-40.0),
        scale: [0.5; 3],
    };
    let one = blend_trs(&[(0.2, a), (0.5, b), (0.3, c)]).unwrap();
    let two = blend_trs(&[(0.3, c), (0.2, a), (0.5, b)]).unwrap();
    assert!(close(one.translation, two.translation));
    assert!(close(one.scale, two.scale));
    assert!((angle(one.rotation) - angle(two.rotation)).abs() < 0.01);
}

#[test]
fn a_pose_with_no_weight_is_ignored() {
    let b = Trs {
        translation: [9.0, 9.0, 9.0],
        ..IDENTITY
    };
    let out = blend_trs(&[(1.0, IDENTITY), (0.0, b)]).unwrap();
    assert!(close(out.translation, [0.0; 3]));
}

#[test]
fn nothing_to_blend_is_none() {
    assert!(blend_trs(&[]).is_none());
    assert!(
        blend_trs(&[(0.0, IDENTITY), (0.0, IDENTITY)]).is_none(),
        "total weight zero"
    );
    assert!(
        blend_trs(&[(-1.0, IDENTITY)]).is_none(),
        "a negative weight"
    );
    assert!(blend_trs(&[(f32::NAN, IDENTITY)]).is_none());
    let bad = Trs {
        translation: [f32::INFINITY, 0.0, 0.0],
        ..IDENTITY
    };
    assert!(blend_trs(&[(1.0, bad)]).is_none(), "a non-finite pose");
    let zero_rotation = Trs {
        rotation: [0.0; 4],
        ..IDENTITY
    };
    assert!(
        blend_trs(&[(1.0, zero_rotation)]).is_none(),
        "a rotation of zero length"
    );
}

#[test]
fn a_whole_machine_pose_blends_into_a_unit_rotation() {
    // Eight layers, the most a pose carries, with awkward weights.
    let layers: Vec<(f32, Trs)> = (0..8)
        .map(|i| {
            let w = 1.0 + i as f32 * 0.37;
            (
                w,
                Trs {
                    translation: [i as f32, 0.0, 0.0],
                    rotation: about_y(i as f32 * 47.0 - 100.0),
                    scale: [1.0; 3],
                },
            )
        })
        .collect();
    let out = blend_trs(&layers).unwrap();
    assert!((length(out.rotation) - 1.0).abs() < 1e-4);
    assert!(out.translation[0] > 0.0 && out.translation[0] < 7.0);
}
