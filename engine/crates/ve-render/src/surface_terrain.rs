//! The terrain surface description carried by a material (ADR 0057).

use bevy_math::Vec4;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};
use bevy_render::render_resource::ShaderType;

/// The layers of a terrain surface. Colours are sRGB; heights are world metres; slopes run from 0 (flat) to 1 (vertical).
///
/// The parameters are serialized through reflection so scripts can tune terrain materials.
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
pub struct SurfaceTerrain {
    /// Colours of grass, rock, snow and sand.
    pub colors: [[f32; 3]; 4],
    /// Roughness of grass, rock, snow and sand.
    pub roughness: [f32; 4],
    /// Slope at which rock takes over, and the width of that edge.
    pub rock: [f32; 2],
    /// Height of the snow line and the width of its edge, in metres.
    pub snow: [f32; 2],
    /// Height of the sand line and the width of its edge, in metres.
    pub sand: [f32; 2],
    /// Large-scale variation, 0 to 1.
    pub variation: f32,
    /// Bump strength 0 to 1, and its features per metre.
    pub relief: [f32; 2],
}

/// Terrain parameters sharing the per-material forward uniform.
#[derive(Clone, Copy, Debug, Default, ShaderType)]
pub(crate) struct TerrainUniform {
    pub(crate) layers: [Vec4; 4],
    pub(crate) a: Vec4,
    pub(crate) b: Vec4,
    pub(crate) c: Vec4,
}

impl TerrainUniform {
    pub(crate) fn new(terrain: Option<SurfaceTerrain>) -> Self {
        let Some(t) = terrain else {
            return Self::default();
        };
        Self {
            layers: std::array::from_fn(|i| {
                let [r, g, b] = t.colors[i];
                let c = bevy_color::Color::srgb(r, g, b).to_linear();
                Vec4::new(c.red, c.green, c.blue, t.roughness[i])
            }),
            a: Vec4::new(t.rock[0], t.rock[1], t.snow[0], t.snow[1]),
            b: Vec4::new(t.sand[0], t.sand[1], t.variation, t.relief[0]),
            c: Vec4::new(t.relief[1], 1.0, 0.0, 0.0),
        }
    }

    pub(crate) fn key(self) -> [u32; 28] {
        let values = [
            self.layers[0],
            self.layers[1],
            self.layers[2],
            self.layers[3],
            self.a,
            self.b,
            self.c,
        ];
        std::array::from_fn(|i| values[i / 4][i % 4].to_bits())
    }
}
