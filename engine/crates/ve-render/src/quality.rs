//! Render quality: a small table of tiers, each fixing the settings that trade image quality for
//! speed. Changing [`RenderQuality`] at runtime takes effect on the next frame; targets are
//! recreated through the texture cache, so stale ones are released rather than leaked.

use std::fmt;
use std::str::FromStr;

use bevy_camera::Camera3d;
use bevy_ecs::prelude::*;
use bevy_render::Extract;
use bevy_render::view::Msaa;

use crate::light::{MAX_POINT_LIGHTS, MAX_SPOT_LIGHTS};

/// A named quality level.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum QualityTier {
    /// Cheapest: no shadows, no multisampling, few lights, a slightly lower render resolution.
    Low,
    /// Settings for a standalone headset.
    Quest,
    /// Balanced; the starting point for a standalone headset.
    Medium,
    /// Everything on; the default on desktop.
    #[default]
    High,
}

impl QualityTier {
    /// All tiers, cheapest first.
    pub const ALL: [Self; 4] = [Self::Low, Self::Quest, Self::Medium, Self::High];

    /// The tier's lowercase name, as accepted by [`FromStr`].
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Quest => "quest",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

impl fmt::Display for QualityTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for QualityTier {
    type Err = ();

    fn from_str(name: &str) -> Result<Self, ()> {
        Self::ALL
            .into_iter()
            .find(|tier| tier.name() == name)
            .ok_or(())
    }
}

/// Which surface effects the renderer draws. Every surface keeps what its author gave it (a `solid.finish` node, a baked map); a switch here only
/// stops the viewer drawing that layer, so a person can see what each one adds, or buy back speed, without touching the document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is an independent switch the page turns on or off; grouping them would only hide that"
)]
pub struct Effects {
    /// Light scattering under the skin (`subsurface`).
    pub subsurface: bool,
    /// The glossy clear layer (`clearcoat`).
    pub clearcoat: bool,
    /// Light through a thin body (`translucency` and `thickness`).
    pub translucency: bool,
    /// Colour that shifts with the view angle (`iridescence`).
    pub iridescence: bool,
    /// Grazing-angle softness (`sheen`).
    pub sheen: bool,
    /// Procedural relief drawn in the pixel shader (`relief`).
    pub relief: bool,
    /// The wet look (`wetness`).
    pub wetness: bool,
    /// Highlights stretched along the surface (`anisotropy`).
    pub anisotropy: bool,
    /// The baked and procedural texture maps (colour, normal, roughness, glow).
    pub maps: bool,
}

impl Effects {
    /// Everything drawn: the default.
    pub const ALL: Self = Self {
        subsurface: true,
        clearcoat: true,
        translucency: true,
        iridescence: true,
        sheen: true,
        relief: true,
        wetness: true,
        anisotropy: true,
        maps: true,
    };

    /// The names the page uses for each switch.
    pub const NAMES: [&'static str; 9] = [
        "subsurface", "clearcoat", "translucency", "iridescence", "sheen", "relief", "wetness", "anisotropy", "maps",
    ];

    /// Turns the effect called `name` on or off; false when there is no such effect.
    pub fn set(&mut self, name: &str, on: bool) -> bool {
        let slot = match name {
            "subsurface" => &mut self.subsurface,
            "clearcoat" => &mut self.clearcoat,
            "translucency" => &mut self.translucency,
            "iridescence" => &mut self.iridescence,
            "sheen" => &mut self.sheen,
            "relief" => &mut self.relief,
            "wetness" => &mut self.wetness,
            "anisotropy" => &mut self.anisotropy,
            "maps" => &mut self.maps,
            _ => return false,
        };
        *slot = on;
        true
    }

