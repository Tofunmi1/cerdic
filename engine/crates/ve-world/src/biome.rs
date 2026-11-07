//! Biomes: named kinds of ground (forest, meadow, scree) that blend into each other. A biome is a set of soft windows over what the terrain can say about a
//! place: its height in metres, its steepness in degrees from flat, and its moisture (a slow world-space noise). Where a place falls inside every window
//! of a biome the biome has weight there; windows have soft edges (`blend` is their width as a share of the window), so weights fade across a boundary
//! instead of cutting it. Weights are normalised where biomes overlap so they never add up to more than one; where none applies they stay zero.
//! Everything is computed per cell from world coordinates and central differences, so two tiles that share an edge agree on it.
//!
//! The rules are written as text, one biome per `;`: `forest: height 0..80, slope 0..30, wet 0.45..1; rock: slope 38..90`. A window is `low..high`, either
//! end may be left out (`slope 38..` is "38 and steeper").

use crate::noise::{Basis, Fractal, FractalKind, Noise};
use crate::terrain::HeightGrid;

/// Most biomes in one rule set.
pub const MAX_BIOMES: usize = 12;

/// A window over one measure; an open end is infinite.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Window {
    /// Where the biome starts (inclusive, softened by the blend).
    pub low: f32,
    /// Where it ends.
    pub high: f32,
}

/// One biome: its name and the windows it needs (absent means anywhere).
#[derive(Clone, Debug, PartialEq)]
pub struct Biome {
    /// Layer-safe name (letters, digits, underscore).
    pub name: String,
    /// Ground height above sea level, in metres.
    pub height: Option<Window>,
    /// Steepness in degrees from flat.
    pub slope: Option<Window>,
    /// Moisture, 0 dry to 1 wet.
    pub wet: Option<Window>,
}

/// Why a rule text was refused, in words that say the fix.
fn bad(text: String) -> String {
    text
}

