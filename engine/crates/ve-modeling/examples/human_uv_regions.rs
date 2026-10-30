//! Prints, for each region of the human body (by the bone that moves each vertex most), the UV rectangle its vertices fall in, to see how the pack's UV islands are laid out.
#![expect(
    clippy::print_stdout,
    clippy::unwrap_used,
    reason = "a command line tool prints its report and stops at its own errors"
)]
use std::collections::BTreeMap;

use ve_human::{Human, Pack};

fn main() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/human/human.pack");
    let human = Human::new(Pack::from_zstd(&std::fs::read(path).unwrap()).unwrap());
    let names = human.bone_names();
    let mesh = human.render_mesh();
    let skin = human.skin();
    let region = |bone: &str| -> &'static str {
        let b = bone.to_lowercase();
        if b.starts_with("finger") || b.starts_with("wrist") || b.starts_with("metacarpal") {
            "hand"
        } else if b.starts_with("upperarm")
            || b.starts_with("shoulder")
            || b.starts_with("clavicle")
        {
            "upperarm+shoulder"
        } else if b.starts_with("lowerarm") {
            "lowerarm"
        } else if b.starts_with("foot") || b.starts_with("toe") {
            "foot"
        } else if b.starts_with("lowerleg") {
            "lowerleg"
        } else if b.starts_with("upperleg") || b.starts_with("pelvis") {
            "upperleg+pelvis"
        } else if b.starts_with("spine") || b == "root" {
            "torso"
        } else {
            "head+neck"
        }
    };
    let mut boxes: BTreeMap<&str, ([f32; 2], [f32; 2], usize)> = BTreeMap::new();
    for (vertex, uv) in mesh.uv.iter().enumerate() {
        let source = mesh.source[vertex] as usize;
        let best = skin[source]
            .iter()
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap();
        let group = region(&names[usize::from(best.0)]);
        let entry = boxes
            .entry(group)
            .or_insert(([f32::MAX; 2], [f32::MIN; 2], 0));
        for k in 0..2 {
            entry.0[k] = entry.0[k].min(uv[k]);
            entry.1[k] = entry.1[k].max(uv[k]);
        }
        entry.2 += 1;
    }
    for (group, (lo, hi, count)) in boxes {
        println!(
            "{group:20} u {:.3}..{:.3}  v {:.3}..{:.3}  ({count} vertices)",
            lo[0], hi[0], lo[1], hi[1]
        );
    }
}
