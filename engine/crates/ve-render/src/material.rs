//! Surface materials: a small physically based parameter set.

use bevy_color::{Color, LinearRgba};
use bevy_ecs::component::Component;

/// Procedural shader pattern selected by the typed `solid.material` recipe.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
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
#[derive(Clone, Copy, Debug, PartialEq)]
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

/// Leaf-like behaviour of a surface: cut away where its albedo map is transparent, and moving in the wind. All zero is an
/// ordinary surface.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FoliageStyle {
    /// Pixels whose albedo-map alpha is below this are discarded (0 keeps everything).
    pub alpha_cutoff: f32,
    /// How far the whole shape bends with the wind, 0 to 1 (more the higher a point is above the shape's origin).
    pub sway: f32,
    /// How much the surface trembles at a small scale, 0 to 1.
    pub flutter: f32,
}

/// Metallic-roughness surface parameters of a mesh instance.
///
/// Parameters are plain numbers (no textures yet): the material language in a later track will
/// generate them procedurally. They travel with each instance, so batching only depends on the mesh.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
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
    /// Layers of the shared [`MaterialTextures`](crate::MaterialTextures) array holding this surface's
    /// baked albedo, tangent-space normal, packed metallic-roughness and emissive maps; -1 for none.
    pub map_layers: [i32; 4],
    /// Cel shading, or none when its `bands` is 0.
    pub toon: ToonStyle,
    /// Cutout and wind, or none when all of it is 0.
    pub foliage: FoliageStyle,
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
            map_layers: [-1; 4],
            toon: ToonStyle::default(),
            foliage: FoliageStyle::default(),
        }
    }
}

/// The three `vec4` slots a material occupies in the instance buffer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PackedMaterial {
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
    /// x: alpha cutoff (0 keeps every pixel), y: sway, z: flutter, w unused.
    pub(crate) foliage: [f32; 4],
}

impl VeMaterial {
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
            base_color: [
                base.red,
                base.green,
                base.blue,
                (base.alpha * self.opacity).clamp(0.0, 1.0),
            ],
            params: [
                if self.recipe == ProceduralRecipe::Water {
                    self.water_time
                } else {
                    self.metallic.clamp(0.0, 1.0)
                },
                self.roughness.clamp(0.0, 1.0),
                self.recipe.code(),
                if self.recipe == ProceduralRecipe::Water {
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
                0.0,
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packs_to_linear_and_clamps_parameters() {
        let material = VeMaterial {
            base_color: Color::srgb(1.0, 0.0, 0.0),
            opacity: 1.0,
            sort_key: 0,
            metallic: 3.0,
            roughness: -1.0,
            emissive: Color::BLACK,
            water_time: 0.0,
            recipe: ProceduralRecipe::None,
            texture_scale: 1.0,
            map_layers: [-1; 4],
            toon: ToonStyle::default(),
            foliage: FoliageStyle::default(),
        };
        let packed = material.pack_for_instance(0);
        assert!((packed.base_color[0] - 1.0).abs() < 1e-6);
        assert!(packed.base_color[1].abs() < 1e-6);
        assert_eq!(packed.params[0].to_bits(), 1.0_f32.to_bits());
        assert_eq!(packed.params[1].to_bits(), 0.0_f32.to_bits());
    }

    #[test]
    fn toon_style_packs_into_its_own_lanes_and_is_off_by_default() {
        assert_eq!(
            VeMaterial::default().pack_for_instance(0).toon_a[0].to_bits(),
            0.0_f32.to_bits()
        );
        let packed = VeMaterial {
            toon: ToonStyle {
                bands: 9,
                feather: 3.0,
                shift: -4.0,
                spec: 0.5,
                rim: 0.4,
                ..ToonStyle::default()
            },
            ..VeMaterial::default()
        }
        .pack_for_instance(0);
        assert_eq!(packed.toon_a, [4.0, 1.0, -1.0, 0.5]);
        assert!((packed.toon_b[3] - 0.4).abs() < 1e-6);
        assert!(
            packed.toon_b[2] > packed.toon_b[0],
            "the default shadow colour is cool: {:?}",
            packed.toon_b
        );
        assert_eq!(
            packed.toon_c[3].to_bits(),
            0.0_f32.to_bits(),
            "no outline unless asked for"
        );
        let outlined = VeMaterial {
            toon: ToonStyle {
                outline_width: 99.0,
                ..ToonStyle::default()
            },
            ..VeMaterial::default()
        }
        .pack_for_instance(0);
        assert_eq!(
            outlined.toon_c[3].to_bits(),
            16.0_f32.to_bits(),
            "the width is clamped"
        );
    }

    #[test]
    fn opacity_is_packed_into_the_fragment_alpha_lane() {
        let packed = VeMaterial {
            opacity: 0.35,
            ..VeMaterial::default()
        }
        .pack_for_instance(0);
        assert!((packed.base_color[3] - 0.35).abs() < 1e-6);
    }

    #[test]
    fn srgb_mid_grey_becomes_darker_linear() {
        let packed = VeMaterial::default().pack_for_instance(0);
        // sRGB 0.8 is about 0.604 in linear space.
        assert!((packed.base_color[0] - 0.604).abs() < 0.01);
    }

    #[test]
    fn procedural_recipe_scale_and_instance_seed_use_the_reserved_material_lanes() {
        let material = VeMaterial {
            recipe: ProceduralRecipe::Fabric,
            texture_scale: 2.5,
            ..VeMaterial::default()
        };
        let packed = material.pack_for_instance(37);
        assert_eq!(
            packed.params[2].to_bits(),
            ProceduralRecipe::Fabric.code().to_bits()
        );
        assert_eq!(packed.params[3].to_bits(), 2.5_f32.to_bits());
        assert_eq!(packed.emissive[3].to_bits(), 37.0_f32.to_bits());
    }
}
