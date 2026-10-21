//! Named anthropometric measurements derived from an evaluated body.
use crate::{Body, Error, Human};

/// A body measurement. Distances and circumferences are in metres; mass is in kilograms.
#[derive(Clone, Debug, PartialEq)]
pub enum Measure {
    /// Maximum minus minimum body height.
    Stature,
    /// Signed triangle-surface volume multiplied by 980 kg/m³.
    Mass,
    /// Distance between left and right upper arm heads.
    ShoulderWidth,
    /// Left upper arm to wrist length along the upper/lower arm and wrist chain.
    ArmLength,
    /// Upper leg head height above the lowest body point.
    LegLength,
    /// Narrowest circumference between the neck01 and head bone heads.
    NeckCircumference,
    /// Circumference just below the armpit between spine03 and spine02.
    ChestCircumference,
    /// Circumference at the narrowest sampled torso section from spine02 to pelvis.
    WaistCircumference,
    /// Circumference at the widest sampled glute section below the pelvis.
    HipCircumference,
    /// X extent of vertices primarily weighted to the head bone.
    HeadWidth,
    /// Distance between two bone heads named in the pack.
    BoneDistance(String, String),
}

impl Measure {
    /// Return the canonical `snake_case` spelling of this measure.
    #[must_use]
    pub fn name(&self) -> String {
        match self {
            Self::Stature => "stature".into(),
            Self::Mass => "mass".into(),
            Self::ShoulderWidth => "shoulder_width".into(),
            Self::ArmLength => "arm_length".into(),
            Self::LegLength => "leg_length".into(),
            Self::NeckCircumference => "neck_circumference".into(),
            Self::ChestCircumference => "chest_circumference".into(),
            Self::WaistCircumference => "waist_circumference".into(),
            Self::HipCircumference => "hip_circumference".into(),
            Self::HeadWidth => "head_width".into(),
            Self::BoneDistance(a, b) => format!("bone_distance:{a}:{b}"),
        }
    }

    /// Parse a canonical measure name.
    ///
    /// # Errors
    /// Returns an error naming an unknown spelling and all valid fixed names.
    pub fn parse(name: impl AsRef<str>) -> Result<Self, Error> {
        let name = name.as_ref();
        let measure = match name {
            "stature" => Some(Self::Stature),
            "mass" => Some(Self::Mass),
            "shoulder_width" => Some(Self::ShoulderWidth),
            "arm_length" => Some(Self::ArmLength),
            "leg_length" => Some(Self::LegLength),
            "neck_circumference" => Some(Self::NeckCircumference),
            "chest_circumference" => Some(Self::ChestCircumference),
            "waist_circumference" => Some(Self::WaistCircumference),
            "hip_circumference" => Some(Self::HipCircumference),
            "head_width" => Some(Self::HeadWidth),
            _ => None,
        };
        if let Some(value) = measure {
            return Ok(value);
        }
        if let Some(pair) = name.strip_prefix("bone_distance:") {
            let mut parts = pair.split(':');
            if let (Some(a), Some(b), None) = (parts.next(), parts.next(), parts.next())
                && !a.is_empty()
                && !b.is_empty()
            {
                return Ok(Self::BoneDistance(a.into(), b.into()));
            }
        }
        Err(Error::InvalidPack(format!(
            "unknown measure '{name}'; valid names: stature, mass, shoulder_width, arm_length, leg_length, neck_circumference, chest_circumference, waist_circumference, hip_circumference, head_width, bone_distance:<a>:<b>"
        )))
    }
}

