use crate::{Fail, fail, noise::value_noise};
use ve_graph::{Node, Value as Param};

/// Most control points in a lattice deformer.
pub(super) const MAX_LATTICE_POINTS: usize = 64;

/// A lattice of control points over the bounding box with sparse offsets (free-form deformation).
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Lattice {
    /// Control points per axis (2 to 4).
    pub(super) divisions: usize,
    /// Offsets in metres, indexed `(i * divisions + j) * divisions + k`; all zero is the identity.
    pub(super) offsets: Vec<[f64; 3]>,
}

/// One deformation.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Map {
    Twist(f64),
    Taper(f64),
    Bend(f64),
    /// The top moves by `[x, z]` metres relative to the bottom, in proportion to height.
    Shear([f64; 2]),
    Wave {
        amplitude: f64,
        wavelength: f64,
        phase: f64,
        along_z: bool,
    },
    Noise {
        amplitude: f64,
        scale: f64,
        seed: u32,
    },
    Lattice(Lattice),
}

impl Map {
    /// How many times refinement may split edges: the original three bend gently and keep two passes; the ones that can
    /// ripple at any scale get more (only the edges that need it are split, and the triangle cap still applies).
    pub(super) fn passes(&self) -> usize {
        match self {
            Self::Twist(_) | Self::Taper(_) | Self::Bend(_) => 2,
            _ => 7,
        }
    }

    /// Reads the deformer's parameters.
    pub(super) fn from_node(node: &Node) -> Result<Self, Fail> {
        let family = node.op.family();
        Ok(match family.as_str() {
            "xform.twist" => Self::Twist(number(node, "angle")?),
            "xform.taper" => Self::Taper(number(node, "top_scale")?),
            "xform.bend" => Self::Bend(number(node, "angle")?),
            "xform.shear" => Self::Shear([number(node, "x_shift")?, number(node, "z_shift")?]),
            "xform.wave" => Self::Wave {
                amplitude: number(node, "amplitude")?,
                wavelength: number(node, "wavelength")?,
                phase: number(node, "phase")?,
                along_z: matches!(node.params.get("direction"), Some(Param::Str(name)) if name == "z"),
            },
            "xform.noise" => Self::Noise {
                amplitude: number(node, "amplitude")?,
                scale: number(node, "scale")?,
                seed: match node.params.get("seed") {
                    Some(Param::Int(value)) => u32::try_from(*value).unwrap_or(0),
                    _ => 0,
                },
            },
            "xform.lattice" => Self::Lattice(lattice(node)?),
            other => {
                return Err(fail(
                    "graph.unknown_op",
                    format!("{other} is not a deformer"),
                ));
            }
        })
    }

    /// Moves a point of a solid whose bounding box is `minimum` to `maximum`.
    pub(super) fn apply(&self, point: [f64; 3], minimum: [f64; 3], maximum: [f64; 3]) -> [f64; 3] {
        match self {
            Self::Twist(angle) => twist(point, *angle, minimum, maximum),
            Self::Taper(top) => taper(point, *top, minimum, maximum),
            Self::Bend(angle) => bend(point, *angle, minimum, maximum),
            Self::Shear(shift) => {
                let t = ratio(point[1], minimum[1], maximum[1]);
                [point[0] + shift[0] * t, point[1], point[2] + shift[1] * t]
            }
            Self::Wave {
                amplitude,
                wavelength,
                phase,
                along_z,
            } => {
                let wave = amplitude
                    * (std::f64::consts::TAU * (point[1] - minimum[1]) / wavelength + phase).sin();
                if *along_z {
                    [point[0], point[1], point[2] + wave]
                } else {
                    [point[0] + wave, point[1], point[2]]
                }
            }
            Self::Noise {
                amplitude,
                scale,
                seed,
            } => {
                let at = [point[0] / scale, point[1] / scale, point[2] / scale];
                [
                    point[0] + amplitude * value_noise(at, seed.wrapping_mul(3)),
                    point[1] + amplitude * value_noise(at, seed.wrapping_mul(3).wrapping_add(1)),
                    point[2] + amplitude * value_noise(at, seed.wrapping_mul(3).wrapping_add(2)),
                ]
            }
            Self::Lattice(grid) => grid.apply(point, minimum, maximum),
        }
    }

    /// The bend and nothing else needs the box checked against its radius; everything else is always valid.
    pub(super) fn bend_angle(&self) -> Option<f64> {
        match self {
            Self::Bend(angle) => Some(*angle),
            _ => None,
        }
    }
}

impl Lattice {
    /// Bernstein weights of control index `i` of `n` points at parameter `t`.
    #[expect(clippy::cast_precision_loss, reason = "n is at most 4")]
    fn bernstein(n: usize, i: usize, t: f64) -> f64 {
        let degree = n - 1;
        let choose = |a: usize, b: usize| {
            (1..=b).fold(1.0_f64, |acc, step| {
                acc * (a + 1 - step) as f64 / step as f64
            })
        };
        choose(degree, i)
            * t.powi(i32::try_from(i).unwrap_or(0))
            * (1.0 - t).powi(i32::try_from(degree - i).unwrap_or(0))
    }

