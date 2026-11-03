//! Surface materials: a small physically based parameter set.

use crate::surface_terrain::SurfaceTerrain;
use crate::surface_water::SurfaceWater;
use bevy_color::{Color, LinearRgba};
use bevy_ecs::{component::Component, reflect::ReflectComponent};
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

/// Procedural shader pattern selected by the typed `solid.material` recipe.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    bevy_reflect::Reflect,
    serde::Serialize,
    serde::Deserialize,
)]
#[reflect(Serialize, Deserialize)]
pub enum ProceduralRecipe {
    /// Uniform PBR material.
    #[default]
    None,
    /// Wood grain.
    Wood,
    /// Wood boards with seams.
    Planks,
    /// Directional brushed metal.
    BrushedMetal,
    /// Leather pebble.
    Leather,
    /// Woven fabric.
    Fabric,
    /// Brick and mortar.
    Brick,
    /// Concrete aggregate.
    Concrete,
    /// Marble veins.
    Marble,
    /// Painted wall grain.
    PaintedWall,
    /// Water surface with a procedural viewer ripple.
    Water,
}

impl ProceduralRecipe {
    fn code(self) -> f32 {
        match self {
            Self::None => 0.0,
            Self::Wood => 1.0,
            Self::Planks => 2.0,
            Self::BrushedMetal => 3.0,
            Self::Leather => 4.0,
            Self::Fabric => 5.0,
            Self::Brick => 6.0,
            Self::Concrete => 7.0,
            Self::Marble => 8.0,
            Self::PaintedWall => 9.0,
            Self::Water => 10.0,
        }
    }
}

/// Cel ("toon") shading of a surface: light falls into a few flat bands with a tinted shadow colour, an optional rim and a
/// small hard highlight, instead of the smooth physically based response. `bands` of 0 leaves the surface physically based.
#[derive(
    Clone, Copy, Debug, PartialEq, bevy_reflect::Reflect, serde::Serialize, serde::Deserialize,
)]
#[reflect(Serialize, Deserialize)]
pub struct ToonStyle {
    /// Steps between the shadow colour and the lit colour: 1 is two-tone, 2 adds a mid tone, up to 4. 0 turns toon off.
    pub bands: u32,
    /// Softness of each band edge, 0 (hard) to 1 (very soft).
    pub feather: f32,
    /// Moves every band edge toward the light (positive) or away from it (negative), from -1 to 1.
    pub shift: f32,
    /// Size of the hard highlight, 0 (none) to 1 (large).
    pub spec: f32,
    /// What the surface colour is multiplied by in shadow (sRGB).
    pub shade_color: Color,
    /// Strength of the rim light on the lit side, 0 to 1.
    pub rim: f32,
    /// Thickness of the outline in screen pixels at any distance; 0 draws none.
    pub outline_width: f32,
    /// Colour of the outline (sRGB).
    pub outline_color: Color,
}

impl Default for ToonStyle {
    fn default() -> Self {
        Self {
            bands: 0,
            feather: 0.08,
            shift: 0.0,
            spec: 0.0,
            shade_color: Color::srgb(0.62, 0.58, 0.82),
            rim: 0.0,
            outline_width: 0.0,
            outline_color: Color::srgb(0.1, 0.06, 0.14),
        }
    }
}

pub use crate::foliage::FoliageStyle;

