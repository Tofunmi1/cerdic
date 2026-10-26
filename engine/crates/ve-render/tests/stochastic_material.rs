//! Sampling controls survive serde and reflection and default safely for old materials.
use bevy_reflect::{PartialReflect, ReflectRef};
use ve_render::VeMaterial;

#[test]
fn sampling_controls_are_reflected_and_old_json_defaults_off()
-> Result<(), Box<dyn std::error::Error>> {
    let material = VeMaterial {
        no_repeat: true,
        no_repeat_scale: 3.0,
        ..VeMaterial::default()
    };
    let ReflectRef::Struct(fields) = material.reflect_ref() else {
        return Err("expected material struct".into());
    };
    assert_eq!(
        fields
            .field("no_repeat")
            .and_then(|v| v.try_downcast_ref::<bool>()),
        Some(&true)
    );
    assert_eq!(
        fields
            .field("no_repeat_scale")
            .and_then(|v| v.try_downcast_ref::<f32>()),
        Some(&3.0)
    );
    let mut json = serde_json::to_value(material)?;
    let restored: VeMaterial = serde_json::from_value(json.clone())?;
    assert_eq!(restored, material);
    let object = json.as_object_mut().ok_or("expected object")?;
    object.remove("no_repeat");
    object.remove("no_repeat_scale");
    let legacy: VeMaterial = serde_json::from_value(json)?;
    assert!(!legacy.no_repeat);
    assert_eq!(legacy.no_repeat_scale, 1.0);
    Ok(())
}
