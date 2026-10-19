//! The engine's easing curves against Blender's own code (`BLI_easing`, linked into the kernel).
use ve_kernel::{BlenderEase, blender_ease};
use ve_modeling::easing;

#[test]
fn our_easing_curves_match_blenders() {
    let curves: [(&str, BlenderEase, fn(f32) -> f32); 3] = [
        ("back", BlenderEase::Back, easing::back_out),
        ("bounce", BlenderEase::Bounce, easing::bounce_out),
        ("elastic", BlenderEase::Elastic, easing::elastic_out),
    ];
    for (name, theirs, ours) in curves {
        for i in 0..=200_u16 {
            let fraction = f32::from(i) / 200.0;
            let (a, b) = (ours(fraction), blender_ease(theirs, fraction));
            assert!(
                (a - b).abs() < 2e-5,
                "{name} at {fraction}: ours {a}, Blender {b}"
            );
        }
    }
}
