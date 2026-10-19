//! Building a [`Scene`] from the geometry a viewer holds: triangles given as corner lists, placed by matrices when they are
//! instances, within a triangle budget (a forest of instanced trees would otherwise be millions of triangles to trace against).

use crate::{Material, Scene, Sky, Sun};

/// The most triangles a baked scene may hold. Beyond it instances are thinned evenly (see [`thinning`]).
pub const MAX_TRIANGLES: usize = 500_000;

/// How many placements apart to take so that `demand` triangles fit `budget`: 1 when they already do.
#[must_use]
pub fn thinning(demand: usize, budget: usize) -> usize {
    demand.div_ceil(budget.max(1)).max(1)
}

/// Collects triangles and materials into a [`Scene`].
#[derive(Debug)]
pub struct SceneBuilder {
    positions: Vec<[f32; 3]>,
    triangle_material: Vec<u32>,
    materials: Vec<Material>,
    budget: usize,
}

impl SceneBuilder {
    /// An empty builder that accepts at most `budget` triangles.
    #[must_use]
    pub fn new(budget: usize) -> Self {
        Self {
            positions: Vec::new(),
            triangle_material: Vec::new(),
            materials: Vec::new(),
            budget,
        }
    }

    /// The index of `material`, adding it if no equal one is there yet.
    pub fn material(&mut self, material: Material) -> u32 {
        let index = self
            .materials
            .iter()
            .position(|known| *known == material)
            .unwrap_or_else(|| {
                self.materials.push(material);
                self.materials.len() - 1
            });
        u32::try_from(index).unwrap_or(u32::MAX)
    }

    /// Adds triangles whose corners come three at a time (a trailing partial triangle is ignored), moved by `transform` (a
    /// column-major 4 by 4 matrix) when given, all with `material` (an index from [`SceneBuilder::material`]). Returns false and
    /// adds nothing when they would not fit the budget or `material` is unknown.
    pub fn add(
        &mut self,
        corners: &[[f32; 3]],
        transform: Option<&[f32; 16]>,
        material: u32,
    ) -> bool {
        let triangles = corners.len() / 3;
        if self.triangle_count() + triangles > self.budget
            || material as usize >= self.materials.len()
        {
            return false;
        }
        self.positions
            .extend(corners[..triangles * 3].iter().map(|c| match transform {
                Some(m) => [
                    m[0] * c[0] + m[4] * c[1] + m[8] * c[2] + m[12],
                    m[1] * c[0] + m[5] * c[1] + m[9] * c[2] + m[13],
                    m[2] * c[0] + m[6] * c[1] + m[10] * c[2] + m[14],
                ],
                None => *c,
            }));
        self.triangle_material
            .extend(std::iter::repeat_n(material, triangles));
        true
    }

    /// How many triangles have been added.
    #[must_use]
    pub fn triangle_count(&self) -> usize {
        self.triangle_material.len()
    }

    /// The box around everything added, or `None` while it is empty.
    #[must_use]
    pub fn bounds(&self) -> Option<([f32; 3], [f32; 3])> {
        let first = *self.positions.first()?;
        Some(
            self.positions
                .iter()
                .fold((first, first), |(low, high), p| {
                    (
                        std::array::from_fn(|a| low[a].min(p[a])),
                        std::array::from_fn(|a| high[a].max(p[a])),
                    )
                }),
        )
    }

