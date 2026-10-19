//! Dual-quaternion skin blending (ADR 0033): bones blend as rigid motions, so a twisted limb keeps its volume where the
//! linear blend collapses it ("candy wrapper").
//!
//! Each bone pose (a column-major matrix) becomes a unit dual quaternion (rotation plus translation; scale in a pose is
//! ignored). The weighted sum is renormalised and applied to the vertex. All arithmetic is in f64 with a fixed order,
//! so native and WASM agree bit for bit.
use serde::{Deserialize, Serialize};

/// How the bones' motions are combined per vertex.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Blend {
    /// Weighted sum of the moved vertex (fast; a strong twist pinches).
    #[default]
    Linear,
    /// Weighted sum of rigid motions, renormalised (keeps volume through twists).
    DualQuaternion,
}

type Quat = [f64; 4]; // x, y, z, w

fn mul(a: Quat, b: Quat) -> Quat {
    [
        a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
        a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
        a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
        a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
    ]
}

fn dot(a: Quat, b: Quat) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3]
}

/// The rotation of a column-major matrix's upper 3 x 3 (scale divided out), as a unit quaternion.
fn rotation(pose: &[f32; 16]) -> Quat {
    let column = |index: usize| {
        let at = index * 4;
        let axis = [
            f64::from(pose[at]),
            f64::from(pose[at + 1]),
            f64::from(pose[at + 2]),
        ];
        let length = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
        if length > 1e-12 {
            [axis[0] / length, axis[1] / length, axis[2] / length]
        } else {
            axis
        }
    };
    let (col_x, col_y, col_z) = (column(0), column(1), column(2));
    // r<row><col>
    let (r00, r10, r20) = (col_x[0], col_x[1], col_x[2]);
    let (r01, r11, r21) = (col_y[0], col_y[1], col_y[2]);
    let (r02, r12, r22) = (col_z[0], col_z[1], col_z[2]);
    let trace = r00 + r11 + r22;
    let quat = if trace > 0.0 {
        let scale = (trace + 1.0).sqrt() * 2.0;
        [
            (r21 - r12) / scale,
            (r02 - r20) / scale,
            (r10 - r01) / scale,
            0.25 * scale,
        ]
    } else if r00 > r11 && r00 > r22 {
        let scale = (1.0 + r00 - r11 - r22).sqrt() * 2.0;
        [
            0.25 * scale,
            (r01 + r10) / scale,
            (r02 + r20) / scale,
            (r21 - r12) / scale,
        ]
    } else if r11 > r22 {
        let scale = (1.0 + r11 - r00 - r22).sqrt() * 2.0;
        [
            (r01 + r10) / scale,
            0.25 * scale,
            (r12 + r21) / scale,
            (r02 - r20) / scale,
        ]
    } else {
        let scale = (1.0 + r22 - r00 - r11).sqrt() * 2.0;
        [
            (r02 + r20) / scale,
            (r12 + r21) / scale,
            0.25 * scale,
            (r10 - r01) / scale,
        ]
    };
    let length = dot(quat, quat).sqrt();
    [
        quat[0] / length,
        quat[1] / length,
        quat[2] / length,
        quat[3] / length,
    ]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Turns `v` by the unit quaternion `q` (`v + w t + u x t` with `t = 2 u x v`).
fn rotate(quat: Quat, vector: [f64; 3]) -> [f64; 3] {
    let axis = [quat[0], quat[1], quat[2]];
    let side = cross(axis, vector);
    let twice = [2.0 * side[0], 2.0 * side[1], 2.0 * side[2]];
    let extra = cross(axis, twice);
    [
        vector[0] + quat[3] * twice[0] + extra[0],
        vector[1] + quat[3] * twice[1] + extra[1],
        vector[2] + quat[3] * twice[2] + extra[2],
    ]
}

/// A bone pose as a unit dual quaternion (rotation, and half the translation times the rotation).
#[derive(Clone, Copy, Debug)]
pub struct DualPose {
    real: Quat,
    dual: Quat,
}

/// Converts every bone pose once, so a frame's many vertices only blend.
#[must_use]
pub fn prepare(poses: &[[f32; 16]]) -> Vec<DualPose> {
    poses
        .iter()
        .map(|pose| {
            let q = rotation(pose);
            let d = mul(
                [
                    f64::from(pose[12]),
                    f64::from(pose[13]),
                    f64::from(pose[14]),
                    0.0,
                ],
                q,
            );
            DualPose {
                real: q,
                dual: [0.5 * d[0], 0.5 * d[1], 0.5 * d[2], 0.5 * d[3]],
            }
        })
        .collect()
}

/// Where a skinned vertex and its normal are when the bones are posed, blended as dual quaternions. Same inputs and
/// meaning as [`super::deform`]; converts the poses on every call, so a mesh should use [`deform_prepared`].
#[must_use]
pub fn deform_dual(
    position: [f32; 3],
    normal: [f32; 3],
    joints: [u16; 4],
    weights: [f32; 4],
    poses: &[[f32; 16]],
) -> ([f32; 3], [f32; 3]) {
    deform_prepared(position, normal, joints, weights, &prepare(poses))
}

/// [`deform_dual`] with the poses already converted by [`prepare`].
#[must_use]
pub fn deform_prepared(
    position: [f32; 3],
    normal: [f32; 3],
    joints: [u16; 4],
    weights: [f32; 4],
    poses: &[DualPose],
) -> ([f32; 3], [f32; 3]) {
    let (mut real, mut dual) = ([0.0_f64; 4], [0.0_f64; 4]);
    let mut first: Option<Quat> = None;
    for (joint, weight) in joints.iter().zip(weights) {
        if weight == 0.0 {
            continue;
        }
        let Some(pose) = poses.get(usize::from(*joint)) else {
            continue;
        };
        // Both q and -q are the same rotation: pick the one on the first bone's side so the sum does not cancel.
        let sign = match first {
            Some(reference) if dot(pose.real, reference) < 0.0 => -1.0,
            Some(_) => 1.0,
            None => {
                first = Some(pose.real);
                1.0
            }
        };
        let w = f64::from(weight) * sign;
        for axis in 0..4 {
            real[axis] += w * pose.real[axis];
            dual[axis] += w * pose.dual[axis];
        }
    }
    let length = dot(real, real).sqrt();
    if length < 1e-12 {
        return (position, normal);
    }
    let real = [
        real[0] / length,
        real[1] / length,
        real[2] / length,
        real[3] / length,
    ];
    let dual = [
        dual[0] / length,
        dual[1] / length,
        dual[2] / length,
        dual[3] / length,
    ];
    // The vector part of dual * conjugate(real) is half the translation.
    let (r, d) = ([real[0], real[1], real[2]], [dual[0], dual[1], dual[2]]);
    let rd = cross(r, d);
    let t = [
        real[3] * d[0] - dual[3] * r[0] + rd[0],
        real[3] * d[1] - dual[3] * r[1] + rd[1],
        real[3] * d[2] - dual[3] * r[2] + rd[2],
    ];
    let moved = rotate(
        real,
        [
            f64::from(position[0]),
            f64::from(position[1]),
            f64::from(position[2]),
        ],
    );
    let turned = rotate(
        real,
        [
            f64::from(normal[0]),
            f64::from(normal[1]),
            f64::from(normal[2]),
        ],
    );
    #[expect(clippy::cast_possible_truncation, reason = "back to the f32 mesh")]
    let out = (
        [
            (moved[0] + 2.0 * t[0]) as f32,
            (moved[1] + 2.0 * t[1]) as f32,
            (moved[2] + 2.0 * t[2]) as f32,
        ],
        [turned[0] as f32, turned[1] as f32, turned[2] as f32],
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const IDENTITY: [f32; 16] = [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ];
    const QUARTER_Z: [f32; 16] = [
        0.0, 1.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ];

    #[test]
    fn halfway_between_rest_and_a_quarter_turn_is_a_45_degree_turn_that_keeps_its_length() {
        let (p, n) = deform_dual(
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0, 1, 0, 0],
            [0.5, 0.5, 0.0, 0.0],
            &[IDENTITY, QUARTER_Z],
        );
        let half = std::f32::consts::FRAC_1_SQRT_2;
        assert!(
            (p[0] + half).abs() < 1e-6 && (p[1] - half).abs() < 1e-6,
            "{p:?}"
        );
        assert!(
            ((p[0] * p[0] + p[1] * p[1]).sqrt() - 1.0).abs() < 1e-6,
            "the linear blend gives 0.707 long; this keeps 1"
        );
        assert!(
            (n[0] + half).abs() < 1e-6 && (n[1] - half).abs() < 1e-6,
            "{n:?}"
        );
    }

    #[test]
    fn one_bone_matches_the_linear_blend_and_pure_translation_blends_linearly() {
        let (p, _) = deform_dual(
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [1, 0, 0, 0],
            [1.0, 0.0, 0.0, 0.0],
            &[IDENTITY, QUARTER_Z],
        );
        assert!((p[0] + 1.0).abs() < 1e-6 && p[1].abs() < 1e-6, "{p:?}");
        let mut moved = IDENTITY;
        moved[12] = 2.0;
        let (p, _) = deform_dual(
            [0.5, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [0, 1, 0, 0],
            [0.5, 0.5, 0.0, 0.0],
            &[IDENTITY, moved],
        );
        assert!((p[0] - 1.5).abs() < 1e-6 && p[1].abs() < 1e-6, "{p:?}");
    }

    #[test]
    fn blending_takes_the_short_way_round() {
        // Turns of +170 and -170 degrees about Z are only 20 degrees apart the short way; their midpoint is a half
        // turn. A sum that ignored the quaternion sign would take the long way and land back at rest.
        let turn = |degrees: f32| {
            let (s, c) = degrees.to_radians().sin_cos();
            [
                c, s, 0.0, 0.0, -s, c, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
            ]
        };
        let (p, _) = deform_dual(
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0, 1, 0, 0],
            [0.5, 0.5, 0.0, 0.0],
            &[turn(170.0), turn(-170.0)],
        );
        assert!((p[0] + 1.0).abs() < 1e-5 && p[1].abs() < 1e-5, "{p:?}");
    }

    /// Cost of one frame's deformation per 100,000 corners, for choosing the skin budget (run with
    /// `cargo test -p ve-modeling --release --lib skin_cost -- --ignored --nocapture`).
    #[test]
    #[ignore = "a measurement, not a check"]
    fn skin_cost_per_hundred_thousand_corners() {
        let poses = [IDENTITY, QUARTER_Z, IDENTITY, QUARTER_Z];
        let prepared = prepare(&poses);
        let time = |dual: bool| {
            let start = std::time::Instant::now();
            let mut sink = 0.0_f32;
            for _ in 0..100_000 {
                let at = std::hint::black_box([0.3, 1.0, 0.0]);
                let (p, _) = if dual {
                    deform_prepared(
                        at,
                        [0.0, 1.0, 0.0],
                        [0, 1, 2, 3],
                        [0.4, 0.3, 0.2, 0.1],
                        &prepared,
                    )
                } else {
                    super::super::deform(
                        at,
                        [0.0, 1.0, 0.0],
                        [0, 1, 2, 3],
                        [0.4, 0.3, 0.2, 0.1],
                        &poses,
                    )
                };
                sink += p[0];
            }
            (start.elapsed(), sink)
        };
        println!(
            "linear {:?}, dual quaternion {:?}",
            time(false).0,
            time(true).0
        );
    }

    #[test]
    fn rotation_extraction_round_trips_each_axis_branch() {
        // Half turns about each axis exercise every branch of the matrix to quaternion conversion.
        let about_x: [f32; 16] = [
            1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ];
        let about_y: [f32; 16] = [
            -1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ];
        let about_z: [f32; 16] = [
            -1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ];
        for (pose, expect) in [
            (about_x, [1.0, -1.0, -1.0]),
            (about_y, [-1.0, 1.0, -1.0]),
            (about_z, [-1.0, -1.0, 1.0]),
        ] {
            let (p, _) = deform_dual(
                [1.0, 1.0, 1.0],
                [0.0, 0.0, 1.0],
                [0, 0, 0, 0],
                [1.0, 0.0, 0.0, 0.0],
                &[pose],
            );
            for axis in 0..3 {
                assert!((p[axis] - expect[axis]).abs() < 1e-6, "{p:?} vs {expect:?}");
            }
        }
    }
}
