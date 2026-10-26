//! `VeMaterial` is reachable by name through reflection (ADR 0065): a script or a model can read a part's surface and change its colour,
//! roughness, glow or finish in the running world. Written before the body; do not weaken.
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use std::any::TypeId;

use bevy_color::Color;
use bevy_ecs::{reflect::ReflectComponent, world::World};
use bevy_reflect::{
    TypeRegistry,
    serde::{TypedReflectDeserializer, TypedReflectSerializer},
};
use serde::de::DeserializeSeed;
use ve_render::{FoliageStyle, ProceduralRecipe, SurfaceFinish, ToonStyle, VeMaterial};

fn registry() -> TypeRegistry {
    let mut registry = TypeRegistry::default();
    registry.register::<VeMaterial>();
    registry
}

fn to_json(registry: &TypeRegistry, material: &VeMaterial) -> serde_json::Value {
    let registration = registry.get(TypeId::of::<VeMaterial>()).unwrap();
    let component = registration.data::<ReflectComponent>().unwrap();
    let mut world = World::new();
    let entity = world.spawn(*material).id();
    let value = component.reflect(world.entity(entity)).unwrap();
    serde_json::to_value(TypedReflectSerializer::new(value, registry)).unwrap()
}

fn from_json(registry: &TypeRegistry, json: &serde_json::Value) -> Result<VeMaterial, String> {
    let registration = registry.get(TypeId::of::<VeMaterial>()).unwrap();
    let component = registration.data::<ReflectComponent>().unwrap().clone();
    let text = json.to_string();
    let mut deserializer = serde_json::Deserializer::from_str(&text);
    let value = TypedReflectDeserializer::new(registration, registry)
        .deserialize(&mut deserializer)
        .map_err(|e| e.to_string())?;
    let mut world = World::new();
    let entity = world.spawn(VeMaterial::default()).id();
    component.apply(&mut world.entity_mut(entity), &*value);
    Ok(*world.entity(entity).get::<VeMaterial>().unwrap())
}

fn painted() -> VeMaterial {
    VeMaterial {
        base_color: Color::srgb(0.1, 0.4, 0.9),
        opacity: 0.75,
        sort_key: 42,
        metallic: 1.0,
        roughness: 0.2,
        emissive: Color::srgb(1.0, 0.1, 0.0),
        recipe: ProceduralRecipe::Marble,
        texture_scale: 3.0,
        map_layers: [4, -1, 7, 2],
        toon: ToonStyle {
            bands: 2,
            ..ToonStyle::default()
        },
        foliage: FoliageStyle {
            alpha_cutoff: 0.5,
            sway: 0.4,
            flutter: 0.1,
            ..FoliageStyle::plain()
        },
        finish: SurfaceFinish {
            clearcoat: 0.5,
            wetness: 0.8,
            ..SurfaceFinish::default()
        },
        ..VeMaterial::default()
    }
}

#[test]
fn the_material_is_a_registered_component_and_its_parts_come_with_it() {
    let registry = registry();
    let registration = registry
        .get(TypeId::of::<VeMaterial>())
        .expect("VeMaterial is registered");
    assert!(
        registration.data::<ReflectComponent>().is_some(),
        "it is a reflected component"
    );
    for (name, id) in [
        ("ToonStyle", TypeId::of::<ToonStyle>()),
        ("FoliageStyle", TypeId::of::<FoliageStyle>()),
        ("SurfaceFinish", TypeId::of::<SurfaceFinish>()),
        ("ProceduralRecipe", TypeId::of::<ProceduralRecipe>()),
    ] {
        assert!(
            registry.get(id).is_some(),
            "{name} is registered along with it"
        );
    }
}

#[test]
fn its_json_names_what_a_model_would_expect() {
    let json = to_json(&registry(), &painted());
    for key in [
        "base_color",
        "opacity",
        "metallic",
        "roughness",
        "emissive",
        "recipe",
        "texture_scale",
        "toon",
        "foliage",
        "finish",
    ] {
        assert!(json.get(key).is_some(), "no `{key}` in {json}");
    }
    assert!((json["roughness"].as_f64().unwrap() - 0.2).abs() < 1e-6);
    assert!((json["metallic"].as_f64().unwrap() - 1.0).abs() < 1e-6);
    assert!((json["finish"]["clearcoat"].as_f64().unwrap() - 0.5).abs() < 1e-6);
    assert_eq!(json["toon"]["bands"], 2);
    assert_eq!(
        json["recipe"], "Marble",
        "an enum is written by its variant name"
    );
}

