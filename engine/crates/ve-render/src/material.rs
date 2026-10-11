//! Surface materials: a small physically based parameter set.

use bevy_color::{Color, LinearRgba};
use bevy_ecs::component::Component;

/// Metallic-roughness surface parameters of a mesh instance.
///
/// Parameters are plain numbers (no textures yet): the material language in a later track will
/// generate them procedurally. They travel with each instance, so batching only depends on the mesh.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct VeMaterial {
    /// Albedo for dielectrics, specular colour for metals.
    pub base_color: Color,
    /// 0 is a dielectric (plastic, wood), 1 a metal.
    pub metallic: f32,
    /// 0 is a mirror, 1 is fully rough. Values below about 0.05 are clamped when shading.
    pub roughness: f32,
    /// Light emitted by the surface, in linear colour.
    pub emissive: Color,
}

impl Default for VeMaterial {
    fn default() -> Self {
        Self {
            base_color: Color::srgb(0.8, 0.8, 0.8),
            metallic: 0.0,
            roughness: 0.5,
            emissive: Color::BLACK,
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
}

impl VeMaterial {
    /// Linear-space, clamped values ready for the GPU.
    pub(crate) fn pack(&self) -> PackedMaterial {
        let base = LinearRgba::from(self.base_color);
        let emissive = LinearRgba::from(self.emissive);
        PackedMaterial {
            base_color: [base.red, base.green, base.blue, base.alpha.clamp(0.0, 1.0)],
            params: [
                self.metallic.clamp(0.0, 1.0),
                self.roughness.clamp(0.0, 1.0),
                0.0,
                0.0,
            ],
            emissive: [emissive.red, emissive.green, emissive.blue, 0.0],
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
            metallic: 3.0,
            roughness: -1.0,
            emissive: Color::BLACK,
        };
        let packed = material.pack();
        assert!((packed.base_color[0] - 1.0).abs() < 1e-6);
        assert!(packed.base_color[1].abs() < 1e-6);
        assert_eq!(packed.params[0].to_bits(), 1.0_f32.to_bits());
        assert_eq!(packed.params[1].to_bits(), 0.0_f32.to_bits());
    }

    #[test]
    fn srgb_mid_grey_becomes_darker_linear() {
        let packed = VeMaterial::default().pack();
        // sRGB 0.8 is about 0.604 in linear space.
        assert!((packed.base_color[0] - 0.604).abs() < 0.01);
    }
}
