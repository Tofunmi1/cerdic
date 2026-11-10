//! Transfers authored body skin weights to a solid by exact nearest body vertex.
use super::{bone_list, pack_or_fail, skinning, weights_for};
use crate::{Fail, cache::Value, fail, fail_suggest};
use std::rc::Rc;
use ve_graph::Node;

pub(super) fn run(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    if inputs.len() < 2 {
        return Err(fail(
            "graph.missing_input",
            "body.skin_to needs a cloth and a body.human",
        ));
    }
    // The body is the input that carries a person; the other is the cloth (the evaluator hands the two over in the order of the port names).
    let (body, cloth) = if inputs[0].human.is_some() {
        (&inputs[0], &inputs[1])
    } else {
        (&inputs[1], &inputs[0])
    };
    let rig = body.human.as_ref().ok_or_else(|| {
        fail_suggest(
            "human.needs_body",
            "body.skin_to needs body.human as its body input",
            "connect body.human directly",
        )
    })?;
    let human = pack_or_fail()?;
    let bones = bone_list(node, "body.skin_to")?;
    if bones.is_empty() {
        return Err(fail("param.range", "body.skin_to needs at least one bone"));
    }
    let (joints, weights) = weights_for(rig, human, &bones, &body.human_source_indices)?;
    let mut output = (**cloth).clone();
    let mut out_j = Vec::with_capacity(cloth.mesh.positions.len());
    let mut out_w = Vec::with_capacity(cloth.mesh.positions.len());
    for p in &cloth.mesh.positions {
        let mut best = 0_usize;
        let mut best_d = f32::INFINITY;
        for (i, q) in body.mesh.positions.iter().enumerate() {
            let d = (p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2);
            if d < best_d {
                best = i;
                best_d = d;
            }
        }
        out_j.push(joints[best]);
        out_w.push(weights[best]);
    }
    output.skin = Some(skinning(node, bones, out_j, out_w));
    Ok(output)
}
