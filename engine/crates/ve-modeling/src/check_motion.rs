//! Post-evaluation sampling for `check.motion`.
use std::collections::BTreeMap;

use glam::{EulerRot, Mat4, Quat, Vec3};
use ve_graph::{Document, Node, Output, Value as Param};

use crate::{
    EvaluationWarning, Fail, Part,
    anim::{Track, TrackProperty},
    fail,
};

pub(crate) fn evaluate(
    doc: &Document,
    outputs: &[Output],
    parts: &[Part],
) -> Result<Vec<EvaluationWarning>, Fail> {
    let by_name: BTreeMap<_, _> = parts
        .iter()
        .enumerate()
        .map(|(i, p)| (p.name.as_str(), i))
        .collect();
    let output_names: BTreeMap<_, _> = outputs
        .iter()
        .map(|o| (o.from.node.clone(), o.name.as_str()))
        .collect();
    let mut warnings = Vec::new();
    for node in &doc.nodes {
        if node.op.family().as_str() != "check.motion" {
            continue;
        }
        let names: Vec<_> = node
            .inputs
            .get("parts")
            .into_iter()
            .flatten()
            .filter_map(|r| output_names.get(&r.node).copied())
            .collect();
        let selected: Vec<_> = names
            .iter()
            .filter_map(|name| by_name.get(name).copied())
            .collect();
        if selected.is_empty() {
            continue;
        }
        let clip = string(node, "clip", "");
        let duration = number(node, "duration", 1.0) as f32;
        let fps = integer(node, "fps", 30) as usize;
        let frames = ((duration * fps as f32).round() as usize).max(2);
        if frames > 3600 {
            return Err(at_node(
                fail(
                    "param.range",
                    "duration times fps must not exceed 3,600 frames",
                ),
                node,
            ));
        }
        let speed = number(node, "speed", 0.0) as f32;
        let slide_max = number(node, "slide_max", 0.1) as f32;
        let severity = string(node, "severity", "warning");
        let mut sampled = selected.clone();
        for &start in &selected {
            let mut parent = parts[start].parent.as_deref();
            let mut depth = 0;
            while let Some(name) = parent {
                depth += 1;
                if depth > crate::hierarchy::MAX_DEPTH {
                    break;
                }
                let Some(&index) = by_name.get(name) else {
                    break;
                };
                if !sampled.contains(&index) {
                    sampled.push(index);
                }
                parent = parts[index].parent.as_deref();
            }
        }
        if !sampled
            .iter()
            .any(|&i| parts[i].animation.iter().any(|track| track.clip == clip))
        {
            push(
                &mut warnings,
                finding(
                    node,
                    "motion.no_tracks",
                    format!("no part has an animation track in clip \"{clip}\""),
                ),
                severity,
                node,
            )?;
            continue;
        }
        let mut foot_points = vec![Vec::with_capacity(frames); selected.len()];
        let mut seam_frames = Vec::with_capacity(frames);
        for frame in 0..frames {
            let t = frame as f32 / fps as f32;
            let mut centres = Vec::with_capacity(selected.len());
            for (slot, &index) in selected.iter().enumerate() {
                let matrix = world_matrix(index, t, &clip, parts, &by_name, 0);
                let (bottom, centre) = bounds(&parts[index]);
                foot_points[slot].push(
                    (matrix.transform_point3(bottom) + Vec3::new(speed * t, 0.0, 0.0)).to_array(),
                );
                centres.push(matrix.transform_point3(centre).to_array());
            }
            seam_frames.push(centres);
        }
        for foot_name in string(node, "feet", "")
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            let Some(&index) = by_name.get(foot_name) else {
                return Err(at_node(
                    fail(
                        "check.motion.unknown_foot",
                        format!(
                            "feet names \"{foot_name}\", but no evaluated part has that output name"
                        ),
                    ),
                    node,
                ));
            };
            let Some(slot) = selected.iter().position(|&i| i == index) else {
                return Err(at_node(
                    fail(
                        "check.motion.unknown_foot",
                        format!(
                            "feet names \"{foot_name}\", but it is not among this check's parts"
                        ),
                    ),
                    node,
                ));
            };
            let measured =
                ve_measure::motion::foot_slide(&foot_points[slot], 1.0 / fps as f32, 0.02)
                    .map_err(|e| at_node(fail("check.motion.measure", e.to_string()), node))?;
            if measured.mean_speed > slide_max {
                let warning = finding_part(
                    &parts[index],
                    "motion.foot_slide",
                    format!(
                        "the foot \"{foot_name}\" slides at {:.2} m/s on average ({slide_max:.2} at most) while planted in clip \"{clip}\"; at a body speed of {speed:.2} m/s it should stand still in the world. Shorten the swing, or match it to the travel speed: a planted foot moves back as fast as the body moves forward.",
                        measured.mean_speed
                    ),
                );
                push(&mut warnings, warning, severity, node)?;
            }
        }
        let seam = ve_measure::motion::loop_seam(&seam_frames)
            .map_err(|e| at_node(fail("check.motion.measure", e.to_string()), node))?;
        if !seam.seamless() {
            let warning = finding_part(
                &parts[selected[0]],
                "motion.loop_seam",
                format!(
                    "clip \"{clip}\" does not loop cleanly: its last frame is {:.2} m from its first (a step is {:.2} m). Make the last key equal the first and set loop=true on every track of the clip.",
                    seam.gap, seam.typical_step
                ),
            );
            push(&mut warnings, warning, severity, node)?;
        }
    }
    Ok(warnings)
}

