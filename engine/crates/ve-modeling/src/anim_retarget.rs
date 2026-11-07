//! `anim.retarget`: copy the keyframe tracks of one rig's bone part onto another rig's bone part. A walk written for a short character plays on a tall
//! one (position keys scale with `scale`, rotations are about the bone's own joint, which follows the target's joint), and a left-arm clip becomes a
//! right-arm clip with `mirror`. Clips keep their names, so the target's state machine or behaviours use them as if they had been authored there.

use std::rc::Rc;

use ve_graph::{Node, Value as Param};

use crate::anim::{MAX_TRACKS, Track, TrackProperty};
use crate::cache::Value;
use crate::{Fail, fail, fail_suggest};

pub(crate) fn run(node: &Node, inputs: &[Rc<Value>]) -> Option<Result<Value, Fail>> {
    (node.op.family().as_str() == "anim.retarget").then(|| retarget(node, inputs))
}

fn mirrored(point: [f32; 3]) -> [f32; 3] {
    [-point[0], point[1], point[2]]
}

fn retarget(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    // Ports sort by name (`from` before `in`), so find each by its place among the node's links.
    let by_name = |name: &str| {
        node.inputs
            .keys()
            .position(|k| k == name)
            .and_then(|at| inputs.get(at))
    };
    let target = by_name("in").ok_or_else(|| {
        fail(
            "graph.missing_input",
            "anim.retarget needs the target part on `in`",
        )
    })?;
    let source = by_name("from").ok_or_else(|| {
        fail(
            "graph.missing_input",
            "anim.retarget needs the part whose clips to copy on `from`",
        )
    })?;
    let scale = node
        .params
        .get("scale")
        .and_then(Param::as_f64)
        .unwrap_or(1.0) as f32;
    let mirror = matches!(node.params.get("mirror"), Some(Param::Bool(true)));
    let only: Vec<String> = match node.params.get("clips") {
        Some(Param::Str(list)) => list
            .split([',', ';'])
            .map(str::trim)
            .filter(|c| !c.is_empty())
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    };
    let wanted: Vec<&Track> = source
        .animation
        .iter()
        .filter(|track| only.is_empty() || only.iter().any(|clip| *clip == track.clip))
        .collect();
    if wanted.is_empty() {
        return Err(fail_suggest(
            "anim.retarget_empty",
            if only.is_empty() {
                "the source part has no tracks to copy".to_owned()
            } else {
                format!("the source part has no tracks in the clips {only:?}")
            },
            "give the part on `from` anim.track nodes (and check the clip names)",
        ));
    }
    if target.animation.len() + wanted.len() > MAX_TRACKS {
        return Err(fail(
            "anim.too_many_tracks",
            format!(
                "the target would carry {} tracks; a part holds at most {MAX_TRACKS}",
                target.animation.len() + wanted.len()
            ),
        ));
    }
    // A rotation about the source bone's joint becomes one about the target bone's joint; without bones the pivot is only mirrored and scaled.
    let (from_head, to_head) = match (source.bone, target.bone) {
        (Some(a), Some(b)) => (Some(a.head), Some(b.head)),
        _ => (None, None),
    };
    let mut value = (**target).clone();
    for track in wanted {
        let mut copy = track.clone();
        for key in &mut copy.keys {
            match copy.property {
                TrackProperty::Position => {
                    let moved = [
                        key.value[0] * scale,
                        key.value[1] * scale,
                        key.value[2] * scale,
                    ];
                    key.value = if mirror { mirrored(moved) } else { moved };
                }
                TrackProperty::Rotation if mirror => {
                    key.value = [key.value[0], -key.value[1], -key.value[2]]
                }
                _ => {}
            }
        }
        copy.pivot = match (from_head, to_head) {
            (Some(from), Some(to)) => {
                let offset = [
                    track.pivot[0] - from[0],
                    track.pivot[1] - from[1],
                    track.pivot[2] - from[2],
                ];
                let offset = if mirror { mirrored(offset) } else { offset };
                [
                    to[0] + offset[0] * scale,
                    to[1] + offset[1] * scale,
                    to[2] + offset[2] * scale,
                ]
            }
            _ => {
                let pivot = [
                    track.pivot[0] * scale,
                    track.pivot[1] * scale,
                    track.pivot[2] * scale,
                ];
                if mirror { mirrored(pivot) } else { pivot }
            }
        };
        value.animation.push(copy);
    }
    Ok(value)
}
