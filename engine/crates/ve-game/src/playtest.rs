//! Seeded, headless policies for running a game definition. Seek target tags are comma-separated priorities.
use crate::{GameDef, Inputs, Status, World};
use std::collections::BTreeMap;

/// Policy used to supply input samples to a headless game.
#[derive(Clone, Debug, PartialEq)]
pub enum Policy {
    /// Supply no input.
    Idle,
    /// Supply seeded random axes and press declared buttons.
    Random,
    /// Steer the first hero toward the nearest entity of the first available target tag.
    Seek {
        /// Tag identifying the entity to steer.
        hero: String,
        /// Comma-separated target tags in priority order.
        target: String,
    },
}

/// Summary of a headless game run.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    /// Final game status.
    pub status: Status,
    /// Number of completed fixed steps.
    pub steps: u32,
    /// Final game variables.
    pub vars: BTreeMap<String, f64>,
    /// Number of action-list runs per rule, in definition order.
    pub rule_fires: Vec<u64>,
    /// Rule indices whose action lists never ran.
    pub never_fired: Vec<usize>,
    /// Deterministic world hash after the last completed step.
    pub hash: u64,
}

/// Run a game until it ends or reaches `max_steps`.
#[must_use]
pub fn run(def: &GameDef, seed: u64, policy: &Policy, max_steps: u32) -> Report {
    let mut world = World::new(def, seed);
    let mut random = seed ^ 0x9e37_79b9_7f4a_7c15;
    let mut steps = 0;
    // A game whose hero follows the viewer is played by walking the viewer, the way a person in a headset would.
    let follows = def.inputs.iter().any(|input| input.follow);
    let mut walker = world.viewer_start();
    let mut heading = 0;
    while steps < max_steps && world.status() == Status::Playing {
        let mut input = match policy {
            Policy::Idle => Inputs::default(),
            Policy::Random => random_inputs(def, &mut random),
            Policy::Seek { hero, target } => seek_inputs(def, &world, hero, target),
        };
        if follows && let Some(position) = walker.as_mut() {
            match policy {
                Policy::Idle => {}
                Policy::Random => {
                    if steps % RANDOM_TURN_STEPS == 0 {
                        heading = usize::try_from(next_bits(&mut random) % 8).unwrap_or(0);
                    }
                    let direction = COMPASS[heading];
                    position[0] += direction[0] * WALK_STEP;
                    position[2] += direction[1] * WALK_STEP;
                    input.viewer = Some(*position);
                }
                Policy::Seek { target, .. } => {
                    walk_toward_nearest(def, &world, target, position);
                    input.viewer = Some(*position);
                }
            }
        }
        if world.step(&input).is_err() {
            break;
        }
        steps += 1;
    }
    let rule_fires = world.rule_fires().to_vec();
    let never_fired = rule_fires
        .iter()
        .enumerate()
        .filter_map(|(index, fires)| (*fires == 0).then_some(index))
        .collect();
    Report {
        status: world.status(),
        steps,
        vars: world.vars().clone(),
        rule_fires,
        never_fired,
        hash: world.hash(),
    }
}

/// A walking pace of 3 m/s at 60 steps a second.
const WALK_STEP: f64 = 3.0 / 60.0;
/// A random walker changes direction every three quarters of a second.
const RANDOM_TURN_STEPS: u32 = 45;
/// Eight compass directions on the ground (x, z), with no trigonometry so native and WASM agree to the bit.
const COMPASS: [[f64; 2]; 8] = [
    [1.0, 0.0],
    [0.707_106_781_186_547_5, 0.707_106_781_186_547_5],
    [0.0, 1.0],
    [-0.707_106_781_186_547_5, 0.707_106_781_186_547_5],
    [-1.0, 0.0],
    [-0.707_106_781_186_547_5, -0.707_106_781_186_547_5],
    [0.0, -1.0],
    [0.707_106_781_186_547_5, -0.707_106_781_186_547_5],
];

/// Moves `position` one walking step toward the nearest entity with the target tag (stopping on it); stays put when there is none.
fn walk_toward_nearest(def: &GameDef, world: &World, target_tag: &str, position: &mut [f64; 3]) {
    let Some(target) = nearest_tagged(def, world, target_tag, position) else {
        return;
    };
    let delta: [f64; 3] = std::array::from_fn(|axis| target.position[axis] - position[axis]);
    let length = delta.iter().map(|value| value * value).sum::<f64>().sqrt();
    if length <= WALK_STEP {
        *position = target.position;
    } else {
        for axis in 0..3 {
            position[axis] += delta[axis] / length * WALK_STEP;
        }
    }
}

fn random_inputs(def: &GameDef, state: &mut u64) -> Inputs {
    let mut inputs = Inputs::default();
    for declaration in &def.inputs {
        for axis in &declaration.axes {
            inputs.axes.insert(
                format!("{}.{}", declaration.name, axis),
                next_random(state) * 2.0 - 1.0,
            );
        }
        if declaration.key {
            inputs.pressed.push(declaration.name.clone());
        }
    }
    inputs
}

fn next_bits(state: &mut u64) -> u32 {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    u32::try_from(*state >> 32).unwrap_or_default()
}

fn next_random(state: &mut u64) -> f64 {
    f64::from(next_bits(state)) / f64::from(u32::MAX)
}

fn seek_inputs(def: &GameDef, world: &World, hero_tag: &str, target_tag: &str) -> Inputs {
    let Some(declaration) = def.inputs.iter().find(|input| !input.axes.is_empty()) else {
        return Inputs::default();
    };
    let tagged = |entity: &&crate::Entity, tag: &str| {
        def.kinds
            .iter()
            .find(|kind| kind.name == entity.kind)
            .is_some_and(|kind| kind.tags.iter().any(|value| value == tag))
    };
    let hero = world
        .entities()
        .iter()
        .find(|entity| tagged(entity, hero_tag));
    let target = hero.and_then(|hero| nearest_tagged(def, world, target_tag, &hero.position));
    let (Some(hero), Some(target)) = (hero, target) else {
        return Inputs::default();
    };
    let delta = std::array::from_fn::<_, 3, _>(|axis| target.position[axis] - hero.position[axis]);
    let len = delta
        .iter()
        .map(|value| value * value)
        .sum::<f64>()
        .sqrt()
        .max(1e-9);
    let mut inputs = Inputs::default();
    for axis in &declaration.axes {
        let index = match axis.as_str() {
            "x" => 0,
            "y" => 1,
            "z" => 2,
            _ => continue,
        };
        inputs
            .axes
            .insert(format!("{}.{}", declaration.name, axis), delta[index] / len);
    }
    inputs
}

fn nearest_tagged<'a>(
    def: &GameDef,
    world: &'a World,
    target_tags: &str,
    position: &[f64; 3],
) -> Option<&'a crate::Entity> {
    target_tags
        .split(',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .find_map(|tag| {
            world
                .entities()
                .iter()
                .filter(|entity| {
                    def.kinds
                        .iter()
                        .find(|kind| kind.name == entity.kind)
                        .is_some_and(|kind| kind.tags.iter().any(|value| value == tag))
                })
                .min_by(|a, b| {
                    distance_squared(position, &a.position)
                        .total_cmp(&distance_squared(position, &b.position))
                })
        })
}

fn distance_squared(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    (0..3).map(|axis| (a[axis] - b[axis]).powi(2)).sum()
}
