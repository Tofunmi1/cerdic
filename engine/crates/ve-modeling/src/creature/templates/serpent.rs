use super::{Creature, Role, Sliders, blob, bone};

/// Bones along a serpent's body.
const SEGMENTS: usize = 9;

pub(super) fn serpent(s: &Sliders) -> Creature {
    let (hd, lm, tl) = (s.head, s.limbs, s.tail);
    let length = 2.0 * tl;
    let point = |k: f64| {
        let along = 1.0 - k;
        [
            0.22 * (k * 9.0).sin() * (0.4 + 0.6 * k),
            0.0,
            length * (along - 0.5),
        ]
    };
    // Girth along the body: a narrow neck, thickest about a third of the way down, then a long taper to the tail tip.
    let radius = |k: f64| {
        let profile = if k < 0.3 {
            0.6 + 0.4 * (k / 0.3)
        } else {
            1.0 - 0.94 * ((k - 0.3) / 0.7).powf(1.3)
        };
        0.12 * lm * profile.max(0.06)
    };
    // The front of the body rears off the ground: no lift past a quarter of the length, a smooth curve up to the head.
    let lift = |k: f64| 0.3 * length * (1.0 - (k / 0.25).min(1.0)).powi(2);
    let mut bones = Vec::new();
    for index in 0..SEGMENTS {
        let count = f64::from(u32::try_from(SEGMENTS).unwrap_or(1));
        let (a, b) = (
            f64::from(u32::try_from(index).unwrap_or(0)) / count,
            f64::from(u32::try_from(index + 1).unwrap_or(0)) / count,
        );
        let name = format!("seg{index}");
        let parent = (index > 0).then(|| format!("seg{}", index - 1));
        let (mut head, mut tail) = (point(a), point(b));
        head[1] = radius(a) + lift(a);
        tail[1] = radius(b) + lift(b);
        bones.push(bone(
            &name,
            parent.as_deref(),
            head,
            tail,
            [radius(a), radius(b)],
            if index == 0 { Role::Head } else { Role::Tail },
        ));
    }
    let front = point(0.0);
    let blobs = vec![blob(
        "skull",
        [front[0], radius(0.0) * 1.1 + lift(0.0), front[2] + 0.06],
        [0.085 * hd, 0.065 * hd, 0.14 * hd],
        false,
    )];
    Creature {
        name: "serpent".to_owned(),
        bones,
        blobs,
        color: s.color.unwrap_or([0.85, 0.55, 0.2]),
        outline: 1.8,
        resolution: 96,
        at: [0.0; 3],
    }
}
