//! Evaluated surface parameters.
use super::{Foliage, MaterialRecipe, Toon};
use crate::finish::Finish;
use serde::{Deserialize, Serialize};

/// A solid's surface, as set by a `solid.material` node. Colour is sRGB, emissive is linear.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Material {
    /// Base colour, each 0 to 1.
    pub color: [f32; 3],
    /// Surface opacity; 0 is transparent and 1 is opaque. Older saved results default to opaque.
    #[serde(default = "opaque_opacity")]
    pub opacity: f32,
    /// 0 dielectric, 1 metal.
    pub metallic: f32,
    /// 0 mirror, 1 matte.
    pub roughness: f32,
    /// Light given off; above 1 glows.
    pub emissive: [f32; 3],
    /// Whether this surface also creates a bounded point light from its area and emission.
    pub emits_light: bool,
    /// Optional shader-side procedural pattern.
    pub recipe: MaterialRecipe,
    /// Bounded object-space frequency multiplier for the recipe.
    pub texture_scale: f32,
    /// Metres one repeat of a texture program covers when it tiles in world space (0 means the texture is laid out once
    /// over the part).
    #[serde(default)]
    pub texture_tile: f32,
    /// Blend three shifted texture samples to hide repetition; off for older results.
    #[serde(default)]
    pub no_repeat: bool,
    /// Grid cells per texture tile, from 0.25 to 8.
    #[serde(default = "unit_scale")]
    pub no_repeat_scale: f32,
    /// Side in texels of the maps the viewer bakes for this surface (256, 512 or 1024); 0 leaves it to the viewer (1024). Smaller
    /// maps leave room in the viewer's texture memory for more surfaces.
    #[serde(default)]
    pub texture_size: u32,
    /// Cel shading from a `solid.toon` node; `None` keeps the surface physically based.
    #[serde(default)]
    pub toon: Option<Toon>,
    /// Optional Material Maker-compatible procedural texture program source.
    pub texture_program: Option<String>,
    /// How the baked texture maps are finished, from a `solid.style` node: `painterly`, `retro` or `ink`; `None` leaves them as
    /// the program made them.
    #[serde(default)]
    pub texture_style: Option<String>,
    /// Cutout and wind from a `solid.foliage` node; `None` is an ordinary solid surface.
    #[serde(default)]
    pub foliage: Option<Foliage>,
    /// Clear coat, stretched highlights and translucency from a `solid.finish` node; `None` is the plain surface.
    #[serde(default)]
    pub finish: Option<Finish>,
}

const fn opaque_opacity() -> f32 {
    1.0
}

const fn unit_scale() -> f32 {
    1.0
}
