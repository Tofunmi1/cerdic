//! Per-colour regions: which part of one solid's surface carries which material.
//!
//! Booleans and rigid moves normally keep the first input's surface for the whole result. When the
//! inputs carry two or more different materials, each input is labelled with kernel origin ids
//! before the operation, the surviving triangles of the result are mapped back to their input's
//! material, and the value keeps them as ordered runs of triangles. Faces a cutter creates take the
//! cutter's material if it has one, otherwise the first input's, so a cavity keeps the body's colour.
use crate::{Fail, Material, cache::Value, fail};
use std::collections::BTreeMap;
use std::rc::Rc;
use ve_kernel::{KernelError, Mesh, Solid};

/// A run of triangles, from `first_triangle` to the next region (or the end), with one material.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Region {
    pub(crate) first_triangle: usize,
    pub(crate) material: Option<Material>,
}

/// The materials of the operands of one operation, by the origin id given to their faces.
#[derive(Default)]
pub(crate) struct Labels {
    materials: BTreeMap<u32, Option<Material>>,
}

/// Whether the inputs carry at least two different materials, so the operation must track faces.
pub(crate) fn needed(inputs: &[Rc<Value>]) -> bool {
    let mut seen: Vec<&Material> = Vec::new();
    for input in inputs {
        let regions = input
            .regions
            .iter()
            .filter_map(|region| region.material.as_ref());
        for material in regions.chain(input.material.as_ref()) {
            if !seen.contains(&material) {
                seen.push(material);
            }
        }
    }
    seen.len() >= 2
}

impl Labels {
    /// A solid from `value` with one fresh origin id per region (or one for the whole value).
    pub(crate) fn solid(&mut self, value: &Value) -> Result<Solid, KernelError> {
        let starts = if value.regions.is_empty() {
            vec![0]
        } else {
            value
                .regions
                .iter()
                .map(|region| region.first_triangle)
                .collect()
        };
        let count = u32::try_from(starts.len())
            .map_err(|_| KernelError::InvalidArgument("too many material regions"))?;
        let first = Solid::reserve_ids(count);
        let ids = (first..first + count).collect::<Vec<_>>();
        for (index, id) in ids.iter().enumerate() {
            let own = value.regions.get(index).and_then(|r| r.material.clone());
            self.materials
                .insert(*id, own.or_else(|| value.material.clone()));
        }
        Solid::from_mesh_runs(&value.mesh, &starts, &ids)
    }

    /// The regions of `result`'s mesh. Faces whose origin has no material take `fallback`; runs with
    /// the same material are merged.
    pub(crate) fn regions(
        &self,
        result: &Solid,
        fallback: Option<&Material>,
    ) -> Result<Vec<Region>, KernelError> {
        let mut regions: Vec<Region> = Vec::new();
        let runs = result.runs()?;
        let total = result.triangle_count();
        for (index, &(first_triangle, id)) in runs.iter().enumerate() {
            // The kernel can report a run with no triangles (copies that coincide); it has no faces.
            let end = runs.get(index + 1).map_or(total, |next| next.0);
            if end <= first_triangle {
                continue;
            }
            let material = self
                .materials
                .get(&id)
                .cloned()
                .flatten()
                .or_else(|| fallback.cloned());
            if regions.last().is_none_or(|last| last.material != material) {
                regions.push(Region {
                    first_triangle,
                    material,
                });
            }
        }
        Ok(regions)
    }
}

/// Stores `regions` on `value`: one region collapses to a plain material.
pub(crate) fn apply(value: &mut Value, mut regions: Vec<Region>) {
    if regions.len() <= 1 {
        value.material = regions.pop().and_then(|region| region.material);
        value.regions.clear();
    } else {
        value.material = regions[0].material.clone();
        value.regions = regions;
    }
}

