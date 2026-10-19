//! The skeleton, the skin and the clips. Bones are written as rig bones with keyframed rotation tracks picked by role.
use super::{Creature, Role, num, vec3};
use std::f64::consts::TAU;
use std::fmt::Write;

/// One bone as it appears in the rig: a centre bone once, a paired bone as a left and a right copy.
struct Instance {
    /// The node and output name, for example `girl_thigh_l`.
    name: String,
    bone: usize,
    /// 0 for a centre bone, +1 for the left (+x) side, -1 for the right.
    side: i32,
}

/// A named motion cycle: its name, its one-letter node suffix, its length in seconds, how strongly the
/// bones move, and whether it repeats.
struct Clip {
    /// The clip name; empty is the part's main clip (the walk).
    name: &'static str,
    /// One letter unique to the clip, so its track node name stays short.
    letter: &'static str,
    period: f64,
    strength: f64,
    /// A cycle loops through its keys; a one-shot (`jump`) holds its last pose and writes `loop=false`.
    looping: bool,
}

const WALK: Clip = Clip {
    name: "",
    letter: "w",
    period: 1.0,
    strength: 1.0,
    looping: true,
};
const RUN: Clip = Clip {
    name: "run",
    letter: "r",
    period: 0.6,
    strength: 1.5,
    looping: true,
};
const IDLE: Clip = Clip {
    name: "idle",
    letter: "i",
    period: 3.0,
    strength: 0.0,
    looping: true,
};
const FLY: Clip = Clip {
    name: "fly",
    letter: "f",
    period: 0.8,
    strength: 1.0,
    looping: true,
};
const JUMP: Clip = Clip {
    name: "jump",
    letter: "j",
    period: 0.8,
    strength: 1.0,
    looping: false,
};
const FALL: Clip = Clip {
    name: "fall",
    letter: "d",
    period: 0.6,
    strength: 1.0,
    looping: true,
};

fn mirrored(point: [f64; 3], side: i32) -> [f64; 3] {
    if side < 0 {
        [-point[0], point[1], point[2]]
    } else {
        point
    }
}

/// How many ancestors of this bone share its role (0 for the top of a limb or the first tail segment).
fn depth(creature: &Creature, bone: usize) -> usize {
    let role = creature.bones[bone].role;
    let mut count = 0;
    let mut current = &creature.bones[bone];
    while let Some(parent) = current
        .parent
        .as_ref()
        .and_then(|p| creature.bones.iter().find(|b| &b.name == p))
    {
        if parent.role != role {
            break;
        }
        count += 1;
        current = parent;
    }
    count
}

/// Rotation of one bone in degrees at the fraction `u` of a cycle, or none if it holds still in this clip.
fn rotation(creature: &Creature, instance: &Instance, clip: &Clip, u: f64) -> Option<[f64; 3]> {
    if clip.name == "jump" {
        return jump_rotation(creature, instance, u);
    }
    if clip.name == "fall" {
        return fall_rotation(creature, instance, u);
    }
    let bone = &creature.bones[instance.bone];
    let level = depth(creature, instance.bone);
    let phase = bone.phase + if instance.side < 0 { 0.5 } else { 0.0 };
    let wave = |offset: f64| (TAU * (u + offset)).sin();
    let moving = clip.strength > 0.0;
    let s = clip.strength;
    let sign = f64::from(if instance.side < 0 { -1 } else { 1 });
    match bone.role {
        Role::Leg | Role::Arm if moving => {
            let down = if bone.role == Role::Leg { 1.0 } else { -1.0 };
            Some(if level == 0 {
                [28.0 * s * wave(phase), 0.0, 0.0]
            } else {
                [
                    down * (6.0 + 36.0 * s * (0.5 + 0.5 * wave(phase + 0.25))),
                    0.0,
                    0.0,
                ]
            })
        }
        Role::Arm => Some([2.5 * wave(phase), 0.0, 0.0]),
        Role::Spine if moving => Some([0.0, 4.0 * s * wave(0.0), 0.0]),
        Role::Spine => Some([1.5 * wave(0.0), 0.0, 0.0]),
        Role::Neck | Role::Head if moving => Some([2.5 * s * wave(0.25), 0.0, 0.0]),
        Role::Neck | Role::Head => Some([2.0 * wave(0.25), 0.0, 0.0]),
        Role::Tail => {
            let lag = 0.15 * f64::from(u32::try_from(level).unwrap_or(0));
            let amplitude = if moving { 14.0 * s } else { 6.0 };
            Some([0.0, amplitude * wave(-lag), 0.0])
        }
        Role::Wing if clip.name == "fly" => Some([0.0, 0.0, sign * 45.0 * wave(0.0)]),
        Role::Wing if moving => Some([0.0, 0.0, sign * 10.0 * s * wave(0.0)]),
        Role::Wing => Some([0.0, 0.0, sign * 4.0 * wave(0.0)]),
        _ => None,
    }
}

