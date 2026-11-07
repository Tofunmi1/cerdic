//! Commands a host gives the game from outside its rules: put an entity of a declared kind into the world, move it, change one of its
//! numbers, take it away. A script that plays the pieces of a turn-based game uses these; the rules and the step go on as before.
use super::{MAX_ENTITIES, World};
use crate::GameError;

/// Most characters of a property name (the names are the script's own).
const MAX_PROP_NAME: usize = 64;

impl World {
    /// Spawns an entity of a declared kind and returns its id. Ids are never reused.
    ///
    /// # Errors
    /// The kind is not declared (the message lists the kinds that are), the position or heading is not finite, or the entity budget is full.
    pub fn command_spawn(
        &mut self,
        kind: &str,
        position: [f64; 3],
        heading: f64,
    ) -> Result<u32, GameError> {
        finite(&position, heading)?;
        if self.entities.len() >= MAX_ENTITIES {
            return Err(GameError::EntityBudget(MAX_ENTITIES));
        }
        let Some(declared) = self.def.kinds.iter().find(|declared| declared.name == kind) else {
            let known: Vec<&str> = self
                .def
                .kinds
                .iter()
                .map(|declared| declared.name.as_str())
                .collect();
            return Err(GameError::InvalidInput(format!(
                "no kind named {kind:?}; declared kinds are {}",
                known.join(", ")
            )));
        };
        let props = declared.props.clone();
        let id = self.next_id;
        self.spawn(kind, position, props, heading);
        Ok(id)
    }

    /// Puts an entity at a position, and facing `heading` (yaw in radians about +Y, 0 faces +Z) when one is given; otherwise it keeps the
    /// way it faces until it moves. Its velocity is cleared, so what the rules integrate starts from rest.
    ///
    /// # Errors
    /// No entity has this id, or the position or heading is not finite.
    pub fn command_place(
        &mut self,
        id: u32,
        position: [f64; 3],
        heading: Option<f64>,
    ) -> Result<(), GameError> {
        finite(&position, heading.unwrap_or(0.0))?;
        let entity = self.entity_mut(id)?;
        entity.position = position;
        entity.velocity = [0.0; 3];
        if let Some(yaw) = heading {
            entity.heading = yaw;
            entity.orientation = [0.0, (yaw * 0.5).sin(), 0.0, (yaw * 0.5).cos()];
        }
        Ok(())
    }

    /// Sets one numeric property of an entity.
    ///
    /// # Errors
    /// No entity has this id, the name is empty or too long, or the value is not finite.
    pub fn command_set_prop(&mut self, id: u32, name: &str, value: f64) -> Result<(), GameError> {
        if name.is_empty() || name.len() > MAX_PROP_NAME {
            return Err(GameError::InvalidInput(format!(
                "a property name is 1 to {MAX_PROP_NAME} characters, got {}",
                name.len()
            )));
        }
        if !value.is_finite() {
            return Err(GameError::InvalidInput(format!(
                "property {name:?} must be a finite number"
            )));
        }
        self.entity_mut(id)?.props.insert(name.to_owned(), value);
        Ok(())
    }

    /// Removes an entity and whatever the game kept about it.
    ///
    /// # Errors
    /// No entity has this id.
    pub fn command_remove(&mut self, id: u32) -> Result<(), GameError> {
        self.entity_mut(id)?;
        self.entities.retain(|entity| entity.id != id);
        self.agents.remove(&id);
        Ok(())
    }

    fn entity_mut(&mut self, id: u32) -> Result<&mut super::Entity, GameError> {
        self.entities
            .iter_mut()
            .find(|entity| entity.id == id)
            .ok_or_else(|| GameError::InvalidInput(format!("no entity has id {id}")))
    }
}

fn finite(position: &[f64; 3], heading: f64) -> Result<(), GameError> {
    if position.iter().all(|value| value.is_finite()) && heading.is_finite() {
        Ok(())
    } else {
        Err(GameError::InvalidInput(
            "a position and a heading are finite numbers".to_owned(),
        ))
    }
}