/// Measure an evaluated body using its model topology and skeleton.
///
/// # Errors
/// Returns an error when a requested bone name is absent.
pub fn measure(human: &Human, body: &Body, what: &Measure) -> Result<f64, Error> {
    let min_y = body
        .positions
        .iter()
        .map(|p| p[1])
        .fold(f32::INFINITY, f32::min);
    let max_y = body
        .positions
        .iter()
        .map(|p| p[1])
        .fold(f32::NEG_INFINITY, f32::max);
    let bones = human.bone_names();
    let head = |name: &str| -> Result<[f32; 3], Error> {
        bones
            .iter()
            .position(|n| n == name)
            .and_then(|i| body.bone_heads.get(i).copied())
            .ok_or_else(|| Error::InvalidPack(format!("unknown bone '{name}'")))
    };
    let distance = |a: [f32; 3], b: [f32; 3]| -> f64 {
        f64::from(((0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f32>()).sqrt())
    };
    let value = match what {
        Measure::Stature => f64::from(max_y - min_y),
        Measure::Mass => signed_volume(human, body) * 980.0,
        Measure::ShoulderWidth => distance(head("upperarm01.L")?, head("upperarm01.R")?),
        Measure::ArmLength => ["upperarm01.L", "lowerarm01.L", "wrist.L"]
            .windows(2)
            .map(|pair| Ok(distance(head(pair[0])?, head(pair[1])?)))
            .collect::<Result<Vec<_>, Error>>()?
            .iter()
            .sum(),
        Measure::LegLength => f64::from(head("upperleg01.L")?[1] - min_y),
        // The narrowest section in the lower half of the neck: higher up the section runs into the jaw and the loop is the head's.
        Measure::NeckCircumference => {
            let (base, skull) = (head("neck01")?[1], head("head")?[1]);
            sampled_torso(human, body, base, base + (skull - base) * 0.5, false)
        }
        Measure::ChestCircumference => section_perimeter(
            human,
            body,
            head("spine03")?[1].min(head("spine02")?[1]) - 0.025,
        ),
        Measure::WaistCircumference => sampled_torso(
            human,
            body,
            head("spine02")?[1],
            head("pelvis.L")?[1],
            false,
        ),
        Measure::HipCircumference => sampled_torso(
            human,
            body,
            head("pelvis.L")?[1] - 0.16,
            head("pelvis.L")?[1],
            true,
        ),
        Measure::HeadWidth => head_width(human, body)?,
        Measure::BoneDistance(a, b) => distance(head(a)?, head(b)?),
    };
    Ok(value)
}

fn section_segments(human: &Human, body: &Body, y: f32) -> Vec<([f32; 2], [f32; 2])> {
    let mut segments = Vec::new();
    for quad in &human.quads {
        let p = quad.map(|index| body.positions[index as usize]);
        for tri in [[p[0], p[1], p[2]], [p[0], p[2], p[3]]] {
            let crossings: Vec<[f32; 2]> = (0..3)
                .filter_map(|i| {
                    let a = tri[i];
                    let b = tri[(i + 1) % 3];
                    ((a[1] <= y && b[1] > y) || (b[1] <= y && a[1] > y)).then(|| {
                        let t = (y - a[1]) / (b[1] - a[1]);
                        [a[0] + t * (b[0] - a[0]), a[2] + t * (b[2] - a[2])]
                    })
                })
                .collect();
            if let [a, b] = crossings.as_slice() {
                segments.push((*a, *b));
            }
        }
    }
    segments
}

fn section_perimeter(human: &Human, body: &Body, y: f32) -> f64 {
    loops(section_segments(human, body, y))
        .into_iter()
        .filter_map(|points| {
            let perimeter = loop_perimeter(&points);
            let min_x = points.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
            let max_x = points
                .iter()
                .map(|p| p[0])
                .fold(f32::NEG_INFINITY, f32::max);
            (perimeter > 0.2 && min_x <= 0.0 && max_x >= 0.0)
                .then_some((contains_axis(&points), perimeter))
        })
        .max_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)))
        .map_or(0.0, |(_, perimeter)| perimeter)
}

fn loops(segments: Vec<([f32; 2], [f32; 2])>) -> Vec<Vec<[f32; 2]>> {
    let mut remaining = segments;
    let mut result = Vec::new();
    while let Some((start, next)) = remaining.pop() {
        let mut chain = vec![start, next];
        while !same_point(*chain.last().unwrap_or(&start), start) {
            let end = *chain.last().unwrap_or(&start);
            let found = remaining
                .iter()
                .position(|(a, b)| same_point(*a, end) || same_point(*b, end));
            let Some(index) = found else { break };
            let (a, b) = remaining.swap_remove(index);
            chain.push(if same_point(a, end) { b } else { a });
        }
        if chain.len() >= 4 && same_point(*chain.last().unwrap_or(&start), start) {
            chain.pop();
            result.push(chain);
        }
    }
    result
}

fn same_point(a: [f32; 2], b: [f32; 2]) -> bool {
    (a[0] - b[0]).abs() < 1e-5 && (a[1] - b[1]).abs() < 1e-5
}

