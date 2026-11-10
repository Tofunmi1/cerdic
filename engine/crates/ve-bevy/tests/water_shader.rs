//! Compose the real water shader with Bevy's imports and validate both entry points without a GPU.
use bevy_asset::Assets;
use bevy_shader::{Shader, ShaderCache, ShaderCacheError, ShaderCacheSource, ShaderDefVal};
use std::path::Path;

fn validate(
    _: &(),
    source: ShaderCacheSource,
    _: &bevy_shader::ValidateShader,
) -> Result<(), ShaderCacheError> {
    let ShaderCacheSource::Wgsl(source) = source else {
        return Err(ShaderCacheError::ProcessShaderError("expected WGSL".into()));
    };
    let module = naga::front::wgsl::parse_str(&source)
        .map_err(|e| ShaderCacheError::ProcessShaderError(e.emit_to_string(&source)))?;
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .map_err(|e| ShaderCacheError::ProcessShaderError(e.emit_to_string(&source)))?;
    Ok(())
}

fn register(
    dir: &Path,
    root: &Path,
    package: &str,
    assets: &mut Assets<Shader>,
    cache: &mut ShaderCache<(), ()>,
) -> Result<(), Box<dyn std::error::Error>> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            register(&path, root, package, assets, cache)?;
        } else if path.extension().is_some_and(|ext| ext == "wesl") {
            let shader = Shader::from_wesl(
                std::fs::read_to_string(&path)?,
                format!(
                    "embedded://{package}/{}",
                    path.strip_prefix(root)?.display()
                ),
            );
            let handle = assets.add(shader.clone());
            cache.set_shader(handle.id(), shader);
            // Keep strong handles alive for the duration of the fixture.
            std::mem::forget(handle);
        }
    }
    Ok(())
}

#[test]
fn water_vertex_and_fragment_compose_and_validate() -> Result<(), Box<dyn std::error::Error>> {
    let mut assets = Assets::default();
    let mut cache = ShaderCache::new((), validate);
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    for package in ["bevy_pbr", "bevy_render", "bevy_core_pipeline"] {
        let root = manifest
            .join("../../lib/bevy/crates")
            .join(package)
            .join("src");
        register(&root, &root, package, &mut assets, &mut cache)?;
    }
    let shader = Shader::from_wesl(
        include_str!("../src/shaders/water.wesl"),
        "embedded://ve_bevy/shaders/water.wesl",
    );
    let handle = assets.add(shader.clone());
    cache.set_shader(handle.id(), shader);
    let mut defs: Vec<ShaderDefVal> = [
        "VERTEX_POSITIONS",
        "VERTEX_NORMALS",
        "VERTEX_COLORS",
        "VERTEX_UVS_A",
        "VERTEX_OUTPUT_INSTANCE_INDEX",
    ]
    .into_iter()
    .map(Into::into)
    .collect();
    for (name, value) in [
        ("MATERIAL_BIND_GROUP", 3),
        ("MAX_DIRECTIONAL_LIGHTS", 10),
        ("MAX_CASCADES_PER_LIGHT", 4),
        ("MAX_RECT_LIGHTS", 16),
    ] {
        defs.push(ShaderDefVal::UInt(name.into(), value));
    }
    cache.get(0, handle.id(), &defs)?;
    Ok(())
}
