//! Motion of the scene being staged: keyframed tracks and behaviours, per authored node.
use super::{Candidate, STATE, placements::Motion, staging};

impl Candidate {
    /// Adds how many placements each placed set has to `sets`, by authored node.
    pub(super) fn count_instance_sets(&self, sets: &mut std::collections::BTreeMap<String, usize>) {
        for placements in self.placements.values() {
            *sets.entry(placements.node.clone()).or_default() += placements.indices.len();
        }
    }

    /// The physics body of each staged part, shaped from the part's vertices and the bounds of everything uploaded for it.
    pub(super) fn body_specs(
        &self,
        uploaded: &[super::Uploaded],
    ) -> std::collections::HashMap<String, std::sync::Arc<crate::physics::BodySpec>> {
        let mut bounds: std::collections::HashMap<&str, ([f32; 3], [f32; 3])> =
            std::collections::HashMap::new();
        for item in uploaded.iter().filter(|item| item.group == 0) {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "bounds are metres, well inside f32"
            )]
            let (lo, hi): ([f32; 3], [f32; 3]) = (
                item.bounds.min.map(|v| v as f32),
                item.bounds.max.map(|v| v as f32),
            );
            let entry = bounds.entry(item.node.as_str()).or_insert((lo, hi));
            entry.0 = std::array::from_fn(|a| entry.0[a].min(lo[a]));
            entry.1 = std::array::from_fn(|a| entry.1[a].max(hi[a]));
        }
        self.bodies
            .iter()
            .filter_map(|(node, staged)| {
                let extent = *bounds.get(node.as_str())?;
                let name = uploaded
                    .iter()
                    .find(|item| &item.node == node)
                    .map_or("", |item| item.name.as_str());
                let spec = crate::physics::build_spec(
                    node,
                    name,
                    &staged.body,
                    &staged.points,
                    &staged.triangles,
                    extent,
                );
                Some((node.clone(), std::sync::Arc::new(spec)))
            })
            .collect()
    }

    /// What moves, reacts or deforms the part made by `node`.
    pub(super) fn motion(&self, node: &str, name: &str) -> Motion<'_> {
        Motion {
            tracks: self.animations.get(node),
            behavior: self.behaviors.get(node),
            machine: self.machines.get(node),
            parent: self.parents.get(node),
            skin: self.skins.get(node),
            ik: self.iks.get(node),
            fx: self.fxs.get(node),
            ragdoll: self.ragdolls.get(node),
            cloth: self.cloths.get(node),
            fluid: self.fluids.get(node),
            sim: self.sims.get(node),
            ik_bone: self.ik_bones.contains(name),
            body: None,
        }
    }
}

/// Stages the keyframed tracks (a JSON array of `anim.track` tracks) of the part made by `node`. A malformed
/// or oversized list refuses the call.
pub(crate) fn animation(text: &str, node: &str, json: &str) -> bool {
    use ve_modeling::anim::{MAX_KEYS, MAX_TRACKS};
    let Ok(tracks) = serde_json::from_str::<Vec<ve_modeling::anim::Track>>(json) else {
        return false;
    };
    if ve_graph::NodeId::parse(node).is_err()
        || tracks.is_empty()
        || tracks.len() > MAX_TRACKS
        || tracks.iter().any(|t| {
            t.keys.len() < 2
                || t.keys.len() > MAX_KEYS
                || t.keys.windows(2).any(|k| k[1].time <= k[0].time)
                || t.keys
                    .iter()
                    .any(|k| !k.time.is_finite() || k.value.iter().any(|v| !v.is_finite()))
                || t.pivot.iter().any(|v| !v.is_finite())
        })
    {
        return false;
    }
    STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let Some(candidate) = state.candidate.as_mut() else {
            return false;
        };
        if serde_json::to_string(&candidate.token).ok().as_deref() != Some(text)
            || !staging::current(&candidate.token)
            || candidate.finished
            || candidate.animations.len() >= 4096
        {
            return false;
        }
        candidate.animations.insert(node.to_owned(), tracks);
        true
    })
}

/// Stages the response (a JSON `behavior.on` behaviour) of the part made by `node`. A malformed or
/// out-of-range one refuses the call.
pub(crate) fn behavior(text: &str, node: &str, json: &str) -> bool {
    use ve_modeling::anim::RADIUS_RANGE;
    let Ok(behavior) = serde_json::from_str::<ve_modeling::anim::Behavior>(json) else {
        return false;
    };
    if ve_graph::NodeId::parse(node).is_err()
        || !(RADIUS_RANGE.0..=RADIUS_RANGE.1).contains(&behavior.radius)
    {
        return false;
    }
    STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let Some(candidate) = state.candidate.as_mut() else {
            return false;
        };
        if serde_json::to_string(&candidate.token).ok().as_deref() != Some(text)
            || !staging::current(&candidate.token)
            || candidate.finished
            || candidate.behaviors.len() >= ve_modeling::anim::MAX_REACTIVE_PARTS
        {
            return false;
        }
        candidate.behaviors.insert(node.to_owned(), behavior);
        true
    })
}