#[test]
fn a_material_survives_its_own_json() {
    let registry = registry();
    let material = painted();
    let back = from_json(&registry, &to_json(&registry, &material)).unwrap();
    assert_eq!(back, material);
    let plain = VeMaterial::default();
    assert_eq!(
        from_json(&registry, &to_json(&registry, &plain)).unwrap(),
        plain
    );
}

#[test]
fn the_colours_can_be_changed_through_json() {
    let registry = registry();
    let mut json = to_json(&registry, &VeMaterial::default());
    json["roughness"] = serde_json::json!(0.05);
    json["metallic"] = serde_json::json!(0.9);
    json["finish"]["wetness"] = serde_json::json!(1.0);
    let changed = from_json(&registry, &json).unwrap();
    assert!((changed.roughness - 0.05).abs() < 1e-6);
    assert!((changed.metallic - 0.9).abs() < 1e-6);
    assert!((changed.finish.wetness - 1.0).abs() < 1e-6);
    assert_eq!(
        changed.base_color,
        VeMaterial::default().base_color,
        "what was not touched is unchanged"
    );
}

#[test]
fn a_value_of_the_wrong_type_is_refused() {
    let registry = registry();
    let mut json = to_json(&registry, &VeMaterial::default());
    json["roughness"] = serde_json::json!("very");
    assert!(from_json(&registry, &json).is_err());
    let mut json = to_json(&registry, &VeMaterial::default());
    json["recipe"] = serde_json::json!("Gold");
    assert!(from_json(&registry, &json).is_err(), "an unknown variant");
}

#[test]
fn the_terrain_and_water_options_survive() {
    // `terrain` and `water` are optional layers of the finish: None for an ordinary surface, and the type must still round trip.
    let registry = registry();
    let json = to_json(&registry, &VeMaterial::default());
    assert!(
        json["finish"].get("terrain").is_some() && json["finish"].get("water").is_some(),
        "{json}"
    );
    assert_eq!(from_json(&registry, &json).unwrap().finish.terrain, None);
}

#[test]
fn water_wave_controls_round_trip_and_old_materials_default_them() {
    let registry = registry();
    let material = VeMaterial {
        finish: SurfaceFinish {
            water: Some(ve_render::SurfaceWater {
                seed: 41,
                direction: -135.0,
                ripple: 1.7,
                ..ve_render::SurfaceWater::default()
            }),
            ..SurfaceFinish::default()
        },
        ..VeMaterial::default()
    };
    let mut json = to_json(&registry, &material);
    assert_eq!(from_json(&registry, &json).unwrap(), material);
    let water = json["finish"]["water"].as_object_mut().unwrap();
    water.remove("seed");
    water.remove("direction");
    let back = from_json(&registry, &json).unwrap().finish.water.unwrap();
    assert_eq!((back.seed, back.direction, back.ripple), (0, 0.0, 1.7));
}

#[test]
fn grass_controls_reflect_and_old_foliage_uses_defaults() {
    let registry = registry();
    let mut material = painted();
    material.foliage = FoliageStyle {
        grass: true,
        ground_color: [0.3, 0.2, 0.1],
        ground_blend: 0.6,
        fade_near: 25.0,
        fade_far: 70.0,
        translucency: 0.5,
        exposure_boost: 1.2,
        ..material.foliage
    };
    let mut json = to_json(&registry, &material);
    assert_eq!(from_json(&registry, &json).unwrap(), material);
    let foliage = json["foliage"].as_object_mut().unwrap();
    for key in [
        "grass",
        "ground_color",
        "ground_blend",
        "fade_near",
        "fade_far",
        "translucency",
        "exposure_boost",
    ] {
        foliage.remove(key);
    }
    let old = from_json(&registry, &json).unwrap();
    assert!(!old.foliage.grass);
    assert_eq!(old.foliage.exposure_boost, 1.0);
    assert_eq!(old.foliage.sway, material.foliage.sway);
}