    /// Whether the effect called `name` is drawn, or `None` for a name that is not an effect.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<bool> {
        Some(match name {
            "subsurface" => self.subsurface,
            "clearcoat" => self.clearcoat,
            "translucency" => self.translucency,
            "iridescence" => self.iridescence,
            "sheen" => self.sheen,
            "relief" => self.relief,
            "wetness" => self.wetness,
            "anisotropy" => self.anisotropy,
            "maps" => self.maps,
            _ => return None,
        })
    }

    /// The finish a surface is drawn with: its own, with every switched-off effect set to none.
    #[must_use]
    pub fn filter(&self, finish: &crate::SurfaceFinish) -> crate::SurfaceFinish {
        let mut out = *finish;
        if !self.subsurface {
            out.subsurface = 0.0;
        }
        if !self.clearcoat {
            out.clearcoat = 0.0;
        }
        if !self.translucency {
            out.translucency = 0.0;
            out.thickness = 0.0;
        }
        if !self.iridescence {
            out.iridescence = 0.0;
        }
        if !self.sheen {
            out.sheen = 0.0;
        }
        if !self.relief {
            out.relief = 0.0;
        }
        if !self.wetness {
            out.wetness = 0.0;
        }
        if !self.anisotropy {
            out.anisotropy = 0.0;
        }
        out
    }
}

impl Default for Effects {
    fn default() -> Self {
        Self::ALL
    }
}

/// Settings that trade image quality for speed.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is an independent switch the page turns on or off; grouping them would only hide that"
)]
pub struct RenderQuality {
    /// The tier these settings came from; Low also disables stochastic texture sampling.
    pub tier: QualityTier,
    /// Samples per pixel: 1, 2 or 4.
    pub msaa_samples: u32,
    /// Side of one cascade of the sun's shadow map in texels; zero turns shadows off.
    pub shadow_resolution: u32,
    /// How many cascades the sun's shadow map has: more keep near shadows sharp and still reach far.
    pub shadow_cascades: u32,
    /// Shadow filter radius in taps: 0 is one hardware-filtered tap, 1 is a 3 by 3 grid.
    pub shadow_pcf_radius: u32,
    /// Point lights shaded per frame (the closest to the viewer are kept).
    pub max_point_lights: usize,
    /// Spot lights shaded per frame (nearest first, unshadowed).
    pub max_spot_lights: usize,
    /// Scale of the headset's render resolution relative to the runtime's recommendation.
    pub resolution_scale: f32,
    /// Broadens specular highlights using the screen-space normal footprint.
    pub specular_aa_enabled: bool,
    /// Enables screen-space ambient occlusion in the post pass.
    pub ssao_enabled: bool,
    /// Enables the bounded bright-region bloom pass.
    pub bloom_enabled: bool,
    /// Adapts the exposure to the brightness of the picture over about a second, like an eye or a camera.
    pub auto_exposure_enabled: bool,
    /// Smooths jagged edges after the picture is drawn (SMAA). Where the backend multisamples instead, this does not apply.
    pub smaa_enabled: bool,
    /// Which surface effects are drawn.
    pub effects: Effects,
}

impl RenderQuality {
    /// The settings of a tier.
    #[must_use]
    pub const fn for_tier(tier: QualityTier) -> Self {
        match tier {
            QualityTier::Low => Self {
                tier,
                msaa_samples: 1,
                shadow_resolution: 0,
                shadow_cascades: 1,
                shadow_pcf_radius: 0,
                max_point_lights: 2,
                max_spot_lights: 2,
                resolution_scale: 0.8,
                specular_aa_enabled: false,
                ssao_enabled: false,
                bloom_enabled: false,
                auto_exposure_enabled: false,
                smaa_enabled: true,
                effects: Effects::ALL,
            },
            QualityTier::Quest => Self {
                tier,
                msaa_samples: 4,
                shadow_resolution: 1024,
                shadow_cascades: 1,
                shadow_pcf_radius: 0,
                max_point_lights: 2,
                max_spot_lights: 2,
                resolution_scale: 1.0,
                specular_aa_enabled: false,
                ssao_enabled: false,
                bloom_enabled: false,
                auto_exposure_enabled: false,
                smaa_enabled: true,
                effects: Effects::ALL,
            },
            QualityTier::Medium => Self {
                tier,
                msaa_samples: 2,
                shadow_resolution: 1024,
                shadow_cascades: 2,
                shadow_pcf_radius: 0,
                max_point_lights: 4,
                max_spot_lights: 4,
                resolution_scale: 1.0,
                specular_aa_enabled: false,
                ssao_enabled: false,
                bloom_enabled: false,
                auto_exposure_enabled: false,
                smaa_enabled: true,
                effects: Effects::ALL,
            },
            QualityTier::High => Self {
                tier,
                msaa_samples: 4,
                shadow_resolution: 2048,
                shadow_cascades: 3,
                shadow_pcf_radius: 1,
                max_point_lights: MAX_POINT_LIGHTS,
                max_spot_lights: MAX_SPOT_LIGHTS,
                resolution_scale: 1.0,
                specular_aa_enabled: false,
                ssao_enabled: false,
                bloom_enabled: false,
                auto_exposure_enabled: false,
                smaa_enabled: true,
                effects: Effects::ALL,
            },
        }
    }

