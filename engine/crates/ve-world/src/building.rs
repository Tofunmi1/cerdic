//! One building of a modern district from its lot footprint: a shop floor, a shaft (stepped back in tiers when tall), a flat roof and rooftop clutter.
//!
//! Three meshes so each gets its own material: the shop floor, the upper walls and the roof with its clutter. Wall UVs are facade units (u every 8 m along a wall, v every three floors
//! up), so one tiling facade texture fits every building whatever its size; roof UVs are metres / 8. A pure function of its parameters: the same lot and seed give the same building.
mod detail;
mod facade;
mod mansard;
mod roof_clutter;
mod solids;
mod tiers;
mod wings;
use detail::add as add_detail;
pub use facade::{HeroAsset, HeroAssetParams, build_hero_asset};
pub use mansard::Roof;
use roof_clutter::add as add_roof_clutter;
use tiers::shaft;
pub use wings::{Wing, wings};

/// Metres of wall in one repeat of the facade texture.
pub const BAY: f32 = 8.0;
/// Floors in one vertical repeat of the facade texture.
pub const FLOORS_PER_REPEAT: f32 = 3.0;

/// Triangles with per-corner UVs, the layout the engine's surfaces use.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    /// Vertex positions in metres (x, y up, z).
    pub positions: Vec<[f32; 3]>,
    /// Triangle corners as vertex numbers, three per triangle.
    pub indices: Vec<u32>,
    /// One UV per triangle corner.
    pub uvs: Vec<[f32; 2]>,
}

/// A building as three meshes: the shop floor, the upper walls and the roof with its clutter.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Building {
    /// The ground floor (shopfronts).
    pub shop: Mesh,
    /// The shaft above it, with its tiers.
    pub walls: Mesh,
    /// The roof, the ledges between tiers and everything standing on them.
    pub roof: Mesh,
    /// Iron fire escapes on the street front (only with `detail`).
    pub metal: Mesh,
    /// Shop awnings (only with `detail`).
    pub awning: Mesh,
}

/// What to build.
#[derive(Clone, Debug, PartialEq)]
pub struct BuildingParams {
    /// The lot outline in (x, z) metres, counter-clockwise (positive area), 3 to 8 corners, convex.
    pub footprint: Vec<[f32; 2]>,
    /// Height of the roof in metres, 3 to 300.
    pub height: f32,
    /// The footprint edge that faces the street (the shop front).
    pub front: usize,
    /// Picks the roof clutter and the setbacks.
    pub seed: u64,
    /// Storey height in metres, 2.4 to 6.
    pub floor: f32,
    /// How much rooftop clutter, 0 to 1.
    pub clutter: f32,
    /// How much New York street detail, 0 to 1: awnings, stoops, fire escapes, cornices, water towers, stepped crowns and spires. 0 adds none.
    pub detail: f32,
    /// The roof: flat, or a mansard with dormers (only for buildings from three storeys to 50 m; others stay flat).
    pub roof: Roof,
}

/// A building that cannot be made; the message names the parameter.
#[derive(Debug, thiserror::Error)]
pub enum BuildingError {
    /// The outline is not a counter-clockwise convex polygon of 3 to 8 corners.
    #[error("footprint must be 3 to 8 convex corners wound counter-clockwise (positive area): {0}")]
    Footprint(&'static str),
    /// A number is outside its range.
    #[error("{name} must be {range}, got {value}")]
    Range {
        /// The parameter.
        name: &'static str,
        /// The allowed range, in words.
        range: &'static str,
        /// What was given.
        value: f32,
    },
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        #[expect(clippy::cast_precision_loss, reason = "24 bits of a hash")]
        {
            (z >> 40) as f32 / 16_777_216.0
        }
    }
}