/// One of five values picked by the clip fraction `u`, which is a sampled fifth (0, 1/4, 1/2, 3/4, 1).
fn fifth(values: [f64; 5], u: f64) -> f64 {
    let index = [0.125, 0.375, 0.625, 0.875]
        .iter()
        .filter(|threshold| u >= **threshold)
        .count();
    values[index]
}

/// Rotation of one bone at a sampled fifth of the jump one-shot: crouch, spring, peak, tuck, land.
///
/// A jump is symmetric, so the two sides hold the same pose; quadrupeds crouch and extend all four legs.
fn jump_rotation(creature: &Creature, instance: &Instance, u: f64) -> Option<[f64; 3]> {
    let bone = &creature.bones[instance.bone];
    let level = depth(creature, instance.bone);
    let sign = f64::from(if instance.side < 0 { -1 } else { 1 });
    let down = if bone.role == Role::Arm { -1.0 } else { 1.0 };
    match bone.role {
        Role::Leg | Role::Arm if level == 0 => Some([
            fifth(
                if bone.role == Role::Leg {
                    [32.0, 4.0, -8.0, 26.0, 2.0]
                } else {
                    [22.0, -35.0, -78.0, -8.0, 6.0]
                },
                u,
            ),
            0.0,
            0.0,
        ]),
        Role::Leg | Role::Arm => Some([
            down * fifth(
                if bone.role == Role::Leg {
                    [58.0, 18.0, 2.0, 50.0, 8.0]
                } else {
                    [30.0, 18.0, 12.0, 28.0, 18.0]
                },
                u,
            ),
            0.0,
            0.0,
        ]),
        Role::Spine => Some([fifth([16.0, 6.0, -12.0, 8.0, 0.0], u), 0.0, 0.0]),
        Role::Neck | Role::Head => Some([fifth([10.0, 3.0, -8.0, 6.0, 0.0], u), 0.0, 0.0]),
        Role::Tail => Some([fifth([-8.0, -16.0, -20.0, -10.0, -2.0], u), 0.0, 0.0]),
        Role::Wing => Some([0.0, 0.0, sign * fifth([42.0, 50.0, 58.0, 48.0, 42.0], u)]),
        Role::Free => None,
    }
}

/// Rotation of one bone at a sampled fifth of the looping fall: a held pose with a small flutter.
///
/// Everybody's legs are slightly bent and spread, the arms are out and up, and tails and wings flutter.
/// Every flutter term repeats at `u = 0` and `u = 1`, so the loop joins without a jump.
fn fall_rotation(creature: &Creature, instance: &Instance, u: f64) -> Option<[f64; 3]> {
    let bone = &creature.bones[instance.bone];
    let level = depth(creature, instance.bone);
    let phase = bone.phase + if instance.side < 0 { 0.5 } else { 0.0 };
    let wave = |offset: f64| (TAU * (u + offset)).sin();
    let sign = f64::from(if instance.side < 0 { -1 } else { 1 });
    let down = if bone.role == Role::Arm { -1.0 } else { 1.0 };
    match bone.role {
        Role::Leg | Role::Arm if level == 0 => {
            let (bend, spread) = if bone.role == Role::Leg {
                (10.0 + 4.0 * wave(phase), 10.0)
            } else {
                (-45.0 + 6.0 * wave(phase), 22.0)
            };
            Some([bend, 0.0, sign * spread])
        }
        Role::Leg | Role::Arm => {
            let base = if bone.role == Role::Leg { 20.0 } else { 14.0 };
            Some([down * (base + 4.0 * wave(phase)), 0.0, 0.0])
        }
        Role::Spine => Some([4.0 + 2.0 * wave(phase), 0.0, 0.0]),
        Role::Neck | Role::Head => Some([8.0 + 3.0 * wave(phase), 0.0, 0.0]),
        Role::Tail => {
            let lag = 0.15 * f64::from(u32::try_from(level).unwrap_or(0));
            Some([0.0, 12.0 * wave(-lag), 0.0])
        }
        Role::Wing => Some([0.0, 0.0, sign * (46.0 + 8.0 * wave(phase))]),
        Role::Free => None,
    }
}