    /// The finished scene with its sun and sky. Corner `i` is vertex `i`, so triangle `t` uses vertices `3t` to `3t + 2`.
    #[must_use]
    pub fn build(self, sun: Option<Sun>, sky: Sky) -> Scene {
        let corners = u32::try_from(self.positions.len()).unwrap_or(u32::MAX);
        Scene {
            indices: (0..corners).collect(),
            positions: self.positions,
            triangle_material: self.triangle_material,
            materials: self.materials,
            sun,
            sky,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MATERIAL: Material = Material {
        albedo: [0.5; 3],
        emissive: [0.0; 3],
    };
    const SKY: Sky = Sky {
        zenith: [1.0; 3],
        horizon: [1.0; 3],
        ground: [1.0; 3],
    };
    const TRIANGLE: [[f32; 3]; 3] = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];

    #[test]
    fn corners_become_numbered_vertices_and_triangles_keep_their_materials() {
        let mut builder = SceneBuilder::new(100);
        let a = builder.material(MATERIAL);
        let b = builder.material(Material {
            albedo: [1.0, 0.0, 0.0],
            emissive: [0.0; 3],
        });
        assert!(builder.add(&TRIANGLE, None, a));
        assert!(builder.add(&[TRIANGLE, TRIANGLE].concat(), None, b));
        let scene = builder.build(None, SKY);
        assert_eq!(scene.positions.len(), 9);
        assert_eq!(scene.indices, (0..9).collect::<Vec<u32>>());
        assert_eq!(scene.triangle_material, [0, 1, 1]);
        assert_eq!(scene.materials.len(), 2);
    }

    #[test]
    fn an_equal_material_is_not_added_twice() {
        let mut builder = SceneBuilder::new(10);
        assert_eq!(builder.material(MATERIAL), builder.material(MATERIAL));
    }

    #[test]
    fn a_matrix_moves_the_corners() {
        let mut builder = SceneBuilder::new(10);
        let material = builder.material(MATERIAL);
        // Scale by 2 in x, turn nothing, and move by (10, 20, 30) (column-major).
        #[rustfmt::skip]
        let matrix = [
            2.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            10.0, 20.0, 30.0, 1.0,
        ];
        assert!(builder.add(&TRIANGLE, Some(&matrix), material));
        let scene = builder.build(None, SKY);
        assert_eq!(
            scene.positions,
            [[10.0, 20.0, 30.0], [12.0, 20.0, 30.0], [10.0, 21.0, 30.0]]
        );
    }

    #[test]
    fn a_rotation_matrix_turns_them() {
        let mut builder = SceneBuilder::new(10);
        let material = builder.material(MATERIAL);
        // A quarter turn about z: x goes to y, y to minus x (column-major).
        #[rustfmt::skip]
        let matrix = [
            0.0, 1.0, 0.0, 0.0,
            -1.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 1.0,
        ];
        assert!(builder.add(&TRIANGLE, Some(&matrix), material));
        let scene = builder.build(None, SKY);
        assert_eq!(
            scene.positions,
            [[0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [-1.0, 0.0, 0.0]]
        );
    }

    #[test]
    fn the_budget_is_a_hard_limit_and_a_refused_add_changes_nothing() {
        let mut builder = SceneBuilder::new(3);
        let material = builder.material(MATERIAL);
        assert!(builder.add(&[TRIANGLE, TRIANGLE].concat(), None, material));
        assert!(
            !builder.add(&[TRIANGLE, TRIANGLE].concat(), None, material),
            "two more would make four"
        );
        assert_eq!(builder.triangle_count(), 2);
        assert!(builder.add(&TRIANGLE, None, material), "one more fits");
        assert!(!builder.add(&TRIANGLE, None, material), "now it is full");
    }

    #[test]
    fn an_unknown_material_is_refused_and_a_partial_triangle_ignored() {
        let mut builder = SceneBuilder::new(10);
        let material = builder.material(MATERIAL);
        assert!(!builder.add(&TRIANGLE, None, material + 1));
        let with_extra = [TRIANGLE.to_vec(), vec![[9.0; 3], [9.0; 3]]].concat();
        assert!(builder.add(&with_extra, None, material));
        assert_eq!(builder.triangle_count(), 1);
        assert_eq!(builder.build(None, SKY).positions.len(), 3);
    }

    #[test]
    fn the_bounds_cover_everything_and_are_none_when_empty() {
        let mut builder = SceneBuilder::new(10);
        assert_eq!(builder.bounds(), None);
        let material = builder.material(MATERIAL);
        builder.add(&TRIANGLE, None, material);
        builder.add(
            &[[-3.0, 2.0, 5.0], [4.0, -6.0, 0.0], [0.0, 0.0, 9.0]],
            None,
            material,
        );
        assert_eq!(builder.bounds(), Some(([-3.0, -6.0, 0.0], [4.0, 2.0, 9.0])));
    }

    #[test]
    fn thinning_takes_every_nth_placement_so_the_demand_fits() {
        assert_eq!(thinning(100, 500), 1, "it fits: take them all");
        assert_eq!(thinning(500, 500), 1);
        assert_eq!(thinning(501, 500), 2);
        assert_eq!(thinning(2_000_000, MAX_TRIANGLES), 4);
        assert_eq!(thinning(0, 500), 1);
        assert_eq!(thinning(10, 0), 10, "a zero budget is treated as one");
        // Taking every nth of `count` placements keeps at most the budget.
        for (per, count, budget) in [
            (1_000, 700, 100_000),
            (37, 4096, 60_000),
            (5, 16_384, 40_000),
        ] {
            let step = thinning(per * count, budget);
            let kept = count.div_ceil(step);
            assert!(
                per * kept <= budget + per,
                "{per} x {kept} against {budget}"
            );
        }
    }
}
