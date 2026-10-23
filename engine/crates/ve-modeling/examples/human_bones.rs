//! Prints the skeleton of the `body.human` pack for a body: each bone's index, name, parent and rest head, one per line, so a rig or an animation can
//! name the bones it needs. `cargo run -p ve-modeling --example human_bones -- [name=value ...]` where a name is a phenotype slider (age, gender, height ...).
#![expect(
    clippy::print_stdout,
    clippy::unwrap_used,
    reason = "a command line tool prints its report and stops at its own errors"
)]
use std::collections::BTreeMap;

use ve_human::{Human, Pack, Params};

fn main() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/human/human.pack");
    let human = Human::new(Pack::from_zstd(&std::fs::read(path).unwrap()).unwrap());
    let mut phenotype = BTreeMap::new();
    for argument in std::env::args().skip(1) {
        if let Some((name, value)) = argument.split_once('=') {
            phenotype.insert(name.to_owned(), value.parse::<f32>().unwrap());
        }
    }
    let body = human
        .evaluate(&Params {
            phenotype,
            ..Params::default()
        })
        .unwrap();
    let parents = human.bone_parents();
    for (index, name) in human.bone_names().iter().enumerate() {
        let head = body.bone_heads[index];
        println!(
            "{index} {name} parent={} head={:.4},{:.4},{:.4}",
            parents[index], head[0], head[1], head[2]
        );
    }
}
