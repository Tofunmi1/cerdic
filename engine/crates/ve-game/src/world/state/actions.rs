//! Applying game actions to world state.
use super::{Event, MAX_ENTITIES, World};
use crate::expr::Expr;
use crate::{Effect, GameError, Status};

fn seconds_to_steps(seconds: f64) -> u64 {
    (seconds * 60.0)
        .round()
        .max(1.0)
        .to_string()
        .parse()
        .unwrap_or(1)
}

impl World {
    pub(super) fn action(
        &mut self,
        action: &str,
        event: &Event,
        queue: &mut Vec<Event>,
        effects: &mut Vec<Effect>,
    ) -> Result<(), GameError> {
        if self.flow_action(action, event, queue) || self.combat_action(action, event, queue) || self.party_action(action, event, queue) {
            return Ok(());
        }
        if let Some(body) = action.strip_prefix("add:") {
            let mut fields = body.splitn(2, ':');
            if let (Some(name), Some(amount)) = (fields.next(), fields.next()) {
                let value = amount.parse::<f64>().unwrap_or(0.0);
                *self.vars.entry(name.into()).or_default() += value;
                queue.push(Event {
                    name: format!("var:{name}"),
                    a: event.a,
                    b: event.b,
                    depth: event.depth + 1,
                });
            }
        } else if let Some(body) = action.strip_prefix("set:") {
            let mut fields = body.splitn(2, ':');
            if let (Some(name), Some(value)) = (fields.next(), fields.next()) {
                let old = self.vars.get(name).copied().unwrap_or(0.0);
                self.vars
                    .insert(name.into(), value.parse::<f64>().unwrap_or(0.0));
                if old.to_bits() != self.vars[name].to_bits() {
                    queue.push(Event {
                        name: format!("var:{name}"),
                        a: event.a,
                        b: event.b,
                        depth: event.depth + 1,
                    });
                }
            }
        } else if let Some(body) = action.strip_prefix("emit:") {
            queue.push(Event {
                name: format!("emit:{body}"),
                a: event.a,
                b: event.b,
                depth: event.depth + 1,
            });
        } else if action == "win" {
            self.status = Status::Won;
        } else if action == "lose" {
            self.status = Status::Lost;
        } else if let Some(body) = action.strip_prefix("timer:") {
            let mut fields = body.splitn(2, ':');
            if let (Some(name), Some(seconds)) = (fields.next(), fields.next()) {
                let steps = seconds.parse::<f64>().map_or(1, seconds_to_steps);
                self.timers.insert((self.tick + steps, name.into()), true);
            }
        } else if let Some(body) = action.strip_prefix("spawn:") {
            self.spawn_action(body)?;
        } else if let Some(body) = action.strip_prefix("destroy:") {
            let ids = self.select_ids(body, event);
            for id in &ids {
                queue.push(Event {
                    name: "destroyed".into(),
                    a: Some(*id),
                    b: None,
                    depth: event.depth + 1,
                });
            }
            self.entities.retain(|entity| !ids.contains(&entity.id));
            self.agents.retain(|id, _| !ids.contains(id));
        } else if let Some(body) = action.strip_prefix("move:") {
            self.move_action(body, event);
        } else if let Some(body) = action.strip_prefix("setvel:") {
            self.velocity_action(body, event);
        } else if let Some(body) = action.strip_prefix("stun:") {
            self.stun_action(body, event, queue);
        } else if let Some(body) = action.strip_prefix("setprop:") {
            self.property_action(body, event);
        } else if let Some(body) = action.strip_prefix("effect:") {
            effects.extend(self.effects(body, event));
        }
        Ok(())
    }

    fn spawn_action(&mut self, body: &str) -> Result<(), GameError> {
        if self.entities.len() >= MAX_ENTITIES {
            return Err(GameError::EntityBudget(MAX_ENTITIES));
        }
        if let Some((kind_name, position)) = body.split_once('@') {
            let values: Vec<f64> = position
                .split(',')
                .filter_map(|value| value.parse().ok())
                .collect();
            if values.len() == 3
                && let Some(kind) = self.def.kinds.iter().find(|kind| kind.name == kind_name)
            {
                self.spawn(
                    kind_name,
                    [values[0], values[1], values[2]],
                    kind.props.clone(),
                    0.0,
                );
            }
        }
        Ok(())
    }

    fn move_action(&mut self, body: &str, event: &Event) {
        // The vector follows the last colon, so `kind:door` and `tag:gate` work as selectors.
        if let Some((selector, vector)) = body.rsplit_once(':') {
            let values: Vec<f64> = vector
                .split(',')
                .filter_map(|value| value.parse().ok())
                .collect();
            if values.len() == 3 {
                for id in self.select_ids(selector, event) {
                    if let Some(entity) = self.entities.iter_mut().find(|entity| entity.id == id) {
                        for (axis, value) in values.iter().enumerate() {
                            entity.position[axis] += value;
                        }
                    }
                }
            }
        }
    }

