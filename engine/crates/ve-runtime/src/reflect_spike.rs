//! Spike (2026-10-25): how fast can the host read and write components through Bevy's reflection, in the browser's wasm?
//!
//! The runtime API for models and scripts (the programmable core) would reach every component by name through `bevy_reflect`. Whether that
//! is affordable for a script that touches a thousand entities a frame is the open question, so this module builds its own small `World`
//! of N entities with a `Transform` each and times three things from the page (`performance.now()` around each call): reading every
//! component to JSON, writing every component from JSON, and the same writes done directly with the typed component (the floor).
//! Nothing here touches the running app's world.
use std::any::TypeId;
use std::cell::RefCell;

use bevy_ecs::{
    entity::Entity,
    reflect::{AppTypeRegistry, ReflectComponent},
    world::World,
};
use bevy_math::Vec3;
use bevy_reflect::{
    PartialReflect,
    serde::{TypedReflectDeserializer, TypedReflectSerializer},
};
use bevy_transform::components::Transform;
use serde::de::DeserializeSeed;
use wasm_bindgen::prelude::wasm_bindgen;

struct Spike {
    world: World,
    entities: Vec<Entity>,
}

thread_local! {
    static SPIKE: RefCell<Option<Spike>> = const { RefCell::new(None) };
}

/// Builds the spike world with `entities` entities (each a `Transform`); returns how many exist.
#[wasm_bindgen]
pub fn reflect_spike_setup(entities: u32) -> u32 {
    let mut world = World::new();
    let registry = AppTypeRegistry::default();
    registry.write().register::<Transform>();
    world.insert_resource(registry);
    let list: Vec<Entity> = (0..entities)
        .map(|i| {
            #[expect(clippy::cast_precision_loss, reason = "a spike's small index")]
            let at = Vec3::new(i as f32, 0.5, -(i as f32));
            world.spawn(Transform::from_translation(at)).id()
        })
        .collect();
    let count = u32::try_from(list.len()).unwrap_or(u32::MAX);
    SPIKE.with(|cell| {
        *cell.borrow_mut() = Some(Spike {
            world,
            entities: list,
        })
    });
    count
}

/// Reads every entity's `Transform` through reflection into JSON text; returns the total number of bytes written.
#[wasm_bindgen]
pub fn reflect_spike_get() -> u32 {
    SPIKE.with(|cell| {
        let guard = cell.borrow();
        let Some(spike) = guard.as_ref() else {
            return 0;
        };
        let app = spike.world.resource::<AppTypeRegistry>().clone();
        let registry = app.read();
        let Some(component) = registry.get_type_data::<ReflectComponent>(TypeId::of::<Transform>())
        else {
            return 0;
        };
        let mut bytes = 0_usize;
        for entity in &spike.entities {
            let Some(value) = component.reflect(spike.world.entity(*entity)) else {
                continue;
            };
            if let Ok(text) = serde_json::to_string(&TypedReflectSerializer::new(value, &registry))
            {
                bytes += text.len();
            }
        }
        u32::try_from(bytes).unwrap_or(u32::MAX)
    })
}

/// Writes every entity's `Transform` from JSON text through reflection (parse, then apply); returns how many were written.
#[wasm_bindgen]
pub fn reflect_spike_set() -> u32 {
    SPIKE.with(|cell| {
        let mut guard = cell.borrow_mut();
        let Some(spike) = guard.as_mut() else { return 0 };
        let app = spike.world.resource::<AppTypeRegistry>().clone();
        let registry = app.read();
        let Some(component) = registry.get_type_data::<ReflectComponent>(TypeId::of::<Transform>()).cloned() else { return 0 };
        let Some(registration) = registry.get(TypeId::of::<Transform>()) else { return 0 };
        let mut written = 0_u32;
        for (index, entity) in spike.entities.clone().iter().enumerate() {
            let text = format!(
                r#"{{"translation":[{index}.0,1.5,2.5],"rotation":[0.0,0.0,0.0,1.0],"scale":[1.0,1.0,1.0]}}"#
            );
            let mut json = serde_json::Deserializer::from_str(&text);
            let Ok(value) = TypedReflectDeserializer::new(registration, &registry).deserialize(&mut json) else { continue };
            let value: Box<dyn PartialReflect> = value;
            let mut entity_mut = spike.world.entity_mut(*entity);
            component.apply(&mut entity_mut, &*value);
            written += 1;
        }
        written
    })
}

/// The floor: the same writes done directly on the typed component, no reflection and no JSON.
#[wasm_bindgen]
pub fn reflect_spike_direct() -> u32 {
    SPIKE.with(|cell| {
        let mut guard = cell.borrow_mut();
        let Some(spike) = guard.as_mut() else {
            return 0;
        };
        let mut written = 0_u32;
        for (index, entity) in spike.entities.clone().iter().enumerate() {
            if let Some(mut transform) = spike.world.get_mut::<Transform>(*entity) {
                #[expect(clippy::cast_precision_loss, reason = "a spike's small index")]
                let at = Vec3::new(index as f32, 1.5, 2.5);
                transform.translation = at;
                written += 1;
            }
        }
        written
    })
}
