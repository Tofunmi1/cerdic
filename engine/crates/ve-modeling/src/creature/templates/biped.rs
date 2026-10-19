use super::{Creature, Role, Sliders, blob, bone, paired, scaled};

pub(super) fn biped(s: &Sliders) -> Creature {
    let (hd, lm, tr, lg) = (s.head, s.limbs, s.torso, s.legs);
    let hip_y = 0.93 * lg + 0.03;
    let ankle_y = 0.08;
    let knee_y = f64::midpoint(hip_y, ankle_y);
    let shoulder_y = hip_y + 0.46;
    let neck_top = shoulder_y + 0.1;
    let head_r = scaled([0.125, 0.15, 0.135], hd);
    let head_y = neck_top + head_r[1] * 0.8;
    let wrist_y = shoulder_y - 0.56;
    let bones = vec![
        bone(
            "pelvis",
            None,
            [0.0, hip_y - 0.02, 0.0],
            [0.0, hip_y + 0.08, 0.0],
            [0.13 * tr, 0.12 * tr],
            Role::Free,
        ),
        bone(
            "spine",
            Some("pelvis"),
            [0.0, hip_y + 0.06, 0.0],
            [0.0, shoulder_y, 0.0],
            [0.13 * tr, 0.12 * tr],
            Role::Spine,
        ),
        bone(
            "neck",
            Some("spine"),
            [0.0, shoulder_y, 0.0],
            [0.0, neck_top, 0.005],
            [0.05, 0.045],
            Role::Neck,
        ),
        bone(
            "head",
            Some("neck"),
            [0.0, neck_top, 0.005],
            [0.0, head_y, 0.005],
            [0.05, 0.04],
            Role::Head,
        ),
        paired(
            bone(
                "thigh",
                Some("pelvis"),
                [0.09, hip_y, 0.0],
                [0.095, knee_y, 0.015],
                [0.085 * lm, 0.06 * lm],
                Role::Leg,
            ),
            0.0,
        ),
        paired(
            bone(
                "shin",
                Some("thigh"),
                [0.095, knee_y, 0.015],
                [0.09, ankle_y, 0.0],
                [0.06 * lm, 0.042 * lm],
                Role::Leg,
            ),
            0.0,
        ),
        paired(
            bone(
                "foot",
                Some("shin"),
                [0.09, ankle_y, 0.0],
                [0.095, 0.04, 0.13],
                [0.045, 0.038],
                Role::Free,
            ),
            0.0,
        ),
        paired(
            bone(
                "upper_arm",
                Some("spine"),
                [0.17 * tr, shoulder_y - 0.04, 0.0],
                [0.23 * tr, shoulder_y - 0.3, 0.01],
                [0.055 * lm, 0.045 * lm],
                Role::Arm,
            ),
            0.5,
        ),
        paired(
            bone(
                "forearm",
                Some("upper_arm"),
                [0.23 * tr, shoulder_y - 0.3, 0.01],
                [0.265 * tr, wrist_y, 0.0],
                [0.045 * lm, 0.033 * lm],
                Role::Arm,
            ),
            0.5,
        ),
    ];
    let blobs = vec![
        blob("skull", [0.0, head_y, 0.01], head_r, false),
        blob(
            "chest",
            [0.0, shoulder_y - 0.12, 0.0],
            [0.16 * tr, 0.19, 0.1],
            false,
        ),
        blob("hips", [0.0, hip_y, 0.0], [0.15 * tr, 0.11, 0.1], false),
        blob(
            "hand",
            [0.27 * tr, wrist_y - 0.03, 0.0],
            [0.035, 0.05, 0.035],
            true,
        ),
    ];
    Creature {
        name: "biped".to_owned(),
        bones,
        blobs,
        color: s.color.unwrap_or([0.96, 0.8, 0.7]),
        outline: 1.8,
        resolution: 72,
        at: [0.0; 3],
    }
}
