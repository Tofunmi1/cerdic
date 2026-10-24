use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{BrainError, Ctx};

/// A ranked utility decision.
#[derive(Clone, Debug)]
pub struct Choice {
    /// Winning option, or `None` when every score is non-positive.
    pub name: Option<String>,
    /// Winning score, including momentum.
    pub score: f64,
    /// Raw scores ranked best first.
    pub ranking: Vec<(String, f64)>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Curve {
    Linear { min: f64, max: f64 },
    Inverse { min: f64, max: f64 },
    Quadratic { min: f64, max: f64, power: i32 },
    Step { threshold: f64 },
    SoftStep { mid: f64, steepness: f64 },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Consideration {
    key: String,
    curve: Curve,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct OptionSpec {
    name: String,
    #[serde(default = "one")]
    weight: f64,
    considerations: Vec<Consideration>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Spec {
    #[serde(default)]
    momentum: f64,
    options: Vec<OptionSpec>,
}
fn one() -> f64 {
    1.0
}

/// Deterministic response curve utility brain.
#[derive(Clone, Debug)]
pub struct UtilityBrain {
    spec: Spec,
    current: Option<usize>,
}

impl UtilityBrain {
    /// Parses and validates a utility brain JSON definition.
    ///
    /// # Errors
    /// Returns `BadSpec` when the JSON or any option/curve is invalid.
    pub fn from_json(text: &str) -> Result<Self, BrainError> {
        let spec: Spec =
            serde_json::from_str(text).map_err(|e| BrainError::BadSpec(e.to_string()))?;
        if spec.options.is_empty() {
            return Err(BrainError::BadSpec("no options".into()));
        }
        let mut names = BTreeSet::new();
        for option in &spec.options {
            if option.name.trim().is_empty() {
                return Err(BrainError::BadSpec("empty option name".into()));
            }
            if !names.insert(&option.name) {
                return Err(BrainError::BadSpec(format!(
                    "duplicate option {}",
                    option.name
                )));
            }
            if option.weight < 0.0 {
                return Err(BrainError::BadSpec("negative weight".into()));
            }
            for item in &option.considerations {
                match item.curve {
                    Curve::Linear { min, max }
                    | Curve::Inverse { min, max }
                    | Curve::Quadratic { min, max, .. }
                        if max <= min =>
                    {
                        return Err(BrainError::BadSpec("max must exceed min".into()));
                    }
                    Curve::Quadratic { power, .. } if !(1..=8).contains(&power) => {
                        return Err(BrainError::BadSpec("power must be between 1 and 8".into()));
                    }
                    _ => {}
                }
            }
        }
        Ok(Self {
            spec,
            current: None,
        })
    }
    /// Selects the highest scoring option using the current blackboard.
    ///
    /// # Errors
    /// Returns `BadSpec` if the brain has no options.
    pub fn choose(&mut self, ctx: &Ctx) -> Result<Choice, BrainError> {
        let mut scores: Vec<(String, f64)> = self
            .spec
            .options
            .iter()
            .map(|o| {
                let mut product = o
                    .considerations
                    .iter()
                    .map(|c| {
                        ctx.blackboard
                            .get_number(&c.key, ctx.now)
                            .map_or(0.0, |v| c.curve.score(v))
                    })
                    .product::<f64>();
                let n = o.considerations.len();
                if n >= 2 {
                    let count = u32::try_from(n).unwrap_or(u32::MAX);
                    product += (1.0 - product) * (1.0 - 1.0 / f64::from(count)) * product;
                }
                (o.name.clone(), product * o.weight)
            })
            .collect();
        let mut ranked = scores.clone();
        ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
        let winner = scores.iter().enumerate().max_by(|(ia, a), (ib, b)| {
            let sa = a.1
                * if Some(*ia) == self.current {
                    1.0 + self.spec.momentum
                } else {
                    1.0
                };
            let sb = b.1
                * if Some(*ib) == self.current {
                    1.0 + self.spec.momentum
                } else {
                    1.0
                };
            sa.total_cmp(&sb).then_with(|| ib.cmp(ia))
        });
        let (index, item) = winner.ok_or_else(|| BrainError::BadSpec("no options".into()))?;
        let score = item.1
            * if Some(index) == self.current {
                1.0 + self.spec.momentum
            } else {
                1.0
            };
        let name = (score > 0.0).then(|| item.0.clone());
        self.current = name.as_ref().map(|_| index);
        scores.clear();
        Ok(Choice {
            name,
            score,
            ranking: ranked,
        })
    }
    /// Serializes this brain definition as JSON.
    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string(&self.spec).unwrap_or_default()
    }
}

impl Curve {
    fn score(&self, value: f64) -> f64 {
        match *self {
            Self::Linear { min, max } => ((value - min) / (max - min)).clamp(0.0, 1.0),
            Self::Inverse { min, max } => 1.0 - ((value - min) / (max - min)).clamp(0.0, 1.0),
            Self::Quadratic { min, max, power } => {
                ((value - min) / (max - min)).clamp(0.0, 1.0).powi(power)
            }
            Self::Step { threshold } => {
                if value >= threshold {
                    1.0
                } else {
                    0.0
                }
            }
            Self::SoftStep { mid, steepness } => {
                let t = steepness * (value - mid);
                0.5 + 0.5 * t / (1.0 + t.abs())
            }
        }
    }
}