/// Extra layers on top of the base surface: the forward renderer supports water, while Bevy also draws a clear coat (car
/// paint, wet skin, glazed ceramic), stretched highlights (brushed metal, hair, silk) and light scattered through a thin body (leaves,
/// wax, ears). All zero is the ordinary surface.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    bevy_reflect::Reflect,
    serde::Serialize,
    serde::Deserialize,
)]
#[reflect(Serialize, Deserialize)]
pub struct SurfaceFinish {
    /// Strength of a glossy clear layer over the surface, 0 to 1.
    pub clearcoat: f32,
    /// Roughness of that layer, 0 (mirror) to 1.
    pub clearcoat_roughness: f32,
    /// How much highlights stretch along the surface, 0 to 1.
    pub anisotropy: f32,
    /// Share of diffuse light that passes through to the other side, 0 to 1.
    pub translucency: f32,
    /// Steepness of procedural relief drawn in the pixel shader (wrinkles, scales, pores finer than the mesh), 0 to 1; 0 is none.
    pub relief: f32,
    /// Features per metre of that relief.
    pub relief_scale: f32,
    /// 0 smooth lumps, 1 ridges and creases, 2 round scales.
    pub relief_style: f32,
    /// How wet the surface is, 0 to 1: darker and glossy, most of all in the low parts of the relief.
    pub wetness: f32,
    /// Colour that shifts with the viewing angle (beetle shell, soap film, fish scale), 0 to 1.
    pub iridescence: f32,
    /// Soft brightening at grazing angles (velvet, peach fuzz, fine hair), 0 to 1.
    pub sheen: f32,
    /// Light that scatters under the skin and softens the shadow edge with a warm tint (skin, wax, ears), 0 to 1.
    pub subsurface: f32,
    /// Sunlight passing through a thin body, glowing on the side that faces the viewer when the sun is behind it, 0 to 1 (wing membranes, leaves, wax).
    pub thickness: f32,
    /// A terrain surface (grass, sand, rock and snow blended by slope and height in the pixel shader); `None` for every other surface.
    pub terrain: Option<SurfaceTerrain>,
    /// A water surface (flowing, depth-tinted, foamy, drawn in the pixel shader from the flow, depth and foam in the vertex colour); `None` for every other surface.
    pub water: Option<SurfaceWater>,
    /// Drawn as a particle (Bevy renderer only): 0 is an ordinary surface; 1 is unlit, takes its colour and opacity from the mesh's vertex
    /// colours and blends like ordinary transparency (smoke, dust); 2 does the same but adds its light to what is behind it (fire, sparks).
    #[serde(default)]
    pub particle: u8,
}

/// Metallic-roughness surface parameters of a mesh instance.
///
/// Parameters are plain numbers (no textures yet): the material language in a later track will
/// generate them procedurally. They travel with each instance; water also shares a per-material wave uniform.
#[derive(
    Component,
    Clone,
    Copy,
    Debug,
    PartialEq,
    bevy_reflect::Reflect,
    serde::Serialize,
    serde::Deserialize,
)]
#[reflect(Component, Serialize, Deserialize)]
pub struct VeMaterial {
    /// Albedo for dielectrics, specular colour for metals.
    pub base_color: Color,
    /// Surface opacity from zero (clear) to one (opaque).
    pub opacity: f32,
    /// Viewer-only time offset for a procedural water surface.
    pub water_time: f32,
    /// Stable authored-node tie breaker for transparent draw ordering.
    pub sort_key: u64,
    /// 0 is a dielectric (plastic, wood), 1 a metal.
    pub metallic: f32,
    /// 0 is a mirror, 1 is fully rough. Values below about 0.05 are clamped when shading.
    pub roughness: f32,
    /// Light emitted by the surface, in linear colour.
    pub emissive: Color,
    /// Bounded shader-side surface pattern.
    pub recipe: ProceduralRecipe,
    /// Object-space frequency multiplier, clamped when packed.
    pub texture_scale: f32,
    /// Blend three shifted samples to hide texture repetition (ignored at Low quality).
    #[serde(default)]
    pub no_repeat: bool,
    /// Grid cells per texture tile, clamped to 0.25..=8 when packed.
    #[serde(default = "no_repeat_scale_default")]
    pub no_repeat_scale: f32,
    /// Layers of the shared [`MaterialTextures`](crate::MaterialTextures) array holding this surface's
    /// baked albedo, tangent-space normal, packed metallic-roughness and emissive maps; -1 for none.
    pub map_layers: [i32; 4],
    /// Cel shading, or none when its `bands` is 0.
    pub toon: ToonStyle,
    /// Cutout and wind, or none when all of it is 0.
    pub foliage: FoliageStyle,
    /// Clear coat, stretched highlights and translucency (Bevy renderer only).
    pub finish: SurfaceFinish,
}

impl Default for VeMaterial {
    fn default() -> Self {
        Self {
            base_color: Color::srgb(0.8, 0.8, 0.8),
            opacity: 1.0,
            water_time: 0.0,
            sort_key: 0,
            metallic: 0.0,
            roughness: 0.5,
            emissive: Color::BLACK,
            recipe: ProceduralRecipe::None,
            texture_scale: 1.0,
            no_repeat: false,
            no_repeat_scale: 1.0,
            map_layers: [-1; 4],
            toon: ToonStyle::default(),
            foliage: FoliageStyle::default(),
            finish: SurfaceFinish::default(),
        }
    }
}