/// Stages the animation state machine (the JSON of a `ve_animstate::StateMachine`) of the part made by `node`. A malformed or invalid
/// machine refuses the call.
pub(crate) fn machine(text: &str, node: &str, json: &str) -> bool {
    let Ok(machine) = serde_json::from_str::<ve_animstate::StateMachine>(json) else {
        return false;
    };
    if ve_graph::NodeId::parse(node).is_err() || machine.validate().is_err() {
        return false;
    }
    STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let Some(candidate) = state.candidate.as_mut() else {
            return false;
        };
        if serde_json::to_string(&candidate.token).ok().as_deref() != Some(text)
            || !staging::current(&candidate.token)
            || candidate.finished
            || candidate.machines.len() >= 256
        {
            return false;
        }
        candidate
            .machines
            .insert(node.to_owned(), std::sync::Arc::new(machine));
        true
    })
}

/// Stages the skin of the part made by `node` (`rig.skin`, ADR 0030): `bones` is a JSON array of bone part names,
/// `indices` the vertex each triangle corner comes from, and `joints`/`weights` four per vertex. Inconsistent arrays
/// refuse the call.
pub(crate) fn skin(
    text: &str,
    node: &str,
    bones: &str,
    indices: &[u32],
    joints: &[u32],
    weights: &[f32],
    dual: bool,
) -> bool {
    let Ok(bones) = serde_json::from_str::<Vec<String>>(bones) else {
        return false;
    };
    let data = crate::skinning::SkinData {
        bones,
        blend: if dual {
            ve_modeling::rig::Blend::DualQuaternion
        } else {
            ve_modeling::rig::Blend::Linear
        },
        indices: indices.to_vec(),
        joints: joints.to_vec(),
        weights: weights.to_vec(),
    };
    if ve_graph::NodeId::parse(node).is_err()
        || !data.valid()
        || data.bones.len() > ve_modeling::rig::MAX_BONES
    {
        return false;
    }
    STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let Some(candidate) = state.candidate.as_mut() else {
            return false;
        };
        if serde_json::to_string(&candidate.token).ok().as_deref() != Some(text)
            || !staging::current(&candidate.token)
            || candidate.finished
            || candidate.skins.len() >= ve_modeling::rig::MAX_SKINNED_PARTS
        {
            return false;
        }
        candidate
            .skins
            .insert(node.to_owned(), std::sync::Arc::new(data));
        true
    })
}

/// Stages the chain of bones (a JSON `IkChain`, segments filled in) that the part made by `node` is the target of
/// (`rig.ik`, ADR 0032). A malformed or inconsistent chain refuses the call.
pub(crate) fn ik(text: &str, node: &str, json: &str) -> bool {
    use ve_modeling::ik::{MAX_CHAIN, MAX_ITERATIONS};
    let Ok(chain) = serde_json::from_str::<ve_modeling::rig::IkChain>(json) else {
        return false;
    };
    let finite = chain
        .segments
        .iter()
        .all(|s| s.head.iter().chain(&s.tail).all(|v| v.is_finite()));
    if ve_graph::NodeId::parse(node).is_err()
        || chain.bones.is_empty()
        || chain.bones.len() > MAX_CHAIN
        || chain.segments.len() != chain.bones.len()
        || !(1..=MAX_ITERATIONS).contains(&chain.iterations)
        || !finite
    {
        return false;
    }
    STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let Some(candidate) = state.candidate.as_mut() else {
            return false;
        };
        if serde_json::to_string(&candidate.token).ok().as_deref() != Some(text)
            || !staging::current(&candidate.token)
            || candidate.finished
            || candidate.iks.len() >= ve_modeling::rig::MAX_IK_CHAINS
        {
            return false;
        }
        candidate.ik_bones.extend(chain.bones.iter().cloned());
        candidate
            .iks
            .insert(node.to_owned(), std::sync::Arc::new(chain));
        true
    })
}

