use super::{Bone, Creature, Role, Sliders, blob, bone, paired};

pub(super) fn quadruped(s: &Sliders) -> Creature {
    let (hd, lm, tr, lg, nk, tl) = (s.head, s.limbs, s.torso, s.legs, s.neck, s.tail);
    let hip_y = 0.7 * lg + 0.05;
    let neck_y = hip_y + 0.1;
    let head_at = [0.0, neck_y + 0.55 * nk + 0.05, 0.35 + 0.38 * nk + 0.08];
    let tail_z = |k: f64| -0.4 - 0.5 * tl * k;
    let mut bones = vec![
        bone(
            "pelvis",
            None,
            [0.0, hip_y, -0.3],
            [0.0, hip_y, -0.1],
            [0.2 * tr, 0.2 * tr],
            Role::Free,
        ),
        bone(
            "spine",
            Some("pelvis"),
            [0.0, hip_y, -0.2],
            [0.0, hip_y + 0.02, 0.35],
            [0.17 * tr, 0.16 * tr],
            Role::Spine,
        ),
        bone(
            "neck1",
            Some("spine"),
            [0.0, neck_y, 0.35],
            [0.0, neck_y + 0.28 * nk, 0.35 + 0.2 * nk],
            [0.15, 0.11],
            Role::Neck,
        ),
        bone(
            "neck2",
            Some("neck1"),
            [0.0, neck_y + 0.28 * nk, 0.35 + 0.2 * nk],
            [0.0, neck_y + 0.55 * nk, 0.35 + 0.38 * nk],
            [0.11, 0.075 * hd],
            Role::Neck,
        ),
        bone(
            "head",
            Some("neck2"),
            [0.0, neck_y + 0.55 * nk, 0.35 + 0.38 * nk],
            head_at,
            [0.075 * hd, 0.06 * hd],
            Role::Head,
        ),
        bone(
            "tail1",
            Some("pelvis"),
            [0.0, hip_y, -0.35],
            [0.0, hip_y - 0.08, tail_z(0.4)],
            [0.14, 0.1],
            Role::Tail,
        ),
        bone(
            "tail2",
            Some("tail1"),
            [0.0, hip_y - 0.08, tail_z(0.4)],
            [0.0, hip_y - 0.12, tail_z(0.75)],
            [0.1, 0.06],
            Role::Tail,
        ),
        bone(
            "tail3",
            Some("tail2"),
            [0.0, hip_y - 0.12, tail_z(0.75)],
            [0.0, hip_y, tail_z(1.0)],
            [0.06, 0.015],
            Role::Tail,
        ),
        paired(
            bone(
                "fore_upper",
                Some("spine"),
                [0.17 * tr, hip_y - 0.02, 0.33],
                [0.18 * tr, hip_y * 0.5, 0.37],
                [0.1 * lm, 0.075 * lm],
                Role::Leg,
            ),
            0.0,
        ),
        paired(
            bone(
                "fore_lower",
                Some("fore_upper"),
                [0.18 * tr, hip_y * 0.5, 0.37],
                [0.17 * tr, 0.04, 0.35],
                [0.075 * lm, 0.045 * lm],
                Role::Leg,
            ),
            0.0,
        ),
        paired(
            bone(
                "hind_upper",
                Some("pelvis"),
                [0.17 * tr, hip_y, -0.3],
                [0.19 * tr, hip_y * 0.55, -0.4],
                [0.13 * lm, 0.08 * lm],
                Role::Leg,
            ),
            0.5,
        ),
        paired(
            bone(
                "hind_lower",
                Some("hind_upper"),
                [0.19 * tr, hip_y * 0.55, -0.4],
                [0.17 * tr, 0.04, -0.33],
                [0.08 * lm, 0.045 * lm],
                Role::Leg,
            ),
            0.5,
        ),
    ];
    let mut blobs = vec![
        blob(
            "torso",
            [0.0, hip_y, 0.0],
            [0.24 * tr, 0.22 * tr, 0.5],
            false,
        ),
        blob(
            "skull",
            [head_at[0], head_at[1], head_at[2] + 0.04],
            [0.085 * hd, 0.075 * hd, 0.15 * hd],
            false,
        ),
    ];
    if s.horns {
        bones.push(paired(
            Bone {
                via: vec![[0.09 * hd, head_at[1] + 0.13, head_at[2] - 0.02]],
                ..bone(
                    "horn",
                    Some("head"),
                    [0.045 * hd, head_at[1] + 0.05, head_at[2] + 0.02],
                    [0.1 * hd, head_at[1] + 0.24, head_at[2] - 0.1],
                    [0.032 * hd, 0.008 * hd],
                    Role::Free,
                )
            },
            0.0,
        ));
    }
    if s.wings {
        let shoulder = [0.2 * tr, hip_y + 0.17, 0.2];
        let wrist = [0.7, hip_y + 0.4, 0.0];
        let finger1 = [1.15, hip_y + 0.3, -0.25];
        let finger2 = [1.0, hip_y + 0.22, -0.45];
        // The membrane: flattened blobs laid on the surface between the arm line (shoulder to wrist) and the finger tips,
        // which blend into one sheet. `inner` runs along the body edge, `outer` along the arm and the first finger.
        for (row, v) in [0.15, 0.5, 0.85].into_iter().enumerate() {
            for (column, u) in [0.35, 0.75].into_iter().enumerate() {
                let lerp = |a: [f64; 3], b: [f64; 3], t: f64| {
                    [
                        a[0] + (b[0] - a[0]) * t,
                        a[1] + (b[1] - a[1]) * t,
                        a[2] + (b[2] - a[2]) * t,
                    ]
                };
                let inner = lerp(shoulder, finger2, v);
                let outer = lerp(wrist, finger1, v);
                let at = lerp(inner, outer, u);
                blobs.push(blob(
                    &format!("wm{row}{column}"),
                    at,
                    [0.17, 0.05, 0.17],
                    true,
                ));
            }
        }
        bones.push(paired(
            bone(
                "wing_arm",
                Some("spine"),
                shoulder,
                wrist,
                [0.045, 0.03],
                Role::Wing,
            ),
            0.0,
        ));
        bones.push(paired(
            bone(
                "wing_finger1",
                Some("wing_arm"),
                wrist,
                finger1,
                [0.025, 0.012],
                Role::Wing,
            ),
            0.0,
        ));
        bones.push(paired(
            bone(
                "wing_finger2",
                Some("wing_arm"),
                wrist,
                finger2,
                [0.025, 0.012],
                Role::Wing,
            ),
            0.0,
        ));
    }
    Creature {
        name: "quadruped".to_owned(),
        bones,
        blobs,
        color: s.color.unwrap_or([0.3, 0.6, 0.42]),
        outline: 1.8,
        resolution: 80,
        at: [0.0; 3],
    }
}
