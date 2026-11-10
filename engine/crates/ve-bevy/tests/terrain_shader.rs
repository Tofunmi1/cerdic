//! Compose the real terrain shader (exactly the text `VeBevyTerrainPlugin` embeds) with Bevy's imports and validate its fragment entry point without a GPU.
//! A terrain shader that does not compose draws nothing useful in the browser (a flat dark ground); nothing else catches it.
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
fn terrain_fragment_composes_and_validates() -> Result<(), Box<dyn std::error::Error>> {
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
    // The same text the plugin embeds (see `VeBevyTerrainPlugin::build`).
    let source = concat!(
        include_str!("../src/shaders/terrain.wesl"),
        include_str!("../src/shaders/terrain_params.wgsl"),
        include_str!("../../ve-render/src/shaders/stochastic.wgsl"),
        include_str!("../../ve-render/src/shaders/forward_terrain.wgsl"),
    );
    let shader = Shader::from_wesl(source, "embedded://ve_bevy/shaders/terrain.wesl");
    let handle = assets.add(shader.clone());
    cache.set_shader(handle.id(), shader);
    let mut defs: Vec<ShaderDefVal> = [
        "VERTEX_POSITIONS",
        "VERTEX_NORMALS",
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

/// The embedded text must define every type and function the terrain shader uses, on its own.
#[test]
fn the_embedded_terrain_text_defines_its_parameter_struct() {
    let source = concat!(
        include_str!("../src/shaders/terrain.wesl"),
        include_str!("../src/shaders/terrain_params.wgsl"),
        include_str!("../../ve-render/src/shaders/stochastic.wgsl"),
        include_str!("../../ve-render/src/shaders/forward_terrain.wgsl"),
    );
    assert!(
        source.contains("struct TerrainParams"),
        "the terrain shader reads `TerrainParams` but the text the plugin embeds never defines it"
    );
}

/// `terrain_params.wgsl` is a copy of the struct in ve-render's `surface_types.wgsl` (the forward renderer reads the same uniform layout): they must not drift.
#[test]
fn the_bevy_copy_of_terrain_params_matches_the_forward_renderers() {
    fn struct_text(source: &str) -> String {
        let start = source
            .find("struct TerrainParams")
            .expect("struct TerrainParams");
        let end = start + source[start..].find("};").expect("end of struct") + 2;
        // Compare the fields only: whitespace and comments differ freely.
        source[start..end]
            .lines()
            .map(|line| {
                line.split("//")
                    .next()
                    .unwrap_or("")
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }
    let bevy = struct_text(include_str!("../src/shaders/terrain_params.wgsl"));
    let forward = struct_text(include_str!(
        "../../ve-render/src/shaders/surface_types.wgsl"
    ));
    assert_eq!(bevy, forward);
}