/// Reads the rules text.
///
/// # Errors
/// A message naming the biome and the piece that could not be read, or the limits.
pub fn parse(text: &str) -> Result<Vec<Biome>, String> {
    let mut biomes = Vec::new();
    for part in text
        .split([';', '\n'])
        .map(str::trim)
        .filter(|p| !p.is_empty())
    {
        let (name, rules) = part.split_once(':').ok_or_else(|| {
            bad(format!(
                "\"{part}\" needs a name and a colon, like `forest: slope 0..30`"
            ))
        })?;
        let name = name.trim();
        if name.is_empty()
            || name.len() > 24
            || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return Err(bad(format!(
                "the biome name \"{name}\" must be 1 to 24 letters, digits or underscores"
            )));
        }
        if biomes.iter().any(|b: &Biome| b.name == name) {
            return Err(bad(format!("the biome \"{name}\" is listed twice")));
        }
        let mut biome = Biome {
            name: name.to_owned(),
            height: None,
            slope: None,
            wet: None,
        };
        for rule in rules.split(',').map(str::trim).filter(|r| !r.is_empty()) {
            let (measure, range) = rule.split_once(char::is_whitespace).ok_or_else(|| {
                bad(format!(
                    "{name}: \"{rule}\" needs a measure and a range, like `slope 0..30`"
                ))
            })?;
            let (low, high) = range.trim().split_once("..").ok_or_else(|| bad(format!("{name}: the range \"{range}\" must look like 0..30 (either end may be left out)")))?;
            let number = |s: &str, default: f32| -> Result<f32, String> {
                if s.trim().is_empty() {
                    Ok(default)
                } else {
                    s.trim()
                        .parse::<f32>()
                        .ok()
                        .filter(|v| v.is_finite())
                        .ok_or_else(|| bad(format!("{name}: \"{s}\" is not a number")))
                }
            };
            let window = Window {
                low: number(low, f32::NEG_INFINITY)?,
                high: number(high, f32::INFINITY)?,
            };
            if window.low > window.high {
                return Err(bad(format!(
                    "{name}: {measure} {range} has its low end above its high end"
                )));
            }
            // A window that reaches the end of what a measure can be is open there: nothing lies below 0 degrees of slope or above 1 of moisture,
            // so the edge must not fade.
            let open = |window: Window, floor: f32, ceiling: f32| Window {
                low: if window.low <= floor {
                    f32::NEG_INFINITY
                } else {
                    window.low
                },
                high: if window.high >= ceiling {
                    f32::INFINITY
                } else {
                    window.high
                },
            };
            match measure {
                "height" => biome.height = Some(window),
                "slope" => biome.slope = Some(open(window, 0.0, 90.0)),
                "wet" => biome.wet = Some(open(window, 0.0, 1.0)),
                other => {
                    return Err(bad(format!(
                        "{name}: unknown measure \"{other}\"; use height (metres), slope (degrees) or wet (0 to 1)"
                    )));
                }
            }
        }
        biomes.push(biome);
    }
    if biomes.is_empty() {
        return Err(bad(
            "name at least one biome, like `forest: slope 0..30`".to_owned()
        ));
    }
    if biomes.len() > MAX_BIOMES {
        return Err(bad(format!(
            "at most {MAX_BIOMES} biomes, got {}",
            biomes.len()
        )));
    }
    Ok(biomes)
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if edge1 <= edge0 {
        return if x >= edge0 { 1.0 } else { 0.0 };
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// How far `value` is inside `window` (0 outside, 1 well inside), with edges softened by `blend` of the window's width (a window with an open end
/// softens its closed end by the blend of `unit`).
fn membership(window: Window, value: f32, blend: f32, unit: f32) -> f32 {
    let width = if window.low.is_finite() && window.high.is_finite() {
        window.high - window.low
    } else {
        unit
    };
    let soft = (blend * width * 0.5).max(1.0e-4);
    let rise = if window.low.is_finite() {
        smoothstep(window.low - soft, window.low + soft, value)
    } else {
        1.0
    };
    let fall = if window.high.is_finite() {
        1.0 - smoothstep(window.high - soft, window.high + soft, value)
    } else {
        1.0
    };
    rise * fall
}

/// The per-cell weights of every biome and the moisture, row-major like the grid.
pub struct Weights {
    /// One layer per biome, in the order of the rules.
    pub biomes: Vec<Vec<f32>>,
    /// The moisture noise, 0 to 1.
    pub wet: Vec<f32>,
    /// Steepness in degrees from flat.
    pub slope: Vec<f32>,
}

/// Works out the weights of `biomes` over `grid`. `wet_scale` is the size of the moisture patches in metres and `seed` picks the pattern; the
/// moisture is a function of world position alone.
#[must_use]
pub fn weigh(
    grid: &HeightGrid,
    biomes: &[Biome],
    blend: f32,
    wet_scale: f32,
    seed: u32,
) -> Weights {
    let (w, d) = (grid.width(), grid.depth());
    let heights = grid.heights();
    let at = |i: usize, j: usize| heights[j * w + i];
    let moisture = Noise {
        basis: Basis::Perlin,
        seed,
        frequency: 1.0 / f64::from(wet_scale.max(1.0)),
        fractal: Some(Fractal {
            kind: FractalKind::Fbm,
            octaves: 3,
            lacunarity: 2.0,
            gain: 0.5,
            weighted_strength: 0.0,
        }),
        warp: None,
    };
    let cell = grid.cell().max(1.0e-3);
    let mut out = Weights {
        biomes: vec![vec![0.0; w * d]; biomes.len()],
        wet: vec![0.0; w * d],
        slope: vec![0.0; w * d],
    };
    for j in 0..d {
        for i in 0..w {
            let k = j * w + i;
            let (x0, x1) = (i.saturating_sub(1), (i + 1).min(w - 1));
            let (z0, z1) = (j.saturating_sub(1), (j + 1).min(d - 1));
            let dx = (at(x1, j) - at(x0, j)) / ((x1 - x0).max(1) as f32 * cell);
            let dz = (at(i, z1) - at(i, z0)) / ((z1 - z0).max(1) as f32 * cell);
            let slope = (dx * dx + dz * dz).sqrt().atan().to_degrees();
            let wet = (moisture.sample2(f64::from(grid.world_x(i)), f64::from(grid.world_z(j)))
                as f32
                * 0.5
                + 0.5)
                .clamp(0.0, 1.0);
            out.slope[k] = slope;
            out.wet[k] = wet;
            let mut total = 0.0;
            for (n, biome) in biomes.iter().enumerate() {
                let mut weight = 1.0;
                if let Some(window) = biome.height {
                    weight *= membership(window, at(i, j), blend, 50.0);
                }
                if let Some(window) = biome.slope {
                    weight *= membership(window, slope, blend, 15.0);
                }
                if let Some(window) = biome.wet {
                    weight *= membership(window, wet, blend, 0.3);
                }
                out.biomes[n][k] = weight;
                total += weight;
            }
            // Overlapping biomes share the place; a place no biome claims stays empty.
            if total > 1.0 {
                for layer in &mut out.biomes {
                    layer[k] /= total;
                }
            }
        }
    }
    out
}