/// Stages the particle effect (a JSON `PartFx`) that the part made by `node` emits (`fx.emitter`). A malformed or invalid effect, or
/// one too many in the scene, refuses the call.
pub(crate) fn fx(text: &str, node: &str, json: &str) -> bool {
    /// Most parts that may emit in one scene.
    const MAX_EFFECTS: usize = 64;
    let Ok(effect) = serde_json::from_str::<ve_modeling::fx::PartFx>(json) else {
        return false;
    };
    if ve_graph::NodeId::parse(node).is_err()
        || effect.effect.validate().is_err()
        || effect.names.len() != effect.effect.emitters.len()
        || effect.ground.is_some_and(|g| !g.is_finite())
    {
        return false;
    }
    STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let Some(candidate) = state.candidate.as_mut() else {
            return false;
        };
        if serde_json::to_string(&candidate.token).ok().as_deref() != Some(text)
            || !staging::current(&candidate.token)
            || candidate.finished
            || candidate.fxs.len() >= MAX_EFFECTS
        {
            return false;
        }
        candidate
            .fxs
            .insert(node.to_owned(), std::sync::Arc::new(effect));
        true
    })
}

/// Stages the ragdoll definition (a JSON `RagdollSpec`, resolved against its bones) carried by the part made by `node` (`phys.ragdoll`). A
/// malformed or inconsistent one refuses the call.
pub(crate) fn ragdoll(text: &str, node: &str, json: &str) -> bool {
    use ve_modeling::ragdoll::MAX_RAGDOLL_BONES;
    let Ok(spec) = serde_json::from_str::<ve_modeling::ragdoll::RagdollSpec>(json) else {
        return false;
    };
    let count = spec.bones.len();
    let numbers = [spec.swing, spec.twist, spec.hinge_limits[0], spec.hinge_limits[1], spec.density, spec.damping, spec.radius];
    if ve_graph::NodeId::parse(node).is_err()
        || count == 0
        || count > MAX_RAGDOLL_BONES
        || spec.segments.len() != count
        || spec.parents.len() != count
        || spec.hinges.len() != count
        || spec.parents.iter().enumerate().any(|(i, p)| p.is_some_and(|p| p >= i))
        || !numbers.iter().chain(&spec.hinge_axis).all(|v| v.is_finite())
        || spec.density <= 0.0
        || !spec.segments.iter().all(|s| s.head.iter().chain(&s.tail).all(|v| v.is_finite()))
    {
        return false;
    }
    STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let Some(candidate) = state.candidate.as_mut() else {
            return false;
        };
        if serde_json::to_string(&candidate.token).ok().as_deref() != Some(text)
            || !staging::current(&candidate.token)
            || candidate.finished
            || candidate.ragdolls.len() >= 16
        {
            return false;
        }
        // Every bone must be posable by the animation pass even with no track or parent of its own (the pelvis): it is a bone like an IK bone.
        candidate.ik_bones.extend(spec.bones.iter().cloned());
        candidate.ragdolls.insert(node.to_owned(), std::sync::Arc::new(spec));
        true
    })
}

/// Stages the live liquid (a JSON `FluidSpec`, collider segments filled in) of the part made by `node` (`fluid.sim`). A malformed or impossible
/// liquid refuses the call.
pub(crate) fn fluid(text: &str, node: &str, json: &str) -> bool {
    let Ok(spec) = serde_json::from_str::<ve_modeling::fluid_sim::FluidSpec>(json) else {
        return false;
    };
    if ve_graph::NodeId::parse(node).is_err() {
        return false;
    }
    let bones = spec.collide_bones.clone();
    let Some(shared) = crate::fluid_live::Shared::new(spec) else {
        return false;
    };
    STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let Some(candidate) = state.candidate.as_mut() else {
            return false;
        };
        if serde_json::to_string(&candidate.token).ok().as_deref() != Some(text)
            || !staging::current(&candidate.token)
            || candidate.finished
            || candidate.fluids.len() >= 8
        {
            return false;
        }
        candidate.ik_bones.extend(bones);
        candidate.fluids.insert(node.to_owned(), shared);
        true
    })
}

/// Stages the simulation program (a JSON `SimSpec`) of the part made by `node` (`sim.program`). A program that does not compile or a
/// description with impossible numbers refuses the call.
pub(crate) fn sim(text: &str, node: &str, json: &str, points: &[f32], corners: &[u32]) -> bool {
    let Ok(spec) = serde_json::from_str::<ve_modeling::sim_program::SimSpec>(json) else {
        return false;
    };
    if ve_graph::NodeId::parse(node).is_err() || !points.len().is_multiple_of(3) {
        return false;
    }
    let mesh = (spec.mode == "mesh").then(|| (points.chunks_exact(3).map(|p| [p[0], p[1], p[2]]).collect(), corners.to_vec()));
    let Some(shared) = crate::sim_live::Shared::new(spec, mesh) else {
        return false;
    };
    STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let Some(candidate) = state.candidate.as_mut() else {
            return false;
        };
        if serde_json::to_string(&candidate.token).ok().as_deref() != Some(text)
            || !staging::current(&candidate.token)
            || candidate.finished
            || candidate.sims.len() >= 16
        {
            return false;
        }
        // The bones it reads must be posed by the animation pass even with no track or parent of their own.
        candidate.ik_bones.extend(shared.bone_parts().map(str::to_owned));
        candidate.sims.insert(node.to_owned(), shared);
        true
    })
}

