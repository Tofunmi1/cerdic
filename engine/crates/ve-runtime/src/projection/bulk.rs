//! Placement sets drawn as one retained set (see `ve-render`'s `VeInstanceSet`) instead of one entity per copy.
//!
//! A set of many copies that never move is far cheaper this way: no entity per copy to extract, sort and upload every frame, culling
//! by chunk, and one pick query over the copies. The set is selected and highlighted as a whole; individual copies are not separate
//! objects (a set with per-copy animation tracks, or too few copies to matter, keeps the entity-per-copy form).
use super::{Authored, Commands, Component, Interactable, JobToken, Label, Transform, Uploaded};
use bevy_math::{Mat4, Vec3};
use std::sync::Arc;
use ve_kernel::Bounds;
use ve_render::{VeInstanceSet, VeMaterial};

/// A set with at least this many copies is drawn as one retained set.
pub(super) const BULK_MIN: usize = 256;

/// The copies of a bulk set, for picking one with a ray.
#[derive(Component, Clone)]
pub(crate) struct BulkInstances(pub(crate) ve_interact::BoxSet);

/// Whether a set of `copies` placements with this surface and these shared tracks is drawn as one retained set: many copies, opaque,
/// and none of them animated.
pub(super) fn eligible(copies: usize, surface: &VeMaterial, animated: bool) -> bool {
    copies >= BULK_MIN && !animated && surface.opacity >= 1.0
}

/// Spawns the one entity of a set of placements. The mesh asset stays alive through the component's handle.
pub(super) fn spawn(
    commands: &mut Commands,
    item: Uploaded,
    token: &JobToken,
    surface: VeMaterial,
    matrices: &[Mat4],
    indices: &[u32],
    range: Option<(f32, f32)>,
    fade: f32,
) {
    let local_min = Vec3::from(item.bounds.min.map(|v| v as f32));
    let local_max = Vec3::from(item.bounds.max.map(|v| v as f32));
    let transforms: Arc<[Mat4]> = Arc::from(matrices);
    let boxes = ve_interact::BoxSet::new(Arc::clone(&transforms), local_min, local_max);
    let Some((low, high)) = boxes.world_bounds() else {
        return;
    };
    let world = Bounds {
        min: low.to_array().map(f64::from),
        max: high.to_array().map(f64::from),
    };
    let centre = (local_min + local_max) * 0.5;
    let radius = (local_max - local_min).length() * 0.5;
    let (min_distance, max_distance) = band(range);
    let mut set = VeInstanceSet::new(
        item.handle.clone(),
        transforms,
        Arc::from(indices),
        (centre, radius),
    );
    set.min_distance = min_distance;
    set.fade = fade;
    set.meshes[0].max_distance = max_distance;
    commands.spawn((
        set,
        surface,
        Transform::IDENTITY,
        Interactable::selection_only(&world),
        Label(item.name.clone()),
        Authored {
            node: item.node,
            name: item.name,
            revision: token.revision,
        },
        BulkInstances(boxes),
    ));
}

fn band(range: Option<(f32, f32)>) -> (f32, f32) {
    range.unwrap_or((0.0, f32::INFINITY))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_many_static_opaque_copies_are_bulk() {
        let opaque = VeMaterial::default();
        assert!(eligible(BULK_MIN, &opaque, false));
        assert!(eligible(16_000, &opaque, false));
        assert!(!eligible(BULK_MIN - 1, &opaque, false), "too few to matter");
        assert!(
            !eligible(16_000, &opaque, true),
            "animated copies are separate entities"
        );
        let glass = VeMaterial {
            opacity: 0.5,
            ..VeMaterial::default()
        };
        assert!(
            !eligible(16_000, &glass, false),
            "transparent copies sort per entity"
        );
    }

    #[test]
    fn no_range_means_unbounded_and_a_range_is_kept() {
        assert_eq!(band(None), (0.0, f32::INFINITY));
        assert_eq!(band(Some((45.0, 140.0))), (45.0, 140.0));
    }
}
