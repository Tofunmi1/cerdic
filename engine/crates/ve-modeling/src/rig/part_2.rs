
/// Why a chain cannot work, if it cannot: a code and a sentence.
fn chain_problem(
    chain: &IkChain,
    parts: &HashMap<String, (Option<Segment>, Option<String>)>,
) -> Option<(&'static str, String)> {
    let mut previous: Option<(&str, Segment)> = None;
    for name in &chain.bones {
        let Some((Some(segment), parent)) = parts.get(name) else {
            return Some((
                "rig.ik_bone_missing",
                format!("\"{name}\" is not an output part with a rig.bone"),
            ));
        };
        if let Some((before, before_segment)) = previous {
            if parent.as_deref() != Some(before) {
                return Some((
                    "rig.ik_unparented",
                    format!(
                        "\"{name}\" must be anim.parent-ed to \"{before}\" so it turns with it"
                    ),
                ));
            }
            let gap = before_segment
                .tail
                .iter()
                .zip(&segment.head)
                .map(|(a, b)| (a - b).abs())
                .fold(0.0_f32, f32::max);
            if gap > 0.01 {
                return Some((
                    "rig.ik_gap",
                    format!(
                        "\"{name}\" starts {gap:.2} m from where \"{before}\" ends; a joint must meet"
                    ),
                ));
            }
        }
        previous = Some((name, *segment));
    }
    None
}

fn warning(part: &Part, code: &str, message: String) -> EvaluationWarning {
    EvaluationWarning {
        code: code.to_owned(),
        node: part.node.to_string(),
        message,
    }
}

/// The distance from `point` to the segment.
fn distance(point: [f32; 3], segment: &Segment) -> f64 {
    let sub = |a: [f32; 3], b: [f32; 3]| {
        [
            f64::from(a[0] - b[0]),
            f64::from(a[1] - b[1]),
            f64::from(a[2] - b[2]),
        ]
    };
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let axis = sub(segment.tail, segment.head);
    let from_head = sub(point, segment.head);
    let length_squared = dot(axis, axis);
    let along = if length_squared > 0.0 {
        (dot(from_head, axis) / length_squared).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let off = [
        from_head[0] - axis[0] * along,
        from_head[1] - axis[1] * along,
        from_head[2] - axis[2] * along,
    ];
    dot(off, off).sqrt()
}

/// Weights for one vertex: `1 / (distance + epsilon)^falloff` per bone, the strongest `influences` kept and normalised.
fn weigh_vertex(
    point: [f32; 3],
    segments: &[Segment],
    falloff: u32,
    influences: usize,
    stamps: &[(usize, &Stamp)],
) -> ([u16; 4], [f32; 4]) {
    const EPSILON: f64 = 1e-3;
    let falloff = i32::try_from(falloff).unwrap_or(3);
    let mut raw: Vec<f64> = segments
        .iter()
        .map(|segment| 1.0 / (distance(point, segment) + EPSILON).powi(falloff))
        .collect();
    if !stamps.is_empty() {
        let sum: f64 = raw.iter().sum();
        for weight in &mut raw {
            *weight /= sum;
        }
        paint::apply(point, &mut raw, stamps);
    }
    let mut scored: Vec<(usize, f64)> = raw.into_iter().enumerate().collect();
    // Strongest first; equal strengths keep bone order, so the result never depends on a sort's tie-breaking.
    scored.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    scored.truncate(influences.clamp(1, MAX_INFLUENCES));
    let total: f64 = scored.iter().map(|(_, w)| w).sum();
    let (mut joints, mut weights) = ([0_u16; 4], [0.0_f32; 4]);
    for (slot, (bone, weight)) in scored.iter().enumerate() {
        joints[slot] = u16::try_from(*bone).unwrap_or(0);
        #[expect(clippy::cast_possible_truncation, reason = "a weight in 0 to 1")]
        {
            weights[slot] = (weight / total) as f32;
        }
    }
    (joints, weights)
}

/// Moves `point` by a column-major 4 x 4 matrix.
fn transform_point(m: &[f32; 16], p: [f32; 3]) -> [f32; 3] {
    [
        m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12],
        m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13],
        m[2] * p[0] + m[6] * p[1] + m[10] * p[2] + m[14],
    ]
}

/// Turns `vector` by a matrix's rotation and scale only.
fn transform_vector(m: &[f32; 16], v: [f32; 3]) -> [f32; 3] {
    [
        m[0] * v[0] + m[4] * v[1] + m[8] * v[2],
        m[1] * v[0] + m[5] * v[1] + m[9] * v[2],
        m[2] * v[0] + m[6] * v[1] + m[10] * v[2],
    ]
}

