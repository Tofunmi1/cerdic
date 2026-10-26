//! Retain the committed world-space triangle geometry used for camera planning.
use super::{Uploaded, placements::Placements};
use bevy_asset::Assets;
use bevy_math::Vec3;
use bevy_mesh::{Mesh, VertexAttributeValues};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet, HashMap},
};
use ve_world::survey::Triangle;

/// Bound the cost of expanding instanced prototypes; ordinary parts are all kept.
const INSTANCE_TRIANGLES: usize = 200_000;

/// World-space triangles from one committed projection and omitted logical placements.
#[derive(Default)]
pub(super) struct Collector {
    triangles: Vec<Triangle>,
    skipped: BTreeSet<(u32, u32)>,
}

thread_local! { static GEOMETRY: RefCell<Collector> = RefCell::new(Collector::default()); }

impl Collector {
    /// Reserves a bounded expansion budget by whole placement, across all prototype chunks.
    pub(super) fn new(
        items: &[Uploaded],
        placements: &HashMap<u32, Placements>,
        meshes: &Assets<Mesh>,
    ) -> Self {
        let mut costs = BTreeMap::<u32, usize>::new();
        for item in items {
            if let Some(mesh) = meshes.get(&item.handle)
                && let Some(VertexAttributeValues::Float32x3(corners)) =
                    mesh.attribute(Mesh::ATTRIBUTE_POSITION)
            {
                *costs.entry(item.group).or_default() += corners.len() / 3;
            }
        }
        let mut collector = Self::default();
        let mut remaining = INSTANCE_TRIANGLES;
        for (group, triangles) in costs {
            if let Some(set) = placements.get(&group) {
                for &index in &set.indices {
                    if triangles > remaining {
                        collector.skipped.insert((group, index));
                    } else {
                        remaining -= triangles;
                    }
                }
            }
        }
        collector
    }

    /// Adds a triangle-list mesh at its authored positions, applying each retained placement.
    pub(super) fn add(
        &mut self,
        item: &Uploaded,
        placements: Option<&Placements>,
        meshes: &Assets<Mesh>,
    ) {
        let Some(mesh) = meshes.get(&item.handle) else {
            return;
        };
        let Some(VertexAttributeValues::Float32x3(corners)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            return;
        };
        let Some(placements) = placements else {
            self.triangles.extend(corners.chunks_exact(3).map(|t| {
                [
                    t[0].map(f64::from),
                    t[1].map(f64::from),
                    t[2].map(f64::from),
                ]
            }));
            return;
        };
        for (matrix, &index) in placements.matrices.iter().zip(&placements.indices) {
            if self.skipped.contains(&(item.group, index)) {
                continue;
            }
            self.triangles.extend(corners.chunks_exact(3).map(|t| {
                std::array::from_fn(|i| {
                    matrix
                        .transform_point3(Vec3::from_array(t[i]))
                        .to_array()
                        .map(f64::from)
                })
            }));
        }
    }

    /// Replaces the previous scene only when this projection commits, including empty scenes.
    pub(super) fn publish(self) {
        GEOMETRY.with(|cell| *cell.borrow_mut() = self);
    }
}

/// Borrows the committed geometry and the number of placements excluded by the expansion budget.
pub(crate) fn with_geometry<T>(read: impl FnOnce(&[Triangle], usize) -> T) -> T {
    GEOMETRY.with(|cell| {
        let geometry = cell.borrow();
        read(&geometry.triangles, geometry.skipped.len())
    })
}
