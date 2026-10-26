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
        no_repeat: false,
        no_repeat_scale: 1.0,
        map_layers: [-1; 4],
        toon: ToonStyle::default(),
        foliage: FoliageStyle::default(),
        finish: SurfaceFinish::default(),
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

#[test]
fn stochastic_scale_packs_only_for_opted_in_materials() {
    let mut material = VeMaterial {
        no_repeat_scale: 20.0,
        ..VeMaterial::default()
    };
    assert_eq!(material.pack_for_instance(0).foliage[3], 0.0);
    material.no_repeat = true;
    assert_eq!(material.pack_for_instance(0).foliage[3], 8.0);
    material.no_repeat_scale = 0.0;
    assert_eq!(material.pack_for_instance(0).foliage[3], 0.25);
    material.no_repeat_scale = f32::NAN;
    assert_eq!(material.pack_for_instance(0).foliage[3], 1.0);
}
