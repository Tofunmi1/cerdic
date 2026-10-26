use super::*;
use crate::material::FoliageStyle;

fn with_foliage(mesh: u128, x: f32, foliage: FoliageStyle) -> ExtractedMesh {
    let mut leaf = instance(mesh, x);
    leaf.material = VeMaterial {
        foliage,
        ..VeMaterial::default()
    }
    .pack_for_instance(0);
    leaf
}

const CUTOUT: FoliageStyle = FoliageStyle {
    alpha_cutoff: 0.5,
    sway: 0.0,
    flutter: 0.0,
    ..FoliageStyle::plain()
};

#[test]
fn an_ordinary_batch_is_not_a_cutout() {
    let (_, batches) = group_into_batches(&[instance(1, 0.0), instance(1, 1.0)]);
    assert!(!batches[0].cutout);
}

#[test]
fn one_cutout_instance_makes_its_whole_batch_a_cutout() {
    // The batch shares one draw, so the shadow and depth passes must test the alpha for all of it; instances with no cutoff
    // drop nothing there.
    let (data, batches) = group_into_batches(&[
        instance(1, 0.0),
        with_foliage(1, 1.0, CUTOUT),
        instance(1, 2.0),
    ]);
    assert_eq!(batches.len(), 1);
    assert!(batches[0].cutout);
    let cutoffs: Vec<f32> = data.iter().map(|d| d.foliage[0]).collect();
    assert_eq!(cutoffs, [0.0, 0.5, 0.0]);
}

#[test]
fn wind_alone_is_not_a_cutout() {
    let swaying = FoliageStyle {
        alpha_cutoff: 0.0,
        sway: 0.8,
        flutter: 0.3,
        ..FoliageStyle::plain()
    };
    let (data, batches) = group_into_batches(&[with_foliage(1, 0.0, swaying)]);
    assert!(!batches[0].cutout);
    assert_eq!(data[0].foliage, [0.0, 0.8, 0.3, 0.0]);
}

#[test]
fn any_foliage_makes_the_batch_two_sided_but_only_a_cutoff_makes_it_a_cutout() {
    let swaying = FoliageStyle {
        alpha_cutoff: 0.0,
        sway: 0.5,
        flutter: 0.0,
        ..FoliageStyle::plain()
    };
    let (_, wind) = group_into_batches(&[instance(1, 0.0), with_foliage(1, 1.0, swaying)]);
    assert!(wind[0].foliage && !wind[0].cutout);
    let (_, cut) = group_into_batches(&[with_foliage(1, 0.0, CUTOUT)]);
    assert!(cut[0].foliage && cut[0].cutout);
    let (_, plain) = group_into_batches(&[instance(1, 0.0)]);
    assert!(!plain[0].foliage && !plain[0].cutout);
}

#[test]
fn another_mesh_keeps_its_own_flag() {
    let (_, batches) = group_into_batches(&[with_foliage(1, 0.0, CUTOUT), instance(2, 1.0)]);
    assert_eq!(batches.len(), 2);
    assert!(batches[0].cutout && !batches[1].cutout);
}

#[test]
fn the_foliage_lane_is_clamped_and_off_by_default() {
    assert_eq!(VeMaterial::default().pack_for_instance(0).foliage, [0.0; 4]);
    let wild = VeMaterial {
        foliage: FoliageStyle {
            alpha_cutoff: 7.0,
            sway: -3.0,
            flutter: 9.0,
            ..FoliageStyle::plain()
        },
        ..VeMaterial::default()
    }
    .pack_for_instance(0);
    assert_eq!(wild.foliage, [1.0, 0.0, 1.0, 0.0]);
}

#[test]
fn the_instance_layout_covers_every_field_of_the_instance_data() {
    // Twelve `vec4`s: the model matrix, then eight material vectors; each is one vertex attribute, and the last is location 15.
    let layout = crate::pipeline::instance_layout();
    assert_eq!(layout.attributes.len(), 12);
    assert_eq!(layout.array_stride, 12 * 16);
    assert_eq!(layout.array_stride, size_of::<InstanceData>() as u64);
    let locations: Vec<u32> = layout
        .attributes
        .iter()
        .map(|a| a.shader_location)
        .collect();
    assert_eq!(locations, [3, 4, 5, 6, 7, 8, 9, 10, 12, 13, 14, 15]);
}
