//! The sun's cascade count follows the quality settings (it used to be fixed at four on WebGPU, whatever the tier said).
use ve_bevy::sun_cascades;
use ve_render::{QualityTier, RenderQuality};

fn cascades(tier: QualityTier) -> usize {
    sun_cascades(&RenderQuality::for_tier(tier)).bounds.len()
}

#[test]
fn a_tier_with_one_cascade_gets_one() {
    assert_eq!(cascades(QualityTier::Quest), 1);
    assert_eq!(cascades(QualityTier::Low), 1);
}

#[test]
fn more_cascades_follow_the_tier_where_the_backend_holds_them() {
    // WebGL2's shaders hold one cascade: asking for more makes the sun cast no shadows at all.
    let (medium, high) = if cfg!(feature = "webgpu") {
        (2, 3)
    } else {
        (1, 1)
    };
    assert_eq!(cascades(QualityTier::Medium), medium);
    assert_eq!(cascades(QualityTier::High), high);
}

#[test]
fn the_shadow_range_stays_bounded_and_the_first_cascade_is_near() {
    let config = sun_cascades(&RenderQuality::for_tier(QualityTier::Quest));
    let far = *config.bounds.last().unwrap();
    assert!(far <= 60.0 && far > 5.0, "far bound {far}");
}
