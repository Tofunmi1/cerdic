//! A `quest` quality tier: the settings of a standalone headset, which has about 200 draw calls to spend, not thousands.
use ve_render::{QualityTier, RenderQuality};

#[test]
fn the_tier_is_named_and_parsed() {
    assert_eq!("quest".parse::<QualityTier>(), Ok(QualityTier::Quest));
    assert_eq!(QualityTier::Quest.name(), "quest");
    assert_eq!(QualityTier::Quest.to_string(), "quest");
}

#[test]
fn all_lists_four_tiers_cheapest_first_with_quest_between_low_and_medium() {
    assert_eq!(
        QualityTier::ALL,
        [
            QualityTier::Low,
            QualityTier::Quest,
            QualityTier::Medium,
            QualityTier::High
        ]
    );
}

#[test]
fn quest_spends_little_on_shadows_and_post() {
    let quest = RenderQuality::for_tier(QualityTier::Quest);
    assert_eq!(quest.tier, QualityTier::Quest);
    // One sun, one cascade: every extra cascade draws every caster again.
    assert!(quest.shadows_enabled());
    assert_eq!(quest.shadow_cascades, 1);
    assert!(quest.shadow_resolution <= 1024);
    assert_eq!(quest.shadow_pcf_radius, 0);
    assert!(!quest.bloom_enabled && !quest.ssao_enabled && !quest.auto_exposure_enabled);
    assert!(quest.max_point_lights <= 2);
    assert!(quest.max_spot_lights <= 2);
    assert!((quest.resolution_scale - 1.0).abs() < 1e-6);
}

#[test]
fn quest_is_cheaper_than_medium_and_dearer_than_low() {
    let low = RenderQuality::for_tier(QualityTier::Low);
    let quest = RenderQuality::for_tier(QualityTier::Quest);
    let medium = RenderQuality::for_tier(QualityTier::Medium);
    assert!(low.shadow_resolution < quest.shadow_resolution);
    assert!(quest.shadow_cascades <= medium.shadow_cascades);
    assert!(quest.shadow_resolution <= medium.shadow_resolution);
}

#[test]
fn the_other_tiers_are_unchanged() {
    assert_eq!(QualityTier::default(), QualityTier::High);
    let high = RenderQuality::for_tier(QualityTier::High);
    assert_eq!(
        (
            high.msaa_samples,
            high.shadow_resolution,
            high.shadow_cascades
        ),
        (4, 2048, 3)
    );
    let medium = RenderQuality::for_tier(QualityTier::Medium);
    assert_eq!(
        (
            medium.msaa_samples,
            medium.shadow_resolution,
            medium.shadow_cascades
        ),
        (2, 1024, 2)
    );
    let low = RenderQuality::for_tier(QualityTier::Low);
    assert_eq!((low.msaa_samples, low.shadow_resolution), (1, 0));
}
