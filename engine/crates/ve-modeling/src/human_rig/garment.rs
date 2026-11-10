use std::rc::Rc;

use ve_graph::{Node, Value as Param};
use ve_kernel::Mesh;

use super::{
    COVERS, bone_list, clean, covers, fail, fail_suggest, hem, pack_or_fail, rig_of, skinning,
    text, weights_for,
};
use crate::{Fail, cache::Value};

pub(super) fn run(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let (body, rig) = rig_of(node, inputs)?;
    let human = pack_or_fail()?;
    let cover = text(node, "cover");
    if !COVERS.contains(&cover) {
        return Err(fail_suggest(
            "param.invalid",
            format!("{cover:?} is not a garment"),
            format!("the garments are: {}", COVERS.join(", ")),
        ));
    }
    let offset = match node.params.get("offset") {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "an offset of a few millimetres"
        )]
        Some(Param::Num(n)) => *n as f32,
        _ => 0.006,
    };
    let source = &body.human_source_indices;
    let bone_weights = human.skin();
    if source.len() != body.mesh.positions.len()
        || source
            .iter()
            .any(|vertex| *vertex as usize >= bone_weights.len())
    {
        return Err(fail(
            "human.skin_mesh",
            "the body's surface changed after body.human, so a garment cannot be cut from it",
        ));
    }
    let cleaned: Vec<String> = rig.bone_names.iter().map(|n| clean(n)).collect();
    let inside: Vec<bool> = source
        .iter()
        .map(|vertex| {
            bone_weights
                .get(*vertex as usize)
                .and_then(|w| w.iter().max_by(|a, b| a.1.total_cmp(&b.1)))
                .is_some_and(|(bone, _)| covers(cover, &cleaned[usize::from(*bone)]))
        })
        .collect();
    let positions = &body.mesh.positions;
    let mut normals = vec![[0.0_f32; 3]; positions.len()];
    for tri in body.mesh.indices.chunks_exact(3) {
        let p = |i: u32| positions[i as usize];
        let (a, b, c) = (p(tri[0]), p(tri[1]), p(tri[2]));
        let (u, v) = (
            [b[0] - a[0], b[1] - a[1], b[2] - a[2]],
            [c[0] - a[0], c[1] - a[1], c[2] - a[2]],
        );
        let n = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        for i in tri {
            for axis in 0..3 {
                normals[*i as usize][axis] += n[axis];
            }
        }
    }
    let mut remap = vec![u32::MAX; positions.len()];
    let (mut kept_positions, mut kept_indices, mut kept_uvs, mut kept_from) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for (t, tri) in body.mesh.indices.chunks_exact(3).enumerate() {
        if tri.iter().filter(|i| inside[**i as usize]).count() < 2 {
            continue;
        }
        for (corner, i) in tri.iter().enumerate() {
            let slot = &mut remap[*i as usize];
            if *slot == u32::MAX {
                *slot = u32::try_from(kept_positions.len()).unwrap_or(0);
                let n = normals[*i as usize];
                let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-9);
                let p = positions[*i as usize];
                kept_positions.push([
                    p[0] + n[0] / length * offset,
                    p[1] + n[1] / length * offset,
                    p[2] + n[2] / length * offset,
                ]);
                kept_from.push(*i as usize);
            }
            kept_indices.push(*slot);
            if let Some(uv) = body.uvs.get(t * 3 + corner) {
                kept_uvs.push(*uv);
            }
        }
    }
    if kept_indices.is_empty() {
        return Err(fail(
            "human.garment_empty",
            format!("no part of the body is covered by {cover:?}"),
        ));
    }
    let mut garment_mesh = Mesh {
        positions: kept_positions,
        indices: kept_indices,
    };
    let mut garment_nearest = kept_from;
    if matches!(node.params.get("hem"), Some(Param::Bool(true))) {
        let clipped = hem::apply(garment_mesh, kept_uvs, garment_nearest);
        garment_mesh = clipped.mesh;
        kept_uvs = clipped.uvs;
        garment_nearest = clipped.nearest;
    }
    let mut value = Value::solid(garment_mesh, 0.0);
    value.uvs = kept_uvs;
    let bones = bone_list(node, "body.garment")?;
    if !bones.is_empty() {
        let (joints, weights) = weights_for(&rig, human, &bones, source)?;
        value.skin = Some(skinning(
            node,
            bones,
            garment_nearest.iter().map(|i| joints[*i]).collect(),
            garment_nearest.iter().map(|i| weights[*i]).collect(),
        ));
    }
    Ok(value)
}