/// Where a skinned vertex is when the bones are posed: the weighted sum of the vertex moved by each influencing
/// bone's pose (`poses[joint]`, column-major, the identity at rest). Returns the position and the (unnormalised
/// blend of the) normal turned the same way, normalised.
#[must_use]
pub fn deform(
    position: [f32; 3],
    normal: [f32; 3],
    joints: [u16; 4],
    weights: [f32; 4],
    poses: &[[f32; 16]],
) -> ([f32; 3], [f32; 3]) {
    let (mut p, mut n) = ([0.0_f32; 3], [0.0_f32; 3]);
    for (joint, weight) in joints.iter().zip(weights) {
        if weight == 0.0 {
            continue;
        }
        let Some(pose) = poses.get(usize::from(*joint)) else {
            continue;
        };
        let (moved, turned) = (
            transform_point(pose, position),
            transform_vector(pose, normal),
        );
        for axis in 0..3 {
            p[axis] += weight * moved[axis];
            n[axis] += weight * turned[axis];
        }
    }
    let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if length > 1e-12 {
        n = [n[0] / length, n[1] / length, n[2] / length];
    }
    (p, n)
}

/// [`deform`] or [`deform_dual`], by how the skin blends.
#[must_use]
pub fn deform_with(
    blend: Blend,
    position: [f32; 3],
    normal: [f32; 3],
    joints: [u16; 4],
    weights: [f32; 4],
    poses: &[[f32; 16]],
) -> ([f32; 3], [f32; 3]) {
    match blend {
        Blend::Linear => deform(position, normal, joints, weights, poses),
        Blend::DualQuaternion => deform_dual(position, normal, joints, weights, poses),
    }
}

/// Resolves every skin against the bone parts and computes the weights. A skin that cannot work is dropped with a
/// warning, so the viewer and the exporter only ever see usable ones.
pub(crate) fn weigh(parts: &mut [Part]) -> Vec<EvaluationWarning> {
    let mut warnings = Vec::new();
    let bones: HashMap<String, Segment> = parts
        .iter()
        .filter(|part| part.transforms.is_none() && part.skin.is_none())
        .filter_map(|part| part.bone.map(|segment| (part.name.clone(), segment)))
        .collect();
    let (mut skinned, mut vertices) = (0, 0);
    for part in parts.iter_mut() {
        let Some(skinning) = part.skin.take() else {
            continue;
        };
        let rigid = !part.animation.is_empty()
            || part.parent.is_some()
            || part.bone.is_some()
            || part.behavior.is_some();
        if part.transforms.is_some() || rigid {
            warnings.push(warning(
                part,
                "rig.skin_not_rigid",
                format!("{} is skinned, so it cannot also be an instance set, a bone, a parent, a child or carry its own tracks or behaviour; the skin is dropped", part.name),
            ));
            continue;
        }
        let mut valid: Vec<(String, Segment)> = Vec::new();
        for name in &skinning.bones {
            match bones.get(name) {
                Some(segment) => valid.push((name.clone(), *segment)),
                None => warnings.push(warning(
                    part,
                    "rig.bone_missing",
                    format!("{} is skinned to \"{name}\", but no output part with a rig.bone has that name; that bone is ignored", part.name),
                )),
            }
        }
        if valid.is_empty() {
            warnings.push(warning(
                part,
                "rig.skin_empty",
                format!("{} has no usable bones, so it is not skinned", part.name),
            ));
            continue;
        }
        skinned += 1;
        vertices += part.positions.len();
        if skinned > MAX_SKINNED_PARTS || vertices > MAX_SKINNED_VERTICES {
            warnings.push(warning(
                part,
                "rig.skin_budget",
                format!("a scene may skin at most {MAX_SKINNED_PARTS} parts and {MAX_SKINNED_VERTICES} vertices; {} is not skinned", part.name),
            ));
            skinned -= 1;
            vertices -= part.positions.len();
            continue;
        }
        let segments: Vec<Segment> = valid.iter().map(|(_, segment)| *segment).collect();
        let mut stamps: Vec<(usize, &Stamp)> = Vec::new();
        for stamp in &skinning.paint {
            match valid.iter().position(|(name, _)| *name == stamp.bone) {
                Some(index) => stamps.push((index, stamp)),
                None => warnings.push(warning(
                    part,
                    "rig.paint_bone_missing",
                    format!("{} paints bone \"{}\", which is not one of its usable bones; that stamp is ignored", part.name, stamp.bone),
                )),
            }
        }
        let (mut joints, mut weights) = (
            Vec::with_capacity(part.positions.len()),
            Vec::with_capacity(part.positions.len()),
        );
        for position in &part.positions {
            let (j, w) = weigh_vertex(
                *position,
                &segments,
                skinning.falloff,
                skinning.influences as usize,
                &stamps,
            );
            joints.push(j);
            weights.push(w);
        }
        part.skin = Some(Skinning {
            bones: valid.into_iter().map(|(name, _)| name).collect(),
            falloff: skinning.falloff,
            influences: skinning.influences,
            blend: skinning.blend,
            paint: skinning.paint,
            joints,
            weights,
        });
    }
    warnings
}