    /// Whether opted-in surfaces may blend three samples rather than one.
    #[must_use]
    pub const fn stochastic_sampling_enabled(&self) -> bool {
        !matches!(self.tier, QualityTier::Low)
    }

    /// Whether the sun casts shadows at these settings.
    #[must_use]
    pub const fn shadows_enabled(&self) -> bool {
        self.shadow_resolution > 0
    }
}

impl Default for RenderQuality {
    fn default() -> Self {
        Self::for_tier(QualityTier::default())
    }
}

/// Copies the main-world [`RenderQuality`] into the render world each frame.
pub(crate) fn extract_quality(mut commands: Commands, quality: Extract<Res<RenderQuality>>) {
    commands.insert_resource(**quality);
}

/// Applies the quality's sample count to every 3D camera when the quality changes or a camera appears.
pub(crate) fn apply_quality_to_cameras(
    quality: Res<RenderQuality>,
    mut cameras: Query<(Ref<Camera3d>, &mut Msaa)>,
) {
    // Occlusion reads its own single-sample depth texture (the `ao_depth` pass), so post effects do not limit the sample count.
    let wanted = Msaa::from_samples(quality.msaa_samples);
    for (camera, mut msaa) in &mut cameras {
        if (quality.is_changed() || camera.is_added()) && *msaa != wanted {
            *msaa = wanted;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiers_only_get_more_expensive() {
        let [low, quest, medium, high] = QualityTier::ALL.map(RenderQuality::for_tier);
        assert!(
            low.msaa_samples <= medium.msaa_samples && medium.msaa_samples <= high.msaa_samples
        );
        assert!(low.msaa_samples <= quest.msaa_samples);
        assert!(low.shadow_resolution <= medium.shadow_resolution);
        assert!(medium.shadow_resolution <= high.shadow_resolution);
        assert!(low.shadow_resolution <= quest.shadow_resolution);
        assert!(low.max_point_lights <= medium.max_point_lights);
        assert!(medium.max_point_lights <= high.max_point_lights);
        assert_eq!(quest.max_point_lights, 2);
        assert!(low.max_spot_lights <= medium.max_spot_lights);
        assert!(medium.max_spot_lights <= high.max_spot_lights);
        assert_eq!(quest.max_spot_lights, 2);
        assert_eq!(
            (
                low.max_point_lights,
                medium.max_point_lights,
                high.max_point_lights
            ),
            (2, 4, 16)
        );
        assert_eq!(
            (
                low.max_spot_lights,
                medium.max_spot_lights,
                high.max_spot_lights
            ),
            (2, 4, 8)
        );
        assert!(low.resolution_scale <= medium.resolution_scale);
        assert!(medium.resolution_scale <= high.resolution_scale);
        assert!(low.resolution_scale <= quest.resolution_scale);
    }

    #[test]
    fn low_has_no_shadows_and_the_others_do() {
        assert!(!RenderQuality::for_tier(QualityTier::Low).shadows_enabled());
        assert!(RenderQuality::for_tier(QualityTier::Quest).shadows_enabled());
        assert!(RenderQuality::for_tier(QualityTier::Medium).shadows_enabled());
        assert!(RenderQuality::for_tier(QualityTier::High).shadows_enabled());
    }

    #[test]
    fn every_tier_stays_within_what_the_renderer_supports() {
        for tier in QualityTier::ALL {
            let quality = RenderQuality::for_tier(tier);
            assert!([1, 2, 4].contains(&quality.msaa_samples), "{tier}");
            assert!(quality.max_point_lights <= MAX_POINT_LIGHTS, "{tier}");
            assert!(quality.max_spot_lights <= MAX_SPOT_LIGHTS, "{tier}");
            assert!(quality.shadow_resolution == 0 || quality.shadow_resolution.is_power_of_two());
            assert!((0.5..=1.5).contains(&quality.resolution_scale), "{tier}");
            assert_eq!(quality.tier, tier);
        }
    }

    #[test]
    fn names_round_trip_and_unknown_names_are_rejected() {
        for tier in QualityTier::ALL {
            assert_eq!(tier.name().parse(), Ok(tier));
        }
        assert!("ultra".parse::<QualityTier>().is_err());
        assert!("".parse::<QualityTier>().is_err());
    }

    #[test]
    fn default_is_the_desktop_tier() {
        let quality = RenderQuality::default();
        assert_eq!(quality.tier, QualityTier::High);
        assert!(!quality.specular_aa_enabled);
        assert!(!quality.ssao_enabled);
        assert!(!quality.bloom_enabled);
    }

    #[test]
    fn applying_quality_sets_each_camera_s_sample_count() {
        let mut app = bevy_app::App::new();
        app.insert_resource(RenderQuality::for_tier(QualityTier::Medium))
            .add_systems(bevy_app::Update, apply_quality_to_cameras);
        let camera = app
            .world_mut()
            .spawn((Camera3d::default(), Msaa::Sample4))
            .id();
        app.update();
        assert_eq!(app.world().get::<Msaa>(camera), Some(&Msaa::Sample2));
        app.insert_resource(RenderQuality::for_tier(QualityTier::Low));
        app.update();
        assert_eq!(app.world().get::<Msaa>(camera), Some(&Msaa::Off));

        // Occlusion and bloom read their own single-sample targets, so they do not take multisampling away.
        app.insert_resource(RenderQuality {
            ssao_enabled: true,
            bloom_enabled: true,
            ..RenderQuality::for_tier(QualityTier::High)
        });
        app.update();
        assert_eq!(app.world().get::<Msaa>(camera), Some(&Msaa::Sample4));
        app.insert_resource(RenderQuality::for_tier(QualityTier::High));
        app.update();
        assert_eq!(app.world().get::<Msaa>(camera), Some(&Msaa::Sample4));
    }

    #[test]
    fn a_switched_off_effect_is_removed_from_the_finish_and_the_others_stay() {
        let finish = crate::SurfaceFinish {
            subsurface: 0.5,
            clearcoat: 0.4,
            translucency: 0.3,
            thickness: 0.2,
            relief: 0.8,
            ..crate::SurfaceFinish::default()
        };
        let mut effects = Effects::ALL;
        assert_eq!(effects.filter(&finish), finish);
        assert!(effects.set("subsurface", false));
        assert!(effects.set("translucency", false));
        let shown = effects.filter(&finish);
        assert_eq!((shown.subsurface, shown.translucency, shown.thickness), (0.0, 0.0, 0.0));
        assert_eq!((shown.clearcoat, shown.relief), (0.4, 0.8));
        assert_eq!(effects.get("subsurface"), Some(false));
        assert_eq!(effects.get("sheen"), Some(true));
        assert!(!effects.set("nonsense", true));
        assert_eq!(effects.get("nonsense"), None);
    }
}