/// One value per material region, each with only its own triangles and surface; the volumes are each
/// region's share of the closed solid's volume and sum to it. Frames stay with the first piece.
pub(crate) fn split(value: &Value) -> Result<Vec<Value>, Fail> {
    if value.regions.len() < 2 {
        return Ok(vec![value.clone()]);
    }
    let triangles = value.mesh.indices.len() / 3;
    let mut pieces = Vec::with_capacity(value.regions.len());
    for (index, region) in value.regions.iter().enumerate() {
        let end = value
            .regions
            .get(index + 1)
            .map_or(triangles, |next| next.first_triangle);
        if region.first_triangle >= end || end > triangles {
            return Err(fail(
                "kernel.failed",
                format!(
                    "material regions do not fit the mesh: region {index} starts at triangle {} and ends at {end} of {triangles}",
                    region.first_triangle
                ),
            ));
        }
        let (mesh, volume) = sub_mesh(&value.mesh, region.first_triangle, end);
        let mut piece = Value::solid(mesh, volume);
        piece.uvs = value
            .uvs
            .get(region.first_triangle * 3..end * 3)
            .unwrap_or(&[])
            .to_vec();
        piece.material.clone_from(&region.material);
        if index == 0 {
            piece.frames.clone_from(&value.frames);
        }
        pieces.push(piece);
    }
    Ok(pieces)
}

fn sub_mesh(mesh: &Mesh, first: usize, end: usize) -> (Mesh, f64) {
    let mut remap = BTreeMap::new();
    let mut positions = Vec::new();
    let mut indices = Vec::with_capacity((end - first) * 3);
    let mut volume = 0.0;
    for triangle in first..end {
        let mut corners = [[0.0_f64; 3]; 3];
        for (corner, slot) in corners.iter_mut().enumerate() {
            let old = mesh.indices[triangle * 3 + corner];
            let new = *remap.entry(old).or_insert_with(|| {
                positions.push(mesh.positions[old as usize]);
                u32::try_from(positions.len() - 1).unwrap_or(u32::MAX)
            });
            indices.push(new);
            *slot = mesh.positions[old as usize].map(f64::from);
        }
        let [a, b, c] = corners;
        volume += (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
            + a[2] * (b[0] * c[1] - b[1] * c[0]))
            / 6.0;
    }
    (Mesh { positions, indices }, volume)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn surface(red: f32) -> Material {
        Material {
            color: [red, 0.0, 0.0],
            opacity: 1.0,
            metallic: 0.0,
            roughness: 0.5,
            emissive: [0.0; 3],
            emits_light: false,
            recipe: crate::MaterialRecipe::None,
            texture_scale: 1.0,
            texture_tile: 0.0,
            texture_size: 0,
            toon: None,
            texture_program: None,
            texture_style: None,
            foliage: None,
        }
    }

    #[test]
    fn only_inputs_with_two_different_materials_need_tracking() {
        let value = |material| {
            let mut value = Value::solid(Mesh::default(), 0.0);
            value.material = material;
            Rc::new(value)
        };
        assert!(!needed(&[value(None), value(None)]));
        assert!(!needed(&[value(Some(surface(1.0))), value(None)]));
        assert!(!needed(&[
            value(Some(surface(1.0))),
            value(Some(surface(1.0)))
        ]));
        assert!(needed(&[
            value(Some(surface(1.0))),
            value(Some(surface(0.5)))
        ]));
    }

    #[test]
    fn apply_collapses_one_region_and_keeps_several() {
        let region = |first, material| Region {
            first_triangle: first,
            material,
        };
        let mut value = Value::solid(Mesh::default(), 0.0);
        apply(&mut value, vec![region(0, Some(surface(1.0)))]);
        assert!(value.regions.is_empty() && value.material == Some(surface(1.0)));
        apply(
            &mut value,
            vec![region(0, Some(surface(1.0))), region(6, Some(surface(0.5)))],
        );
        assert_eq!(value.regions.len(), 2);
        assert_eq!(value.material, Some(surface(1.0)));
    }
}