    fn apply(&self, point: [f64; 3], minimum: [f64; 3], maximum: [f64; 3]) -> [f64; 3] {
        let n = self.divisions;
        let local =
            [0, 1, 2].map(|axis| ratio(point[axis], minimum[axis], maximum[axis]).clamp(0.0, 1.0));
        let mut moved = point;
        for i in 0..n {
            let wi = Self::bernstein(n, i, local[0]);
            for j in 0..n {
                let wj = wi * Self::bernstein(n, j, local[1]);
                for k in 0..n {
                    let weight = wj * Self::bernstein(n, k, local[2]);
                    let offset = self.offsets[(i * n + j) * n + k];
                    for axis in 0..3 {
                        moved[axis] += weight * offset[axis];
                    }
                }
            }
        }
        moved
    }
}

/// Parses `divisions` and the sparse `offsets` string `"i j k dx dy dz; ..."`.
fn lattice(node: &Node) -> Result<Lattice, Fail> {
    let divisions = match node.params.get("divisions") {
        Some(Param::Int(value)) => usize::try_from(*value).unwrap_or(0),
        _ => 3,
    };
    if !(2..=4).contains(&divisions) {
        return Err(fail(
            "param.range",
            "xform.lattice divisions must be 2 to 4",
        ));
    }
    let mut offsets = vec![[0.0; 3]; divisions * divisions * divisions];
    let Some(Param::Str(text)) = node.params.get("offsets") else {
        return Ok(Lattice { divisions, offsets });
    };
    let mut seen = 0;
    for item in text
        .split(';')
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        let fields: Vec<&str> = item.split_whitespace().collect();
        let [i, j, k, dx, dy, dz] = fields[..] else {
            return Err(fail(
                "param.type",
                format!("lattice offset \"{item}\" must be: i j k dx dy dz"),
            ));
        };
        let index = |s: &str| s.parse::<usize>().ok().filter(|v| *v < divisions);
        let (Some(i), Some(j), Some(k)) = (index(i), index(j), index(k)) else {
            return Err(fail(
                "param.range",
                format!(
                    "lattice offset \"{item}\": indices run from 0 to {}",
                    divisions - 1
                ),
            ));
        };
        let metres = |s: &str| {
            s.parse::<f64>()
                .ok()
                .filter(|v| v.is_finite() && v.abs() <= 1000.0)
        };
        let (Some(dx), Some(dy), Some(dz)) = (metres(dx), metres(dy), metres(dz)) else {
            return Err(fail(
                "param.range",
                format!("lattice offset \"{item}\": offsets must be numbers within 1000 m"),
            ));
        };
        seen += 1;
        if seen > MAX_LATTICE_POINTS {
            return Err(fail(
                "param.range",
                format!("xform.lattice takes at most {MAX_LATTICE_POINTS} offsets"),
            ));
        }
        offsets[(i * divisions + j) * divisions + k] = [dx, dy, dz];
    }
    Ok(Lattice { divisions, offsets })
}

fn number(node: &Node, name: &str) -> Result<f64, Fail> {
    node.params
        .get(name)
        .and_then(Param::as_f64)
        .ok_or_else(|| {
            fail(
                "param.type",
                format!("deformer parameter {name} is not numeric"),
            )
        })
}

pub(super) fn ratio(value: f64, minimum: f64, maximum: f64) -> f64 {
    let span = maximum - minimum;
    if span.abs() <= 1.0e-12 {
        0.5
    } else {
        (value - minimum) / span
    }
}

fn midpoint(a: f64, b: f64) -> f64 {
    f64::midpoint(a, b)
}

fn twist(point: [f64; 3], angle: f64, minimum: [f64; 3], maximum: [f64; 3]) -> [f64; 3] {
    let t = ratio(point[1], minimum[1], maximum[1]);
    let (sine, cosine) = (angle * t).sin_cos();
    let (x, z) = (
        point[0] - midpoint(minimum[0], maximum[0]),
        point[2] - midpoint(minimum[2], maximum[2]),
    );
    [
        midpoint(minimum[0], maximum[0]) + x * cosine - z * sine,
        point[1],
        midpoint(minimum[2], maximum[2]) + x * sine + z * cosine,
    ]
}

fn taper(point: [f64; 3], top_scale: f64, minimum: [f64; 3], maximum: [f64; 3]) -> [f64; 3] {
    let scale = 1.0 + (top_scale - 1.0) * ratio(point[1], minimum[1], maximum[1]);
    let center = [
        midpoint(minimum[0], maximum[0]),
        midpoint(minimum[2], maximum[2]),
    ];
    [
        center[0] + (point[0] - center[0]) * scale,
        point[1],
        center[1] + (point[2] - center[1]) * scale,
    ]
}

fn bend(point: [f64; 3], angle: f64, minimum: [f64; 3], maximum: [f64; 3]) -> [f64; 3] {
    if angle.abs() <= 1.0e-12 {
        return point;
    }
    let length = maximum[1] - minimum[1];
    let radius = length / angle;
    let (sine, cosine) = (angle * ratio(point[1], minimum[1], maximum[1])).sin_cos();
    let offset_x = point[0] - midpoint(minimum[0], maximum[0]);
    [
        midpoint(minimum[0], maximum[0]) + radius * (1.0 - cosine) + offset_x * cosine,
        minimum[1] + radius * sine - offset_x * sine,
        point[2],
    ]
}