fn area(polygon: &[[f32; 2]]) -> f32 {
    (0..polygon.len())
        .map(|i| {
            let (a, b) = (polygon[i], polygon[(i + 1) % polygon.len()]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum::<f32>()
        * 0.5
}

fn convex(polygon: &[[f32; 2]]) -> bool {
    (0..polygon.len()).all(|i| {
        let (a, b, c) = (
            polygon[i],
            polygon[(i + 1) % polygon.len()],
            polygon[(i + 2) % polygon.len()],
        );
        (b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0]) >= -1e-4
    })
}

/// The polygon moved in by `distance` on every side (mitred corners; right for convex outlines; a negative distance moves it out).
#[must_use]
pub fn inset(polygon: &[[f32; 2]], distance: f32) -> Vec<[f32; 2]> {
    let n = polygon.len();
    let normal = |a: [f32; 2], b: [f32; 2]| {
        let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
        let length = (dx * dx + dz * dz).sqrt().max(1e-9);
        [-dz / length, dx / length]
    };
    (0..n)
        .map(|i| {
            let (prev, here, next) = (polygon[(i + n - 1) % n], polygon[i], polygon[(i + 1) % n]);
            let (na, nb) = (normal(prev, here), normal(here, next));
            let k = distance / (1.0 + na[0] * nb[0] + na[1] * nb[1]).max(0.2);
            [here[0] + (na[0] + nb[0]) * k, here[1] + (na[1] + nb[1]) * k]
        })
        .collect()
}

fn contains(polygon: &[[f32; 2]], p: [f32; 2]) -> bool {
    (0..polygon.len()).all(|i| {
        let (a, b) = (polygon[i], polygon[(i + 1) % polygon.len()]);
        (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]) >= 0.0
    })
}

impl Mesh {
    fn vertex(&mut self, p: [f32; 3]) -> u32 {
        self.positions.push(p);
        u32::try_from(self.positions.len() - 1).unwrap_or(0)
    }

    fn triangle(&mut self, corners: [([f32; 3], [f32; 2]); 3]) {
        for (p, uv) in corners {
            let v = self.vertex(p);
            self.indices.push(v);
            self.uvs.push(uv);
        }
    }

    /// A wall strip from `y0` to `y1` along a counter-clockwise outline, outward facing; u counts bays along the whole outline.
    pub fn walls(&mut self, outline: &[[f32; 2]], y0: f32, y1: f32, floor: f32) {
        let mut along = 0.0_f32;
        let repeat = floor * FLOORS_PER_REPEAT;
        for i in 0..outline.len() {
            let (a, b) = (outline[i], outline[(i + 1) % outline.len()]);
            let length = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
            let (u0, u1) = (along / BAY, (along + length) / BAY);
            let (v0, v1) = (y0 / repeat, y1 / repeat);
            let (b0, b1) = ([a[0], y0, a[1]], [b[0], y0, b[1]]);
            let (t0, t1) = ([a[0], y1, a[1]], [b[0], y1, b[1]]);
            self.triangle([(b0, [u0, v0]), (t1, [u1, v1]), (b1, [u1, v0])]);
            self.triangle([(b0, [u0, v0]), (t0, [u0, v1]), (t1, [u1, v1])]);
            along += length;
        }
    }

    /// A horizontal cap at height `y` over a counter-clockwise outline, facing up, as a fan from the centroid (UVs are metres / 8).
    pub fn cap(&mut self, outline: &[[f32; 2]], y: f32) {
        let n = outline.len() as f32;
        let c = outline
            .iter()
            .fold([0.0, 0.0], |s, p| [s[0] + p[0] / n, s[1] + p[1] / n]);
        let uv = |p: [f32; 2]| [p[0] / BAY, p[1] / BAY];
        for i in 0..outline.len() {
            let (a, b) = (outline[i], outline[(i + 1) % outline.len()]);
            self.triangle([
                ([c[0], y, c[1]], uv(c)),
                ([b[0], y, b[1]], uv(b)),
                ([a[0], y, a[1]], uv(a)),
            ]);
        }
    }

    /// The flat ring between an outer and an inner outline (the ledge where a tier steps back), facing up.
    fn ring(&mut self, outer: &[[f32; 2]], inner: &[[f32; 2]], y: f32) {
        let uv = |p: [f32; 2]| [p[0] / BAY, p[1] / BAY];
        let n = outer.len();
        for i in 0..n {
            let j = (i + 1) % n;
            let p = |q: [f32; 2]| [q[0], y, q[1]];
            self.triangle([
                (p(outer[i]), uv(outer[i])),
                (p(inner[j]), uv(inner[j])),
                (p(outer[j]), uv(outer[j])),
            ]);
            self.triangle([
                (p(outer[i]), uv(outer[i])),
                (p(inner[i]), uv(inner[i])),
                (p(inner[j]), uv(inner[j])),
            ]);
        }
    }

    fn append(&mut self, other: &Mesh) {
        let base = u32::try_from(self.positions.len()).unwrap_or(0);
        self.positions.extend_from_slice(&other.positions);
        self.indices.extend(other.indices.iter().map(|i| i + base));
        self.uvs.extend_from_slice(&other.uvs);
    }
}

fn check(params: &BuildingParams) -> Result<(), BuildingError> {
    let n = params.footprint.len();
    if !(3..=8).contains(&n) {
        return Err(BuildingError::Footprint("not 3 to 8 corners"));
    }
    if params.footprint.iter().flatten().any(|c| !c.is_finite()) {
        return Err(BuildingError::Footprint("a corner is not a finite number"));
    }
    if area(&params.footprint) <= 0.0 {
        return Err(BuildingError::Footprint(
            "it winds clockwise or has no area",
        ));
    }
    if !convex(&params.footprint) {
        return Err(BuildingError::Footprint("it is not convex"));
    }
    if !(3.0..=300.0).contains(&params.height) {
        return Err(BuildingError::Range {
            name: "height",
            range: "3 to 300 metres",
            value: params.height,
        });
    }
    if !(2.4..=6.0).contains(&params.floor) {
        return Err(BuildingError::Range {
            name: "floor",
            range: "2.4 to 6 metres",
            value: params.floor,
        });
    }
    if !(0.0..=1.0).contains(&params.clutter) {
        return Err(BuildingError::Range {
            name: "clutter",
            range: "0 to 1",
            value: params.clutter,
        });
    }
    if !(0.0..=1.0).contains(&params.detail) {
        return Err(BuildingError::Range {
            name: "detail",
            range: "0 to 1",
            value: params.detail,
        });
    }
    Ok(())
}

/// Builds one building.
///
/// # Errors
/// The footprint, height, floor or clutter is out of range; the message names it.
pub fn build(params: &BuildingParams) -> Result<Building, BuildingError> {
    check(params)?;
    let mut rng = Rng(params.seed.wrapping_mul(0x2545_F491_4F6C_DD1D) ^ 0xA076_1D64_78BD_642F);
    let outline = &params.footprint;
    let height = params.height;
    let shop = (params.floor * 1.35).min(height * 0.5);
    let mut building = Building::default();
    building.shop.walls(outline, 0.0, shop, params.floor);
    // a mansard roof takes the top storey: the walls stop one storey lower and the slope runs up to the height
    let mansard = params.roof == Roof::Mansard && mansard::fits(height, params.floor);
    let walled = BuildingParams {
        height: if mansard {
            height - params.floor
        } else {
            height
        },
        ..params.clone()
    };
    let eaves = shaft(&mut building, outline, &walled, shop, &mut rng);
    let mut roof_outline = eaves.clone();
    if mansard {
        roof_outline = inset(&eaves, params.floor * 0.9);
        mansard::add(
            &mut building.walls,
            &mut building.roof,
            &eaves,
            &roof_outline,
            walled.height,
            params.floor,
        );
    }
    building.roof.cap(&roof_outline, height);
    add_roof_clutter(&mut building.roof, &roof_outline, height, params, &mut rng);
    add_detail(&mut building, &walled, shop, &eaves, &mut rng);
    Ok(building)
}
