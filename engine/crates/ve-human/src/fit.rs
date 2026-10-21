//! Bounded regularised fitting of named body measurements.
use crate::{Body, Error, Human, Measure, Params, measure};

/// A requested measurement and its relative importance.
#[derive(Clone, Debug)]
pub struct Target {
    /// Requested measurement.
    pub measure: Measure,
    /// Desired positive value.
    pub value: f64,
    /// Relative importance, defaulting to one.
    pub weight: f64,
}
impl Target {
    /// Create a target with unit importance.
    #[must_use]
    pub fn new(measure: Measure, value: f64) -> Self {
        Self {
            measure,
            value,
            weight: 1.0,
        }
    }
}

/// Controls which parameters can move and how fitting stops.
#[derive(Clone, Debug)]
pub struct FitOptions {
    /// Free phenotype controls.
    pub free_phenotype: Vec<String>,
    /// Free local controls.
    pub free_local: Vec<String>,
    /// Maximum solver iterations.
    pub max_iterations: usize,
    /// Penalty on movement from the starting parameters.
    pub regularisation: f64,
}
impl Default for FitOptions {
    fn default() -> Self {
        Self {
            free_phenotype: ["height", "weight", "muscle", "proportions"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            free_local: [
                "measure-upperarm-length-incr",
                "measure-upperarm-circ-incr",
                "measure-lowerarm-length-incr",
                "measure-ankle-circ-incr",
                "measure-wrist-circ-incr",
                "measure-calf-circ-incr",
                "measure-lowerleg-height-incr",
                "measure-upperleg-height-incr",
                "measure-knee-circ-incr",
                "measure-thigh-circ-incr",
                "measure-neck-circ-incr",
                "measure-neck-height-incr",
                "measure-underbust-circ-incr",
                "measure-napetowaist-dist-incr",
                "measure-waisttohip-dist-incr",
                "measure-shoulder-dist-incr",
                "measure-hips-circ-incr",
                "measure-waist-circ-incr",
                "measure-frontchest-dist-incr",
                "measure-bust-circ-incr",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            max_iterations: 20,
            regularisation: 0.01,
        }
    }
}

/// One target and its achieved value after fitting.
#[derive(Clone, Debug)]
pub struct Achieved {
    /// Measurement.
    pub measure: Measure,
    /// Requested value.
    pub target: f64,
    /// Measured result.
    pub value: f64,
}
/// Fitted body controls and convergence report.
#[derive(Clone, Debug)]
pub struct FitResult {
    /// Resulting named controls.
    pub params: Params,
    /// Final measurements.
    pub achieved: Vec<Achieved>,
    /// Whether every normalized residual is below threshold.
    pub converged: bool,
    /// Solver iterations performed.
    pub iterations: usize,
    /// Weighted normalized least-squares residual.
    pub residual: f64,
}

#[derive(Clone)]
struct Variable {
    name: String,
    local: bool,
}

/// Fit allowed controls to positive measurement targets.
///
/// # Errors
/// Returns errors for absent/non-positive targets, unknown free names, or invalid starting parameters.
#[expect(
    clippy::too_many_lines,
    reason = "one deterministic LM iteration loop keeps evaluation caching and stopping decisions together"
)]
pub fn fit(
    human: &Human,
    start: &Params,
    targets: &[Target],
    options: &FitOptions,
) -> Result<FitResult, Error> {
    if targets.is_empty() {
        return Err(Error::InvalidPack("at least one target is required".into()));
    }
    if targets
        .iter()
        .any(|t| !t.value.is_finite() || t.value <= 0.0 || !t.weight.is_finite() || t.weight <= 0.0)
    {
        return Err(Error::InvalidPack(
            "target values and weights must be positive and finite".into(),
        ));
    }
    human.evaluate(start)?;
    let mut variables = Vec::new();
    for n in &options.free_phenotype {
        if !human.phenotype_labels().contains(n) {
            return Err(unknown_free(n, human));
        }
        variables.push(Variable {
            name: n.clone(),
            local: false,
        });
    }
    for n in &options.free_local {
        if !human.local_change_labels().contains(n) {
            return Err(unknown_free(n, human));
        }
        if targets
            .iter()
            .any(|target| local_may_affect(n, &target.measure))
        {
            variables.push(Variable {
                name: n.clone(),
                local: true,
            });
        }
    }
    let mut params = start.clone();
    let mut body = human.evaluate(&params)?;
    let mut residual = loss(human, &body, targets)?;
    let mut iterations = 0;
    let mut converged = residual.max_component < 0.05;
    for step in 0..options.max_iterations {
        if converged || variables.is_empty() {
            break;
        }
        iterations = step + 1;
        let (jacobian, current) = jacobian(human, &params, &body, targets, &variables)?;
        // Levenberg-Marquardt: a step that does not help is retried with more damping, from the same Jacobian.
        let mut damping = options.regularisation;
        let mut improved = false;
        for _ in 0..8 {
            let (matrix, rhs) =
                normal_equations(&jacobian, &current, &params, start, &variables, damping);
            let delta = solve(matrix, rhs);
            let mut candidate = params.clone();
            for (i, var) in variables.iter().enumerate() {
                let old = get(&params, var);
                set(
                    &mut candidate,
                    var,
                    (old + delta[i]).clamp(if var.local { -1.0 } else { 0.0 }, 1.0),
                );
            }
            let candidate_body = human.evaluate(&candidate)?;
            let candidate_loss = loss(human, &candidate_body, targets)?;
            if candidate_loss.squared < residual.squared {
                params = candidate;
                body = candidate_body;
                residual = candidate_loss;
                converged = residual.max_component < 0.05;
                improved = true;
                break;
            }
            damping = (damping * 5.0).max(1e-3);
        }
        if !improved {
            break;
        }
    }
    let achieved = targets
        .iter()
        .map(|t| {
            Ok(Achieved {
                measure: t.measure.clone(),
                target: t.value,
                value: measure(human, &body, &t.measure)?,
            })
        })
        .collect::<Result<_, Error>>()?;
    Ok(FitResult {
        params,
        achieved,
        converged,
        iterations,
        residual: residual.squared.sqrt(),
    })
}

