//! Builds the winged horse: a stocky quadruped with a horse muzzle, a deep jaw, withers and a mane
//! line, plus the template's wing bones. Prints graph text.

use ve_modeling::creature::{Blob, Bone, Plan, Role, Sliders, template};

fn bone(
    name: &str,
    parent: &str,
    head: [f64; 3],
    tail: [f64; 3],
    radius: [f64; 2],
    role: Role,
) -> Bone {
    Bone {
        name: name.to_owned(),
        parent: Some(parent.to_owned()),
        head,
        tail,
        via: Vec::new(),
        radius,
        role,
        pair: false,
        phase: 0.0,
        blend: 0.0,
    }
}

fn blob(name: &str, center: [f64; 3], radii: [f64; 3]) -> Blob {
    Blob {
        name: name.to_owned(),
        center,
        radii,
        pair: false,
        blend: 0.0,
    }
}

fn main() {
    let head_scale = 1.7;
    let sliders = Sliders {
        size: 2.1,
        head: head_scale,
        limbs: 1.1,
        torso: 1.35,
        legs: 1.0,
        neck: 1.15,
        tail: 1.15,
        color: Some([0.95, 0.93, 0.89]),
        horns: false,
        wings: true,
    };
    let mut creature = template(Plan::Quadruped, &sliders).expect("template");
    creature.name = "pegasus".to_owned();
    creature.at = [0.0, 1.3, 0.0];
    creature.outline = 1.5;

    // Thicker neck and head than the dog-like template.
    for bone in &mut creature.bones {
        if bone.name == "neck1" || bone.name == "neck2" {
            bone.radius = bone.radius.map(|radius| radius * 1.15);
        }
        if bone.name == "neck2" {
            bone.radius[1] = 0.06 * head_scale;
        }
        if bone.name == "head" {
            bone.radius = bone.radius.map(|radius| radius * 1.05);
        }
    }
    if let Some(skull) = creature.blobs.iter_mut().find(|blob| blob.name == "skull") {
        skull.radii = [0.075 * head_scale, 0.065 * head_scale, 0.13 * head_scale];
    }

    let head_tail = creature
        .bones
        .iter()
        .find(|bone| bone.name == "head")
        .expect("head")
        .tail;
    let hip = creature
        .bones
        .iter()
        .find(|bone| bone.name == "pelvis")
        .expect("pelvis")
        .head;

    // A horse face: the muzzle is a thick wedge from inside the skull, with a broad nose and a jaw.
    creature.bones.push(bone(
        "muzzle",
        "head",
        [0.0, head_tail[1] - 0.02, head_tail[2] + 0.10],
        [0.0, head_tail[1] - 0.12, head_tail[2] + 0.36],
        [0.115, 0.080],
        Role::Head,
    ));
    creature.blobs.push(blob(
        "nose",
        [0.0, head_tail[1] - 0.135, head_tail[2] + 0.42],
        [0.078, 0.062, 0.07],
    ));
    creature.blobs.push(blob(
        "jaw",
        [0.0, head_tail[1] - 0.17, head_tail[2] + 0.10],
        [0.095, 0.085, 0.13],
    ));

    // Withers: the ridge where the neck meets the back, so the body reads as a horse rather than a dog.
    creature.blobs.push(blob(
        "withers",
        [0.0, hip[1] + 0.11, 0.32],
        [0.28, 0.19, 0.27],
    ));

    print!("{}", creature.to_text().expect("text"));
}
