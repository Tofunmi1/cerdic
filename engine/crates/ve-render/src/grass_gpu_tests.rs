//! Grass integration: material persistence, per-look batching, exact mesh bounds and tier selection.
use super::*;
use crate::{VeMaterial, extract::ExtractedMesh, prepare::group_into_batches};
use bevy_math::{Mat4, Vec3};

fn grass(blend: f32) -> FoliageStyle {
    FoliageStyle {
        grass: true,
        ground_blend: blend,
        translucency: 0.5,
        ..FoliageStyle::default()
    }
}

#[test]
fn grass_looks_split_draws_without_adding_instance_attributes() {
    let mesh = bevy_asset::Handle::<Mesh>::default().id();
    let make = |style| ExtractedMesh {
        mesh,
        world_from_local: Mat4::IDENTITY,
        material: VeMaterial {
            foliage: style,
            ..VeMaterial::default()
        }
        .pack_for_instance(0),
        transparent: false,
        sort_key: 0,
        center: Vec3::ZERO,
    };
    let (_, batches) = group_into_batches(&[
        make(FoliageStyle::default()),
        make(grass(0.6)),
        make(grass(0.6)),
        make(grass(0.2)),
    ]);
    assert_eq!(batches.len(), 3);
    assert!(!batches[0].foliage);
    assert!(batches[1].foliage && !batches[1].cutout);
    assert_eq!(batches[1].instances, 1..3);
    assert_eq!(crate::pipeline::instance_layout().attributes.len(), 12);
    assert_eq!(size_of::<crate::prepare::InstanceData>(), 192);
}

#[test]
fn uniform_keys_include_look_and_exact_shape_range() {
    let a = GrassUniform::new(Some(grass(0.6)), [-0.1, 2.0]);
    assert_eq!(a.shape.to_array(), [25.0, 70.0, -0.1, 2.0]);
    assert_eq!(a.look.to_array(), [1.0, 0.5, 1.0, 0.0]);
    let linear = bevy_color::Color::srgb(0.25, 0.18, 0.10).to_linear();
    assert_eq!(
        a.ground.to_array(),
        [linear.red, linear.green, linear.blue, 0.6]
    );
    assert_ne!(
        a.key(),
        GrassUniform::new(Some(grass(0.2)), [-0.1, 2.0]).key()
    );
    assert_ne!(
        a.key(),
        GrassUniform::new(Some(grass(0.6)), [0.0, 3.0]).key()
    );
    assert_eq!(GrassUniform::new(None, [-0.1, 2.0]).key(), [0; 12]);
}

#[test]
fn low_quality_skips_expensive_grass_lighting() {
    let mut view = crate::prepare::ViewUniform::new(Mat4::IDENTITY, Mat4::IDENTITY);
    for tier in crate::QualityTier::ALL {
        view.set_quality(&crate::RenderQuality::for_tier(tier));
        assert_eq!(
            view.grading.w,
            if tier == crate::QualityTier::Low {
                0.0
            } else {
                1.0
            }
        );
    }
}

#[test]
fn old_foliage_json_loads_with_grass_off_and_unit_exposure() {
    let style: Result<FoliageStyle, _> =
        serde_json::from_str(r#"{"alpha_cutoff":0.4,"sway":0.5,"flutter":0.2}"#);
    let style = style.unwrap_or_default();
    assert_eq!(style.alpha_cutoff, 0.4);
    assert!(!style.grass);
    assert_eq!(style.exposure_boost, 1.0);
    let authored = grass(0.6);
    let restored = serde_json::to_string(&authored)
        .ok()
        .and_then(|s| serde_json::from_str::<FoliageStyle>(&s).ok());
    assert_eq!(restored, Some(authored));
}

#[test]
fn fade_is_bounded_for_nonfinite_and_extreme_values() {
    for d in [
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        -f32::MAX,
        0.0,
        f32::MAX,
    ] {
        for (near, far) in [
            (25.0, 70.0),
            (50.0, 50.0),
            (70.0, 25.0),
            (-f32::MAX, f32::MAX),
            (f32::NAN, 70.0),
        ] {
            let f = crate::grass::fade_scale(d, near, far);
            assert!(f.is_finite() && (0.0..=1.0).contains(&f));
        }
    }
}