/// Stages the live cloth (a JSON `ClothSpec`, collider segments filled in) of the part made by `node` (`cloth.sim`), with the part's vertices
/// (flat xyz) and the vertex each triangle corner comes from. A malformed or inconsistent cloth refuses the call.
pub(crate) fn cloth(text: &str, node: &str, json: &str, points: &[f32], corners: &[u32]) -> bool {
    let Ok(spec) = serde_json::from_str::<ve_modeling::cloth_sim::ClothSpec>(json) else {
        return false;
    };
    if !points.len().is_multiple_of(3) {
        return false;
    }
    let data = crate::cloth_live::ClothData {
        spec,
        rest: points.chunks_exact(3).map(|p| [p[0], p[1], p[2]]).collect(),
        corners: corners.to_vec(),
    };
    if ve_graph::NodeId::parse(node).is_err() || !data.valid() {
        return false;
    }
    STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let Some(candidate) = state.candidate.as_mut() else {
            return false;
        };
        if serde_json::to_string(&candidate.token).ok().as_deref() != Some(text)
            || !staging::current(&candidate.token)
            || candidate.finished
            || candidate.cloths.len() >= 16
        {
            return false;
        }
        // The pin bone and the collider bones must be posed by the animation pass even with no track or parent of their own.
        candidate.ik_bones.extend(data.spec.collide_bones.iter().cloned());
        if !data.spec.pin_bone.is_empty() {
            candidate.ik_bones.insert(data.spec.pin_bone.clone());
        }
        candidate
            .cloths
            .insert(node.to_owned(), crate::cloth_live::Shared::new(std::sync::Arc::new(data)));
        true
    })
}

/// Stages the physics body (a JSON `PhysBody`) of the part made by `node` (`phys.body`, ADR 0035), with the part's
/// vertices (flat xyz) and triangles (flat indices) for shapes that use them (empty otherwise). A malformed body or
/// inconsistent arrays refuse the call.
pub(crate) fn body(text: &str, node: &str, json: &str, points: &[f32], triangles: &[u32]) -> bool {
    use ve_modeling::physics::MAX_BODIES;
    let Ok(body) = serde_json::from_str::<ve_modeling::physics::PhysBody>(json) else {
        return false;
    };
    let numbers = [
        body.density,
        body.friction,
        body.restitution,
        body.linear_damping,
        body.angular_damping,
    ];
    if ve_graph::NodeId::parse(node).is_err()
        || !numbers.iter().all(|v| v.is_finite())
        || body.density <= 0.0
        || !points.len().is_multiple_of(3)
        || !triangles.len().is_multiple_of(3)
        || points.len() > 3 * ve_physics::MAX_SHAPE_POINTS
        || triangles.len() > 3 * 200_000
        || !points.iter().all(|v| v.is_finite())
        || triangles
            .iter()
            .any(|index| (*index as usize) * 3 >= points.len())
    {
        return false;
    }
    STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let Some(candidate) = state.candidate.as_mut() else {
            return false;
        };
        if serde_json::to_string(&candidate.token).ok().as_deref() != Some(text)
            || !staging::current(&candidate.token)
            || candidate.finished
            || candidate.bodies.len() >= MAX_BODIES
        {
            return false;
        }
        candidate.bodies.insert(
            node.to_owned(),
            super::StagedBody {
                body,
                points: points.to_vec(),
                triangles: triangles.to_vec(),
            },
        );
        true
    })
}

/// Stages the output name of the part that the part made by `node` follows (`anim.parent`).
pub(crate) fn parent(text: &str, node: &str, parent: &str) -> bool {
    if ve_graph::NodeId::parse(node).is_err() || parent.is_empty() || parent.len() > 64 {
        return false;
    }
    STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let Some(candidate) = state.candidate.as_mut() else {
            return false;
        };
        if serde_json::to_string(&candidate.token).ok().as_deref() != Some(text)
            || !staging::current(&candidate.token)
            || candidate.finished
            || candidate.parents.len() >= 4096
        {
            return false;
        }
        candidate.parents.insert(node.to_owned(), parent.to_owned());
        true
    })
}