fn local_may_affect(name: &str, measure: &Measure) -> bool {
    let topic = match measure {
        Measure::Stature | Measure::Mass | Measure::HeadWidth => return false,
        Measure::ShoulderWidth => "shoulder",
        Measure::ArmLength => "arm",
        Measure::LegLength => "leg",
        Measure::NeckCircumference => "neck",
        Measure::ChestCircumference => "chest",
        Measure::WaistCircumference => "waist",
        Measure::HipCircumference => "hip",
        Measure::BoneDistance(a, b) => {
            if a.contains("arm") || b.contains("arm") {
                "arm"
            } else if a.contains("leg") || b.contains("leg") {
                "leg"
            } else {
                return false;
            }
        }
    };
    name.contains(topic)
        || (topic == "chest" && (name.contains("bust") || name.contains("underbust")))
        || (topic == "hip" && name.contains("waisttohip"))
}

fn unknown_free(name: &str, h: &Human) -> Error {
    Error::InvalidPack(format!(
        "unknown free parameter '{name}'; valid names: {}, {}",
        h.phenotype_labels().join(", "),
        h.local_change_labels().join(", ")
    ))
}
fn get(p: &Params, v: &Variable) -> f64 {
    f64::from(if v.local {
        p.local.get(&v.name).copied().unwrap_or(0.0)
    } else {
        p.phenotype.get(&v.name).copied().unwrap_or(0.5)
    })
}
#[expect(
    clippy::cast_possible_truncation,
    reason = "projected solver controls are bounded to the f32 parameter ranges"
)]
fn set(p: &mut Params, v: &Variable, x: f64) {
    let x = x as f32;
    if v.local {
        p.local.insert(v.name.clone(), x);
    } else {
        p.phenotype.insert(v.name.clone(), x);
    }
}
fn tolerance(m: &Measure) -> f64 {
    if *m == Measure::Mass { 1.0 } else { 0.01 }
}
fn loss(h: &Human, b: &Body, targets: &[Target]) -> Result<Loss, Error> {
    let mut vals = Vec::new();
    let mut squared = 0.0;
    let mut max_component: f64 = 0.0;
    for t in targets {
        let r = (measure(h, b, &t.measure)? - t.value) / tolerance(&t.measure) * t.weight;
        squared += r * r;
        max_component = max_component.max(r.abs());
        vals.push(r);
    }
    Ok(Loss {
        vals,
        squared,
        max_component,
    })
}
struct Loss {
    vals: Vec<f64>,
    squared: f64,
    max_component: f64,
}
fn jacobian(
    h: &Human,
    p: &Params,
    b: &Body,
    targets: &[Target],
    vars: &[Variable],
) -> Result<(Vec<Vec<f64>>, Vec<f64>), Error> {
    let current = loss(h, b, targets)?.vals;
    let mut j = vec![vec![0.0; vars.len()]; targets.len()];
    for (col, v) in vars.iter().enumerate() {
        let old = get(p, v);
        // A step of 0.05 smooths the piecewise section sampling of the circumferences; at the top of the range step down instead.
        let delta = if old + 0.05 <= 1.0 { 0.05 } else { -0.05 };
        let mut changed = p.clone();
        set(&mut changed, v, old + delta);
        let cb = h.evaluate(&changed)?;
        let changed_values = loss(h, &cb, targets)?.vals;
        for row in 0..targets.len() {
            j[row][col] = (changed_values[row] - current[row]) / delta;
        }
    }
    Ok((j, current))
}
fn normal_equations(
    jacobian: &[Vec<f64>],
    residuals: &[f64],
    params: &Params,
    start: &Params,
    vars: &[Variable],
    lambda: f64,
) -> (Vec<Vec<f64>>, Vec<f64>) {
    let size = vars.len();
    let mut matrix = vec![vec![0.0; size]; size];
    let mut rhs = vec![0.0; size];
    for i in 0..size {
        for row in 0..residuals.len() {
            rhs[i] -= jacobian[row][i] * residuals[row];
            for k in 0..size {
                matrix[i][k] += jacobian[row][i] * jacobian[row][k];
            }
        }
        let displacement = get(params, &vars[i]) - get(start, &vars[i]);
        matrix[i][i] += lambda;
        rhs[i] -= lambda * displacement;
    }
    (matrix, rhs)
}
fn solve(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Vec<f64> {
    let n = b.len();
    for i in 0..n {
        let pivot = a[i][i];
        if pivot.abs() < 1e-12 {
            continue;
        }
        for value in a[i].iter_mut().skip(i) {
            *value /= pivot;
        }
        b[i] /= pivot;
        let pivot_row = a[i].clone();
        for row in 0..n {
            if row != i {
                let factor = a[row][i];
                for (value, pivot_value) in a[row].iter_mut().skip(i).zip(pivot_row.iter().skip(i))
                {
                    *value -= factor * pivot_value;
                }
                b[row] -= factor * b[i];
            }
        }
    }
    b
}

#[cfg(test)]
mod tests {
    use super::solve;
    #[test]
    fn gaussian_solver_handles_small_linear_system() {
        let x = solve(vec![vec![3.0, 2.0], vec![1.0, 2.0]], vec![7.0, 5.0]);
        assert!((x[0] - 1.0).abs() < 1e-10);
        assert!((x[1] - 2.0).abs() < 1e-10);
    }
}
