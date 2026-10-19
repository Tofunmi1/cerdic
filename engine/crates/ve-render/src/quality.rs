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
    /// Balanced; the starting point for a standalone headset.
    Medium,
    /// Everything on; the default on desktop.
    #[default]
    High,
}

impl QualityTier {
    /// All tiers, cheapest first.
    pub const ALL: [Self; 3] = [Self::Low, Self::Medium, Self::High];

    /// The tier's lowercase name, as accepted by [`FromStr`].
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Low => "low",
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

/// Settings that trade image quality for speed.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct RenderQuality {
    /// The tier these settings came from (informational; the fields below are what the renderer reads).
    pub tier: QualityTier,
    /// Samples per pixel: 1, 2 or 4.
    pub msaa_samples: u32,
    /// Side of one cascade of the sun's shadow map in texels; zero turns shadows off.
    pub shadow_resolution: u32,
    /// How many cascades the sun's shadow map has (1 to 3): more keep near shadows sharp and still reach far.
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
            },
        }
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
    // The post pass samples resolved depth. Use single-sample depth while AO or bloom is active;
    // this also bounds the headset cost of the extra pass.
    let wanted = Msaa::from_samples(if quality.ssao_enabled || quality.bloom_enabled {
        1
    } else {
        quality.msaa_samples
    });
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
        let [low, medium, high] = QualityTier::ALL.map(RenderQuality::for_tier);
        assert!(
            low.msaa_samples <= medium.msaa_samples && medium.msaa_samples <= high.msaa_samples
        );
        assert!(low.shadow_resolution <= medium.shadow_resolution);
        assert!(medium.shadow_resolution <= high.shadow_resolution);
        assert!(low.max_point_lights <= medium.max_point_lights);
        assert!(medium.max_point_lights <= high.max_point_lights);
        assert!(low.max_spot_lights <= medium.max_spot_lights);
        assert!(medium.max_spot_lights <= high.max_spot_lights);
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
    }

    #[test]
    fn low_has_no_shadows_and_the_others_do() {
        assert!(!RenderQuality::for_tier(QualityTier::Low).shadows_enabled());
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

        app.insert_resource(RenderQuality {
            ssao_enabled: true,
            ..RenderQuality::for_tier(QualityTier::High)
        });
        app.update();
        assert_eq!(app.world().get::<Msaa>(camera), Some(&Msaa::Off));
        app.insert_resource(RenderQuality::for_tier(QualityTier::High));
        app.update();
        assert_eq!(app.world().get::<Msaa>(camera), Some(&Msaa::Sample4));
    }
}
