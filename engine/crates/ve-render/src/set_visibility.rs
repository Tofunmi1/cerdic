//! Which chunks of which instance sets each view draws, and in which level of detail.

use bevy_asset::AssetId;
use bevy_ecs::prelude::*;
use bevy_math::Vec3;
use bevy_mesh::Mesh;
use bevy_render::view::ExtractedView;

use crate::frustum::Frustum;
use crate::instance_sets::{RetainedSets, SetStats};
use crate::prepare::{Batch, InstanceBatches, view_clip_matrices};
use crate::set_chunks::Chunk;
use crate::shadow::ShadowState;
use crate::stereo::StereoViews;

/// What visibility needs to know about one set.
pub(crate) struct SetShape<'a> {
    pub(crate) chunks: &'a [Chunk],
    /// Levels of detail, nearest first: the mesh and how far it is used.
    pub(crate) levels: &'a [(AssetId<Mesh>, f32)],
    /// Whether any copy has an outline, a cutout or foliage.
    pub(crate) flags: (bool, bool, bool),
}

/// Which level draws something `distance` metres away: the first whose range reaches it, or `None` past the last (not drawn).
pub(crate) fn level_for(levels: &[(AssetId<Mesh>, f32)], distance: f32) -> Option<usize> {
    levels.iter().position(|(_, reach)| distance <= *reach)
}

/// How far a chunk counts as being from the eye for choosing its level: from its centre, brought a little nearer by its extent so the
/// copies at its near edge are not drawn coarser than their distance asks.
fn level_distance(chunk: &Chunk, eye: Vec3) -> f32 {
    (chunk.centre.distance(eye) - 0.5 * chunk.radius).max(0.0)
}

/// Appends to `out` one batch per run of consecutive chunks that are visible from at least one frustum and use the same level, and
/// returns how many chunks and copies were kept.
pub(crate) fn visible_runs(
    shape: &SetShape,
    set: Entity,
    frusta: &[Frustum],
    eye: Vec3,
    out: &mut Vec<Batch>,
) -> (u32, u32) {
    let (mut chunks, mut copies) = (0_u32, 0_u32);
    // The run being built: its level, its first chunk's centre and its range.
    let mut run: Option<(usize, Vec3, std::ops::Range<u32>)> = None;
    let flush = |run: &mut Option<(usize, Vec3, std::ops::Range<u32>)>, out: &mut Vec<Batch>| {
        if let Some((level, centre, range)) = run.take() {
            out.push(Batch {
                mesh: shape.levels[level].0,
                instances: range,
                transparent: false,
                center: centre,
                sort_key: 0,
                outlined: shape.flags.0,
                cutout: shape.flags.1,
                foliage: shape.flags.2,
                set: Some(set),
            });
        }
    };
    for chunk in shape.chunks {
        let seen = frusta
            .iter()
            .any(|frustum| frustum.intersects_sphere(chunk.centre, chunk.radius));
        let level = seen
            .then(|| level_for(shape.levels, level_distance(chunk, eye)))
            .flatten();
        let Some(level) = level else {
            flush(&mut run, out);
            continue;
        };
        chunks += 1;
        copies += chunk.range.end - chunk.range.start;
        match &mut run {
            Some((current, _, range)) if *current == level && range.end == chunk.range.start => {
                range.end = chunk.range.end;
            }
            _ => {
                flush(&mut run, out);
                run = Some((level, chunk.centre, chunk.range.clone()));
            }
        }
    }
    flush(&mut run, out);
    (chunks, copies)
}

/// Decides, for the camera and for each shadow cascade, which chunk runs of which sets are drawn this frame.
pub(crate) fn cull_sets(
    mut batches: ResMut<InstanceBatches>,
    views: Query<(&ExtractedView, Option<&StereoViews>)>,
    shadow: Res<ShadowState>,
) {
    let RetainedSets {
        sets,
        main,
        cascades,
        stats,
        ..
    } = &mut batches.sets;
    main.clear();
    cascades.iter_mut().for_each(Vec::clear);
    *stats = SetStats {
        sets: u32::try_from(sets.len()).unwrap_or(u32::MAX),
        ..SetStats::default()
    };
    // Cascades follow the first view, as the shadow state does.
    let Some((view, stereo)) = views.iter().next() else {
        crate::counters::record_sets(*stats);
        return;
    };
    let frusta: Vec<Frustum> = view_clip_matrices(view, stereo)
        .into_iter()
        .map(Frustum::from_clip_from_world)
        .collect();
    let eye = view.world_from_view.translation();
    let cascade_frusta: Vec<Frustum> = if shadow.params.enabled {
        shadow.params.clip_from_world[..shadow.params.cascades.min(cascades.len())]
            .iter()
            .map(|matrix| Frustum::from_clip_from_world(*matrix).sides_only())
            .collect()
    } else {
        Vec::new()
    };
    for (entity, retained) in sets.iter() {
        stats.copies = stats.copies.saturating_add(retained.copies);
        stats.chunks = stats
            .chunks
            .saturating_add(u32::try_from(retained.chunks.len()).unwrap_or(0));
        let shape = SetShape {
            chunks: &retained.chunks,
            levels: &retained.levels,
            flags: (retained.outlined, retained.cutout, retained.foliage),
        };
        if shape.levels.is_empty() {
            continue;
        }
        let (chunks, copies) = visible_runs(&shape, *entity, &frusta, eye, main);
        stats.chunks_visible = stats.chunks_visible.saturating_add(chunks);
        stats.copies_visible = stats.copies_visible.saturating_add(copies);
        // A cascade's casters are chosen by the sun's box, with the level the camera sees them at.
        for (frustum, out) in cascade_frusta.iter().zip(cascades.iter_mut()) {
            visible_runs(&shape, *entity, std::slice::from_ref(frustum), eye, out);
        }
    }
    crate::counters::record_sets(*stats);
}

#[cfg(test)]
#[path = "set_visibility_tests.rs"]
mod tests;