fn world_matrix(
    index: usize,
    t: f32,
    clip: &str,
    parts: &[Part],
    by_name: &BTreeMap<&str, usize>,
    depth: usize,
) -> Mat4 {
    let part = &parts[index];
    let mut matrix = Mat4::IDENTITY;
    for track in part.animation.iter().filter(|track| track.clip == clip) {
        matrix = track_matrix(track, t) * matrix;
    }
    if depth < crate::hierarchy::MAX_DEPTH
        && let Some(parent) = part.parent.as_deref()
        && let Some(&parent_index) = by_name.get(parent)
    {
        matrix = world_matrix(parent_index, t, clip, parts, by_name, depth + 1) * matrix;
    }
    matrix
}

fn track_matrix(track: &Track, t: f32) -> Mat4 {
    let value = Vec3::from_array(track.sample(t));
    let pivot = Vec3::from_array(track.pivot);
    let about = |change| Mat4::from_translation(pivot) * change * Mat4::from_translation(-pivot);
    match track.property {
        TrackProperty::Position => Mat4::from_translation(value),
        TrackProperty::Rotation => about(Mat4::from_quat(Quat::from_euler(
            EulerRot::YXZ,
            value.y.to_radians(),
            value.x.to_radians(),
            value.z.to_radians(),
        ))),
        TrackProperty::Scale => about(Mat4::from_scale(value)),
        _ => Mat4::IDENTITY,
    }
}

fn bounds(part: &Part) -> (Vec3, Vec3) {
    let mut low = Vec3::splat(f32::INFINITY);
    let mut high = Vec3::splat(f32::NEG_INFINITY);
    let mut bottom = None;
    for point in &part.positions {
        let point = Vec3::from_array(*point);
        low = low.min(point);
        high = high.max(point);
        if bottom.is_none_or(|current: Vec3| point.y < current.y) {
            bottom = Some(point);
        }
    }
    (bottom.unwrap_or(Vec3::ZERO), (low + high) * 0.5)
}

fn finding(node: &Node, code: &str, message: String) -> EvaluationWarning {
    EvaluationWarning {
        code: code.to_owned(),
        node: node.id.to_string(),
        related: Vec::new(),
        message,
    }
}
fn finding_part(part: &Part, code: &str, message: String) -> EvaluationWarning {
    EvaluationWarning {
        code: code.to_owned(),
        node: part.node.to_string(),
        related: Vec::new(),
        message,
    }
}
fn push(
    warnings: &mut Vec<EvaluationWarning>,
    warning: EvaluationWarning,
    severity: &str,
    node: &Node,
) -> Result<(), Fail> {
    if severity == "error" {
        return Err(at_node(fail(&warning.code, warning.message), node));
    }
    warnings.push(warning);
    Ok(())
}
fn at_node(mut error: Fail, node: &Node) -> Fail {
    error.node = Some(node.id.clone());
    error
}
fn string<'a>(node: &'a Node, key: &str, default: &'a str) -> &'a str {
    node.params
        .get(key)
        .and_then(|value| {
            if let Param::Str(text) = value {
                Some(text.as_str())
            } else {
                None
            }
        })
        .unwrap_or(default)
}
fn number(node: &Node, key: &str, default: f64) -> f64 {
    node.params
        .get(key)
        .and_then(Param::as_f64)
        .unwrap_or(default)
}
fn integer(node: &Node, key: &str, default: i64) -> i64 {
    node.params
        .get(key)
        .and_then(Param::as_f64)
        .map_or(default, |value| value as i64)
}