fn track(
    text: &mut String,
    node: &mut String,
    instance: &Instance,
    creature: &Creature,
    clip: &Clip,
) {
    let keys: Vec<(f64, [f64; 3])> = (0..=4)
        .filter_map(|k| {
            let u = f64::from(k) / 4.0;
            rotation(creature, instance, clip, u).map(|r| (u * clip.period, r))
        })
        .collect();
    if keys.len() != 5 || keys.iter().all(|(_, r)| r.iter().all(|v| v.abs() < 1e-9)) {
        return;
    }
    let keys = keys
        .iter()
        .map(|(t, r)| format!("{}:{},{},{}", num(*t), num(r[0]), num(r[1]), num(r[2])))
        .collect::<Vec<_>>()
        .join("; ");
    let head = mirrored(creature.bones[instance.bone].head, instance.side);
    let head = [
        head[0] + creature.at[0],
        head[1] + creature.at[1],
        head[2] + creature.at[2],
    ];
    // One letter per clip so the node name stays short: walk, run, idle, fly, jump, fall.
    let next = format!("{}_{}", instance.name, clip.letter);
    let clip_param = if clip.name.is_empty() {
        String::new()
    } else {
        format!(", clip=\"{}\"", clip.name)
    };
    // A one-shot holds its last pose instead of starting over.
    let loop_param = if clip.looping { "" } else { ", loop=false" };
    let _ = writeln!(
        text,
        "{next} = anim.track@1({node}, property=\"rotation\", keys=\"{keys}\", interpolation=\"bezier\"{clip_param}, pivot={}{loop_param})",
        vec3(head)
    );
    *node = next;
}

/// Writes the rig for `creature`, skins `solid` to it and gives it a toon material.
pub(super) fn write(creature: &Creature, solid: &str, text: &mut String) {
    let prefix = &creature.name;
    let mut instances = Vec::new();
    for (index, bone) in creature.bones.iter().enumerate() {
        let sides: &[(i32, &str)] = if bone.pair {
            &[(1, "_l"), (-1, "_r")]
        } else {
            &[(0, "")]
        };
        for (side, suffix) in sides {
            instances.push(Instance {
                name: format!("{prefix}_{}{suffix}", bone.name),
                bone: index,
                side: *side,
            });
        }
    }
    let _ = writeln!(text, "{prefix}_knot = solid.box@1(size=[0.02, 0.02, 0.02])");
    let has_wings = creature.bones.iter().any(|b| b.role == Role::Wing);
    for instance in &instances {
        let bone = &creature.bones[instance.bone];
        let shift = |p: [f64; 3]| {
            let p = mirrored(p, instance.side);
            [
                p[0] + creature.at[0],
                p[1] + creature.at[1],
                p[2] + creature.at[2],
            ]
        };
        let (head, tail) = (shift(bone.head), shift(bone.tail));
        let name = &instance.name;
        let _ = writeln!(
            text,
            "{name}_a = xform.move@1({prefix}_knot, by={})",
            vec3(head)
        );
        let _ = writeln!(
            text,
            "{name}_b = rig.bone@1({name}_a, head={}, tail={})",
            vec3(head),
            vec3(tail)
        );
        let mut node = format!("{name}_b");
        if let Some(parent) = &bone.parent {
            let parent_side = creature
                .bones
                .iter()
                .find(|b| &b.name == parent)
                .map_or(0, |p| if p.pair { instance.side } else { 0 });
            let suffix = match parent_side {
                1 => "_l",
                -1 => "_r",
                _ => "",
            };
            let _ = writeln!(
                text,
                "{name}_p = anim.parent@1({node}, parent=\"{prefix}_{parent}{suffix}\")"
            );
            node = format!("{name}_p");
        }
        for clip in [&WALK, &RUN, &IDLE, &JUMP, &FALL] {
            track(text, &mut node, instance, creature, clip);
        }
        if has_wings {
            track(text, &mut node, instance, creature, &FLY);
        }
        let _ = writeln!(text, "output {name} = {node}");
    }

    let bones = instances
        .iter()
        .map(|i| i.name.clone())
        .collect::<Vec<_>>()
        .join("; ");
    let [r, g, b] = creature.color;
    let shade = [r * 0.7 + 0.05, g * 0.6 + 0.04, b * 0.85 + 0.12].map(|v| v.min(1.0));
    let line = [r * 0.35, g * 0.35, b * 0.35];
    let _ = writeln!(
        text,
        "{prefix}_skin = rig.skin@1({solid}, bones=\"{bones}\", influences=2, falloff=3)"
    );
    let _ = writeln!(
        text,
        "{prefix}_mat = solid.material@1({prefix}_skin, color={}, roughness=0.6)",
        vec3(creature.color)
    );
    let _ = writeln!(
        text,
        "{prefix}_toon = solid.toon@1({prefix}_mat, bands=2, feather=0.05, shade_color={}, rim=0.2, outline={}, outline_color={})",
        vec3(shade),
        num(creature.outline),
        vec3(line)
    );
    let _ = writeln!(text, "output {prefix} = {prefix}_toon");
}