    fn velocity_action(&mut self, body: &str, event: &Event) {
        if let Some((selector, vector)) = body.rsplit_once(':') {
            let values: Vec<f64> = vector
                .split(',')
                .filter_map(|value| value.parse().ok())
                .collect();
            if values.len() == 3 {
                for id in self.select_ids(selector, event) {
                    if let Some(entity) = self.entities.iter_mut().find(|entity| entity.id == id) {
                        entity.velocity = [values[0], values[1], values[2]];
                    }
                }
            }
        }
    }

    fn property_action(&mut self, body: &str, event: &Event) {
        // SELECTOR:NAME:VALUE, read from the right because a selector such as tag:hero has a colon of its own
        let Some((rest, value)) = body.rsplit_once(':') else {
            return;
        };
        let Some((selector, name)) = rest.rsplit_once(':') else {
            return;
        };
        let Ok(value) = value.parse::<f64>() else {
            return;
        };
        for id in self.select_ids(selector, event) {
            if let Some(entity) = self.entities.iter_mut().find(|entity| entity.id == id) {
                entity.props.insert(name.into(), value);
            }
        }
    }

    pub(super) fn select_ids(&self, selector: &str, event: &Event) -> Vec<u32> {
        if selector == "a" {
            return event.a.into_iter().collect();
        }
        if selector == "b" {
            return event.b.into_iter().collect();
        }
        if selector == "other" {
            return event.b.or(event.a).into_iter().collect();
        }
        self.entities
            .iter()
            .filter(|entity| self.matches(selector, entity))
            .map(|entity| entity.id)
            .collect()
    }

    fn effects(&self, body: &str, event: &Event) -> Vec<Effect> {
        if body.starts_with("anim_set:") || body.starts_with("anim_flag:") {
            let (spec, selector) = body.rsplit_once('@').unwrap_or((body, ""));
            let Some((kind_name, remainder)) = spec.split_once(':') else {
                return Vec::new();
            };
            let Some((name, expression)) = remainder.split_once(':') else {
                return Vec::new();
            };
            let Ok(expression) = Expr::parse(expression) else {
                return Vec::new();
            };
            let kind = if kind_name == "anim_set" {
                crate::EffectKind::AnimSet
            } else {
                crate::EffectKind::AnimFlag
            };
            let targets = if selector.is_empty() {
                vec![None]
            } else {
                self.select_ids(selector, event)
                    .into_iter()
                    .map(Some)
                    .collect()
            };
            return targets
                .into_iter()
                .map(|target| {
                    let parameter_value = expression.eval(&|variable| {
                        if variable == "speed" || variable == "vy" {
                            if let Some(id) = target {
                                if let Some(entity) =
                                    self.entities.iter().find(|entity| entity.id == id)
                                {
                                    return if variable == "speed" {
                                        (entity.velocity[0] * entity.velocity[0]
                                            + entity.velocity[2] * entity.velocity[2])
                                            .sqrt()
                                    } else {
                                        entity.velocity[1]
                                    };
                                }
                            }
                        }
                        self.lookup(variable, target)
                    });
                    Effect {
                        kind: kind.clone(),
                        name: name.into(),
                        target,
                        value: Some(if kind == crate::EffectKind::AnimFlag {
                            f64::from(parameter_value != 0.0)
                        } else {
                            parameter_value
                        }),
                    }
                })
                .collect();
        }
        let Some((kind_name, target)) = body.split_once(':') else {
            return Vec::new();
        };
        if kind_name == "camera" {
            // `camera:NAME` cuts to a camera, `camera:NAME:SECONDS` blends to it
            let (name, seconds) = target.rsplit_once(':').map_or((target, 0.0), |(name, seconds)| (name, seconds.parse().unwrap_or(0.0)));
            return vec![Effect { kind: crate::EffectKind::Camera, name: name.into(), target: None, value: Some(seconds) }];
        }
        let (name, selector) = target.split_once('@').unwrap_or((target, ""));
        let kind = match kind_name {
            "sfx" => crate::EffectKind::Sfx,
            "particles" => crate::EffectKind::Particles,
            "camera_shake" => crate::EffectKind::CameraShake,
            "message" => crate::EffectKind::Message,
            "anim" => crate::EffectKind::Anim,
            _ => return Vec::new(),
        };
        if kind == crate::EffectKind::Anim {
            let targets = if selector.is_empty() {
                vec![None]
            } else {
                self.select_ids(selector, event)
                    .into_iter()
                    .map(Some)
                    .collect()
            };
            return targets
                .into_iter()
                .map(|target| Effect {
                    kind: kind.clone(),
                    name: name.into(),
                    target,
                    value: None,
                })
                .collect();
        }
        let target = match selector {
            "a" => event.a,
            "b" | "other" => event.b.or(event.a),
            "" => None,
            _ => selector
                .parse()
                .ok()
                .or_else(|| self.select_ids(selector, event).first().copied()),
        };
        vec![Effect {
            kind,
            name: name.into(),
            target,
            value: None,
        }]
    }
}