fn contains_axis(points: &[[f32; 2]]) -> bool {
    let z_low = points.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
    let z_high = points
        .iter()
        .map(|p| p[1])
        .fold(f32::NEG_INFINITY, f32::max);
    let axis_z = (z_low + z_high) * 0.5;
    let mut inside = false;
    for (i, a) in points.iter().enumerate() {
        let b = points[(i + 1) % points.len()];
        if (a[1] > axis_z) != (b[1] > axis_z)
            && 0.0 < (b[0] - a[0]) * (axis_z - a[1]) / (b[1] - a[1]) + a[0]
        {
            inside = !inside;
        }
    }
    inside
}

fn loop_perimeter(points: &[[f32; 2]]) -> f64 {
    points
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let b = points[(i + 1) % points.len()];
            f64::from(((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt())
        })
        .sum()
}

#[cfg(test)]
fn hull_perimeter(mut pts: Vec<[f32; 2]>) -> f64 {
    pts.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    pts.dedup();
    if pts.len() < 3 {
        return 0.0;
    }
    let cross = |o: [f32; 2], a: [f32; 2], b: [f32; 2]| {
        (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
    };
    let mut hull = Vec::new();
    for &p in &pts {
        while hull.len() >= 2 && cross(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0.0 {
            hull.pop();
        }
        hull.push(p);
    }
    let lower = hull.len();
    for &p in pts.iter().rev().skip(1) {
        while hull.len() > lower && cross(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0.0 {
            hull.pop();
        }
        hull.push(p);
    }
    hull.pop();
    hull.iter()
        .enumerate()
        .map(|(i, a)| {
            let b = hull[(i + 1) % hull.len()];
            f64::from(((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt())
        })
        .sum()
}

/// The widest or narrowest torso section between two heights; heights where no loop encloses the body axis are skipped.
fn sampled_torso(human: &Human, body: &Body, low: f32, high: f32, widest: bool) -> f64 {
    (0_u16..=20)
        .map(|i| section_perimeter(human, body, low + (high - low) * f32::from(i) / 20.0))
        .filter(|perimeter| *perimeter > 0.0)
        .reduce(|a, b| if (b > a) == widest { b } else { a })
        .unwrap_or(0.0)
}

fn head_width(human: &Human, body: &Body) -> Result<f64, Error> {
    let index = human
        .bone_names()
        .iter()
        .position(|n| n == "head")
        .ok_or_else(|| Error::InvalidPack("unknown bone 'head'".into()))?;
    let skin = human.skin();
    let xs: Vec<f32> = body
        .positions
        .iter()
        .enumerate()
        .filter(|(i, _)| {
            skin[*i]
                .iter()
                .any(|(b, w)| usize::from(*b) == index && *w > 0.5)
        })
        .map(|(_, p)| p[0])
        .collect();
    Ok(f64::from(
        xs.iter().copied().fold(f32::NEG_INFINITY, f32::max)
            - xs.iter().copied().fold(f32::INFINITY, f32::min),
    ))
}

fn signed_volume(human: &Human, body: &Body) -> f64 {
    let mesh = human.render_mesh();
    mesh.triangles
        .iter()
        .map(|t| {
            let a = body.render_positions[t[0] as usize];
            let b = body.render_positions[t[1] as usize];
            let c = body.render_positions[t[2] as usize];
            f64::from(a[0])
                * (f64::from(b[1]) * f64::from(c[2]) - f64::from(b[2]) * f64::from(c[1]))
                + f64::from(a[1])
                    * (f64::from(b[2]) * f64::from(c[0]) - f64::from(b[0]) * f64::from(c[2]))
                + f64::from(a[2])
                    * (f64::from(b[0]) * f64::from(c[1]) - f64::from(b[1]) * f64::from(c[0]))
        })
        .sum::<f64>()
        / 6.0
}

#[cfg(test)]
mod tests {
    use super::hull_perimeter;
    #[test]
    fn square_section_has_known_perimeter() {
        let perimeter = hull_perimeter(vec![
            [0.0, 0.0],
            [2.0, 0.0],
            [2.0, 1.0],
            [0.0, 1.0],
            [1.0, 0.5],
        ]);
        assert!((perimeter - 6.0).abs() < 1e-6);
    }
}
