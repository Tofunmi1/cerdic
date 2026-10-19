//! The body surface: every bone is a tapered tube, every blob an ellipsoid, all blended into one field and meshed once.
use super::{Creature, num, vec3};
use std::fmt::Write;

/// Writes the field nodes and the meshing node and returns the name of the solid.
pub(super) fn write(creature: &Creature, text: &mut String) -> String {
    let prefix = &creature.name;
    let mirrored = creature.bones.iter().any(|b| b.pair) || creature.blobs.iter().any(|b| b.pair);

    // Bounds of everything, then the grid spacing they imply. Thin parts are widened to what the grid can hold.
    let (mut low, mut high) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    let mut grow = |point: [f64; 3], reach: [f64; 3]| {
        for axis in 0..3 {
            low[axis] = low[axis].min(point[axis] - reach[axis]);
            high[axis] = high[axis].max(point[axis] + reach[axis]);
        }
    };
    for bone in &creature.bones {
        let reach = bone.radius[0].max(bone.radius[1]);
        for point in std::iter::once(bone.head)
            .chain(bone.via.iter().copied())
            .chain(std::iter::once(bone.tail))
        {
            grow(point, [reach; 3]);
        }
    }
    for blob in &creature.blobs {
        grow(blob.center, blob.radii);
    }
    if mirrored {
        let reach = high[0].abs().max(low[0].abs());
        (low[0], high[0]) = (-reach, reach);
    }
    let extent = (0..3).map(|a| high[a] - low[a]).fold(0.0_f64, f64::max);
    let edge = extent / f64::from(creature.resolution);
    let thinnest = edge * 0.75;
    let margin = edge * 3.0 + 0.05;

    let mut names = Vec::new();
    for bone in &creature.bones {
        let mut points = vec![vec3(bone.head)];
        points.extend(bone.via.iter().map(|p| vec3(*p)));
        points.push(vec3(bone.tail));
        let (head, tail) = (bone.radius[0].max(thinnest), bone.radius[1].max(thinnest));
        let name = format!("{prefix}_{}_t", bone.name);
        let _ = writeln!(
            text,
            "{name} = field.tube@1(points=[{}], radius={}, end_radius={})",
            points.join(", "),
            num(head),
            num(tail)
        );
        let blend = if bone.blend > 0.0 {
            bone.blend
        } else {
            (0.7 * head.min(tail).max(head * 0.5)).max(edge * 0.5)
        };
        names.push((name, blend));
    }
    for blob in &creature.blobs {
        let radii = blob.radii.map(|r| r.max(thinnest));
        let shape = format!("{prefix}_{}_s", blob.name);
        let name = format!("{prefix}_{}_k", blob.name);
        let _ = writeln!(text, "{shape} = field.ellipsoid@1(radii={})", vec3(radii));
        let _ = writeln!(
            text,
            "{name} = field.move@1({shape}, by={})",
            vec3(blob.center)
        );
        let smallest = radii[0].min(radii[1]).min(radii[2]);
        let blend = if blob.blend > 0.0 {
            blob.blend
        } else {
            (0.5 * smallest).max(edge * 0.5)
        };
        names.push((name, blend));
    }

    let mut joined = names[0].0.clone();
    for (index, (name, blend)) in names.iter().enumerate().skip(1) {
        let next = format!("{prefix}_join{index}");
        let _ = writeln!(
            text,
            "{next} = field.smooth_union@1({joined}, {name}, radius={})",
            num(*blend)
        );
        joined = next;
    }
    if mirrored {
        let _ = writeln!(
            text,
            "{prefix}_sym = field.mirror@1({joined}, axes=[1, 0, 0])"
        );
        joined = format!("{prefix}_sym");
    }
    let at = creature.at;
    if at.iter().any(|v| v.abs() > 0.0) {
        let _ = writeln!(
            text,
            "{prefix}_placed = field.move@1({joined}, by={})",
            vec3(at)
        );
        joined = format!("{prefix}_placed");
    }
    let _ = writeln!(
        text,
        "{prefix}_body = field.to_solid@1({joined}, min={}, max={}, edge_length={})",
        vec3([0, 1, 2].map(|a| low[a] - margin + at[a])),
        vec3([0, 1, 2].map(|a| high[a] + margin + at[a])),
        num(edge)
    );
    format!("{prefix}_body")
}
