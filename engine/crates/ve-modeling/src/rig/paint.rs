//! Painted skin weights (ADR 0033): brush stamps that pull vertices toward a bone, or push them away, on top of the
//! distance weights. A stamp is a point, a radius and a strength; its effect fades smoothly to zero at the radius.
use serde::{Deserialize, Serialize};

/// Most stamps one skin may carry.
pub const MAX_STAMPS: usize = 64;

/// One brush stamp.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stamp {
    /// Output name of the bone the stamp paints.
    pub bone: String,
    /// Centre of the brush, world coordinates at rest.
    pub at: [f32; 3],
    /// Brush radius in metres.
    pub radius: f32,
    /// 1 hands the bone every vertex under the brush centre, -1 takes the bone away from them; 0 does nothing.
    pub strength: f32,
}

/// Parses `bone x y z radius strength; ...`.
///
/// # Errors
/// A message naming the stamp that is malformed, out of range, or one too many.
pub fn parse(text: &str) -> Result<Vec<Stamp>, String> {
    let mut stamps = Vec::new();
    for item in text
        .split(';')
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        let fields: Vec<&str> = item.split_whitespace().collect();
        let [bone, x, y, z, radius, strength] = fields[..] else {
            return Err(format!(
                "paint stamp \"{item}\" must be: bone x y z radius strength"
            ));
        };
        let number = |s: &str| {
            s.parse::<f32>()
                .ok()
                .filter(|v| v.is_finite())
                .ok_or_else(|| format!("\"{s}\" is not a number in paint stamp \"{item}\""))
        };
        let stamp = Stamp {
            bone: bone.to_owned(),
            at: [number(x)?, number(y)?, number(z)?],
            radius: number(radius)?,
            strength: number(strength)?,
        };
        if stamp.radius <= 0.0 || stamp.radius > 1000.0 || !(-1.0..=1.0).contains(&stamp.strength) {
            return Err(format!(
                "paint stamp \"{item}\": radius must be above 0 and strength from -1 to 1"
            ));
        }
        stamps.push(stamp);
    }
    if stamps.len() > MAX_STAMPS {
        return Err(format!(
            "a skin takes at most {MAX_STAMPS} paint stamps, got {}",
            stamps.len()
        ));
    }
    Ok(stamps)
}

/// Applies the stamps in order to one vertex's weights over all bones (`weights` sums to 1; each stamp comes with the
/// index of the bone it paints). The result sums to 1 again; a vertex whose weight was erased entirely keeps its
/// unpainted weights.
pub fn apply(point: [f32; 3], weights: &mut [f64], stamps: &[(usize, &Stamp)]) {
    let before = weights.to_vec();
    for (bone, stamp) in stamps {
        let d = (0..3)
            .map(|i| f64::from(point[i] - stamp.at[i]).powi(2))
            .sum::<f64>()
            .sqrt();
        let t = 1.0 - d / f64::from(stamp.radius);
        if t <= 0.0 || *bone >= weights.len() {
            continue;
        }
        let k = t * t * (3.0 - 2.0 * t) * f64::from(stamp.strength);
        if k >= 0.0 {
            // Pull toward the bone: blend the whole vector toward all weight on it.
            for (i, w) in weights.iter_mut().enumerate() {
                let target = if i == *bone { 1.0 } else { 0.0 };
                *w += (target - *w) * k;
            }
        } else {
            weights[*bone] *= 1.0 + k;
            let total: f64 = weights.iter().sum();
            if total > 1e-12 {
                for w in weights.iter_mut() {
                    *w /= total;
                }
            }
        }
    }
    let total: f64 = weights.iter().sum();
    if total > 1e-12 {
        for w in weights.iter_mut() {
            *w /= total;
        }
    } else {
        weights.copy_from_slice(&before);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stamp(bone: &str, at: [f32; 3], radius: f32, strength: f32) -> Stamp {
        Stamp {
            bone: bone.to_owned(),
            at,
            radius,
            strength,
        }
    }

    #[test]
    fn parses_and_refuses_bad_stamps() {
        let stamps = parse("arm 0 1 0 0.5 1; leg 0 -1 0 0.3 -0.5").unwrap_or_default();
        assert_eq!(stamps.len(), 2);
        assert_eq!(stamps[1], stamp("leg", [0.0, -1.0, 0.0], 0.3, -0.5));
        for bad in [
            "arm 0 1 0 0.5",
            "arm a 1 0 0.5 1",
            "arm 0 1 0 0 1",
            "arm 0 1 0 0.5 2",
        ] {
            assert!(parse(bad).is_err(), "{bad}");
        }
        assert!(parse(&"a 0 0 0 1 1;".repeat(65)).is_err());
    }

    #[test]
    fn a_full_stamp_hands_over_the_centre_and_fades_to_nothing_at_the_radius() {
        let s = stamp("b", [0.0; 3], 1.0, 1.0);
        let mut w = [0.7, 0.3];
        apply([0.0; 3], &mut w, &[(1, &s)]);
        assert!((w[1] - 1.0).abs() < 1e-12 && w[0].abs() < 1e-12, "{w:?}");
        let mut w = [0.7, 0.3];
        apply([1.0, 0.0, 0.0], &mut w, &[(1, &s)]);
        assert!(
            (w[0] - 0.7).abs() < 1e-12,
            "outside the brush nothing changes: {w:?}"
        );
        let mut w = [0.7, 0.3];
        apply([0.5, 0.0, 0.0], &mut w, &[(1, &s)]);
        assert!(
            w[1] > 0.3 && w[1] < 1.0 && (w[0] + w[1] - 1.0).abs() < 1e-12,
            "halfway is partial: {w:?}"
        );
    }

    #[test]
    fn a_negative_stamp_removes_a_bone_and_keeps_the_sum() {
        let s = stamp("b", [0.0; 3], 1.0, -1.0);
        let mut w = [0.5, 0.5];
        apply([0.0; 3], &mut w, &[(1, &s)]);
        assert!((w[0] - 1.0).abs() < 1e-12 && w[1].abs() < 1e-12, "{w:?}");
        let mut only = [0.0, 1.0];
        apply([0.0; 3], &mut only, &[(1, &s)]);
        assert!(
            only[0].abs() < 1e-12 && (only[1] - 1.0).abs() < 1e-12,
            "erasing the only weight leaves the unpainted weights"
        );
    }
}