/// Material instance attributes and the water/terrain looks used to select the shared surface uniform.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PackedMaterial {
    pub(crate) grass: Option<FoliageStyle>,
    pub(crate) water: Option<SurfaceWater>,
    pub(crate) terrain: Option<SurfaceTerrain>,
    pub(crate) base_color: [f32; 4],
    /// x: metallic, y: roughness (both clamped to 0..=1).
    pub(crate) params: [f32; 4],
    pub(crate) emissive: [f32; 4],
    /// Texture-array layers of the albedo, normal, metallic-roughness and emissive maps (negative: none).
    pub(crate) maps: [f32; 4],
    /// x: toon bands (0 is off), y: feather, z: shift, w: highlight size.
    pub(crate) toon_a: [f32; 4],
    /// rgb: shadow colour multiplier (linear), w: rim strength.
    pub(crate) toon_b: [f32; 4],
    /// rgb: outline colour (linear), w: outline width in screen pixels (0 is none).
    pub(crate) toon_c: [f32; 4],
    /// x: alpha cutoff (0 keeps every pixel), y: sway, z: flutter, w: stochastic scale (0 is off).
    pub(crate) foliage: [f32; 4],
}

impl VeMaterial {
    /// Water recipes and water finishes share the same forward shading path.
    pub(crate) fn water_surface(&self) -> Option<SurfaceWater> {
        self.finish.water.or_else(|| {
            (self.recipe == ProceduralRecipe::Water).then(|| {
                let base = self.base_color.to_srgba();
                SurfaceWater {
                    shallow: [base.red, base.green, base.blue],
                    flow: self.texture_scale,
                    ..SurfaceWater::default()
                }
            })
        })
    }

    /// Adds a stable instance index to the packed variation lane.
    pub(crate) fn pack_for_instance(&self, instance_index: u32) -> PackedMaterial {
        let base = LinearRgba::from(self.base_color);
        let emissive = LinearRgba::from(self.emissive);
        #[expect(
            clippy::cast_precision_loss,
            reason = "instance indices are bounded to 16,384 before reaching extraction"
        )]
        let seed = instance_index as f32;
        PackedMaterial {
            grass: self.foliage.grass.then_some(self.foliage),
            water: self.water_surface(),
            terrain: self.finish.terrain,
            base_color: [
                base.red,
                base.green,
                base.blue,
                (base.alpha * self.opacity).clamp(0.0, 1.0),
            ],
            params: [
                if self.water_surface().is_some() {
                    self.water_time
                } else {
                    self.metallic.clamp(0.0, 1.0)
                },
                self.roughness.clamp(0.0, 1.0),
                if self.water_surface().is_some() {
                    10.0
                } else {
                    self.recipe.code()
                },
                if self.water_surface().is_some() {
                    self.texture_scale.clamp(0.0, 10.0)
                } else {
                    self.texture_scale.clamp(0.01, 100.0)
                },
            ],
            emissive: [emissive.red, emissive.green, emissive.blue, seed],
            maps: self.map_layers.map(|layer| {
                #[expect(clippy::cast_precision_loss, reason = "layer indices are below 256")]
                let value = layer as f32;
                value
            }),
            toon_a: {
                #[expect(clippy::cast_precision_loss, reason = "bands are at most four")]
                let bands = self.toon.bands.min(4) as f32;
                [
                    bands,
                    self.toon.feather.clamp(0.0, 1.0),
                    self.toon.shift.clamp(-1.0, 1.0),
                    self.toon.spec.clamp(0.0, 1.0),
                ]
            },
            toon_b: {
                let shade = LinearRgba::from(self.toon.shade_color);
                [
                    shade.red,
                    shade.green,
                    shade.blue,
                    self.toon.rim.clamp(0.0, 1.0),
                ]
            },
            toon_c: {
                let line = LinearRgba::from(self.toon.outline_color);
                [
                    line.red,
                    line.green,
                    line.blue,
                    self.toon.outline_width.clamp(0.0, 16.0),
                ]
            },
            foliage: [
                self.foliage.alpha_cutoff.clamp(0.0, 1.0),
                self.foliage.sway.clamp(0.0, 1.0),
                self.foliage.flutter.clamp(0.0, 1.0),
                if self.no_repeat && self.no_repeat_scale.is_finite() {
                    self.no_repeat_scale.clamp(0.25, 8.0)
                } else if self.no_repeat {
                    1.0
                } else {
                    0.0
                },
            ],
        }
    }
}

const fn no_repeat_scale_default() -> f32 {
    1.0
}

#[cfg(test)]
#[path = "material_tests.rs"]
mod tests;
