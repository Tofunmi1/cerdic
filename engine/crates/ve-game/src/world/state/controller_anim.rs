//! What a controlled hero tells the animation machine, so a walk cycle needs no rules: the number `speed` (metres per second, to a tenth),
//! the flags `grounded` and `jumping` (rising in the air), and the triggers `jump` (leaves the ground going up) and `land`. Each is sent
//! only when it changes, aimed at the entity, so a model with a machine reaches its own copy.
use super::{Entity, World};
use crate::{Effect, EffectKind};

/// The last value sent for each, kept in the entity's props so a save and a replay agree. Before anything was sent they are absent.
const SENT_SPEED: &str = "ctl_a_speed";
const SENT_GROUNDED: &str = "ctl_a_grounded";
const SENT_JUMPING: &str = "ctl_a_jumping";

fn flag(on: bool) -> f64 {
    if on { 1.0 } else { 0.0 }
}

fn last(entity: &Entity, key: &str) -> Option<f64> {
    entity.props.get(key).copied()
}

impl World {
    /// Appends the animation updates of this step for every entity of a controlled kind.
    pub(super) fn controller_animation(&mut self, effects: &mut Vec<Effect>) {
        if self.def.controllers.is_empty() {
            return;
        }
        let controlled: Vec<String> = self.def.controllers.iter().map(|c| c.kind.clone()).collect();
        for entity in self.entities.iter_mut().filter(|entity| controlled.contains(&entity.kind)) {
            // the model is drawn from the orientation, so it turns with the heading (the controller or the motion sets the heading)
            let half = entity.heading / 2.0;
            entity.orientation = [0.0, half.sin(), 0.0, half.cos()];
            let target = Some(entity.id);
            let emit = |effects: &mut Vec<Effect>, kind: EffectKind, name: &str, value: Option<f64>| effects.push(Effect { kind, name: name.into(), target, value });
            let speed = (entity.props.get("speed").copied().unwrap_or(0.0) * 10.0).round() / 10.0;
            let grounded = entity.props.get("grounded").copied().unwrap_or(0.0) > 0.5;
            let jumping = !grounded && entity.velocity[1] > 0.0;
            if last(entity, SENT_SPEED).is_none_or(|before| (before - speed).abs() > 1e-9) {
                emit(effects, EffectKind::AnimSet, "speed", Some(speed));
            }
            let was_grounded = last(entity, SENT_GROUNDED).map(|before| before > 0.5);
            if was_grounded != Some(grounded) {
                emit(effects, EffectKind::AnimFlag, "grounded", Some(flag(grounded)));
                if was_grounded == Some(true) && jumping {
                    emit(effects, EffectKind::Anim, "jump", None);
                }
                if was_grounded == Some(false) && grounded {
                    emit(effects, EffectKind::Anim, "land", None);
                }
            }
            if last(entity, SENT_JUMPING).is_none_or(|before| (before > 0.5) != jumping) {
                emit(effects, EffectKind::AnimFlag, "jumping", Some(flag(jumping)));
            }
            entity.props.insert(SENT_SPEED.into(), speed);
            entity.props.insert(SENT_GROUNDED.into(), flag(grounded));
            entity.props.insert(SENT_JUMPING.into(), flag(jumping));
        }
    }
}
