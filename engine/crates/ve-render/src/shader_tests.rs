//! Every shader parses and validates. A WGSL mistake otherwise shows up only as a silently missing pass in a browser (the
//! lens, the headset split or every mesh stops drawing), so it is caught here.
use naga::valid::{Capabilities, ValidationFlags, Validator};

fn check(name: &str, source: &str) {
    let module = naga::front::wgsl::parse_str(source)
        .unwrap_or_else(|error| panic!("{name} does not parse:\n{}", error.emit_to_string(source)));
    Validator::new(ValidationFlags::all(), Capabilities::all())
        .validate(&module)
        .unwrap_or_else(|error| panic!("{name} does not validate: {error:?}"));
    if let Err(message) = translates_to_webgl(source) {
        panic!("{name} does not translate to GLSL ES 3.00 (the browser's backend): {message}");
    }
}

/// Translates every entry point to GLSL ES 3.00 for WebGL2, which is what the browser's wgpu backend does at pipeline creation.
/// Some valid WGSL has no GLSL form there (a `textureLoad` from a depth texture, for example), and the browser only reports it
/// as a validation error that stops the whole frame.
fn translates_to_webgl(source: &str) -> Result<String, String> {
    use naga::back::glsl::{Options, PipelineOptions, Version, Writer};
    let module =
        naga::front::wgsl::parse_str(source).map_err(|error| error.emit_to_string(source))?;
    let info = Validator::new(ValidationFlags::all(), Capabilities::all())
        .validate(&module)
        .map_err(|error| format!("{error:?}"))?;
    let options = Options {
        version: Version::Embedded {
            version: 300,
            is_webgl: true,
        },
        ..Options::default()
    };
    let mut all = String::new();
    for entry in &module.entry_points {
        let pipeline = PipelineOptions {
            shader_stage: entry.stage,
            entry_point: entry.name.clone(),
            multiview: None,
        };
        let mut out = String::new();
        let mut writer = Writer::new(
            &mut out,
            &module,
            &info,
            &options,
            &pipeline,
            naga::proc::BoundsCheckPolicies::default(),
        )
        .map_err(|error| format!("entry point {}: {error}", entry.name))?;
        writer
            .write()
            .map_err(|error| format!("entry point {}: {error}", entry.name))?;
        all.push_str(&out);
    }
    Ok(all)
}

#[test]
fn a_depth_texture_load_has_no_webgl_form() {
    // Documents the limit that broke the first screen-space occlusion: reading depth needs `textureSampleLevel`.
    let load = "@group(0) @binding(0) var d: texture_depth_2d;\n@fragment fn f() -> @location(0) vec4<f32> { return vec4<f32>(textureLoad(d, vec2<i32>(0, 0), 0)); }";
    assert!(translates_to_webgl(load).is_err());
    let sample = "@group(0) @binding(0) var d: texture_depth_2d;\n@group(0) @binding(1) var s: sampler;\n@fragment fn f() -> @location(0) vec4<f32> { return vec4<f32>(textureSampleLevel(d, s, vec2<f32>(0.5, 0.5), 0)); }";
    // A plain sample of a depth texture translates, but to a shadow sampler that takes a comparison reference, which the
    // GLSL compiler then rejects for the two-component coordinate: depth cannot be read as a value on this backend.
    let glsl = translates_to_webgl(sample).unwrap_or_default();
    assert!(glsl.contains("sampler2DShadow"), "{glsl}");
}

#[test]
fn the_post_pass_reads_depth_from_a_colour_texture() {
    // `present.wgsl` must not bind a real depth texture: its depth input is the `ao_depth` pass's float texture.
    let present = include_str!("shaders/present.wgsl");
    assert!(present.contains("depth_source: texture_2d<f32>"));
    assert!(!present.contains("texture_depth_2d"));
    check("ao_depth.wgsl", crate::ao_depth::SHADER);
    check("ao.wgsl", include_str!("shaders/ao.wgsl"));
    check("exposure.wgsl", include_str!("shaders/exposure.wgsl"));
}

#[test]
fn the_forward_shader_parses_and_validates() {
    check("forward.wgsl", crate::pipeline::FORWARD_SHADER);
    check(
        "forward_without_sim",
        &crate::pipeline::without_sim(crate::pipeline::FORWARD_SHADER),
    );
    check(
        "forward_without_uv",
        &crate::pipeline::forward_shader_without_uv(),
    );
    for has_uv in [false, true] {
        check(
            "forward_color",
            &crate::pipeline::forward_shader_with_color(has_uv),
        );
    }
    for has_uv in [false, true] {
        check(
            "ordinary_water",
            &crate::pipeline::without_sim(&crate::pipeline::forward_shader_with_color(has_uv)),
        );
    }
    check("water_waves.wgsl", concat!(include_str!("shaders/grass.wgsl"), include_str!("shaders/surface_types.wgsl"), include_str!("shaders/forward_terrain.wgsl"), include_str!("shaders/stochastic.wgsl"), include_str!("shaders/water_waves.wgsl"),
        "@vertex fn v(@location(0) p: vec2<f32>) -> @builtin(position) vec4<f32> { return vec4<f32>(p.x, water_wave_height(p, 1.0), p.y, 1.0); }
",
        "@fragment fn f() -> @location(0) vec4<f32> { return vec4<f32>(water_wave_normal(vec2<f32>(1.0), 1.0) + water_detail_normal(vec2<f32>(1.0), 1.0), 1.0); }"));
}

#[test]
fn the_present_shader_parses_and_validates() {
    check("present.wgsl", include_str!("shaders/present.wgsl"));
}

#[test]
fn the_shadow_shader_parses_and_validates() {
    check("shadow.wgsl", crate::shadow::SHADOW_SHADER);
}

#[test]
fn the_sky_shader_parses_and_validates() {
    check("sky.wgsl", include_str!("shaders/sky.wgsl"));
}

#[test]
fn stochastic_shader_translates_to_webgl() {
    check(
        "stochastic.wgsl",
        concat!(
            include_str!("shaders/stochastic.wgsl"),
            "@fragment fn f(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> { let s = stochastic_cells(uv, 1.0, 7u); return vec4<f32>(s[0].uv, s[0].weight, 1.0); }"
        ),
    );
}

#[test]
fn shared_surface_uniform_has_the_same_cpu_and_shader_size() {
    use bevy_render::render_resource::ShaderType;
    let module = naga::front::wgsl::parse_str(crate::pipeline::FORWARD_SHADER).unwrap();
    let span = module
        .types
        .iter()
        .find_map(|(_, ty)| match (&ty.name, &ty.inner) {
            (Some(name), naga::TypeInner::Struct { span, .. }) if name == "WaterWaves" => {
                Some(u64::from(*span))
            }
            _ => None,
        });
    assert_eq!(span, Some(crate::water_gpu::WaterUniform::min_size().get()));
}

#[test]
fn grass_helpers_translate_to_webgl() {
    check(
        "grass.wgsl",
        concat!(
            include_str!("shaders/grass.wgsl"),
            r#"
        @vertex fn v(@location(0) p: vec3<f32>) -> @builtin(position) vec4<f32> {
            let ratio = grass_height_ratio(p.y, -0.1, 2.0);
            let normal = grass_upward_normal(vec3<f32>(1.0, 0.0, 0.0), ratio);
            var grass: GrassParams;
            grass.look = vec4<f32>(1.0, 0.5, 1.0, 0.0);
            grass.shape = vec4<f32>(25.0, 70.0, -0.1, 2.0);
            let world = grass_world_position(p, mat4x4<f32>(), p, vec3<f32>(0.0), grass);
            return vec4<f32>(world + normal * grass_sink_correction(p.y, ratio) * grass_fade_scale(40.0, 25.0, 70.0), 1.0);
        }
        @fragment fn f() -> @location(0) vec4<f32> {
            return vec4<f32>(grass_ground_blend(vec3<f32>(0.5), vec3<f32>(0.2), 0.2, 0.6, 1.0) * grass_sky_occlusion(0.2) * grass_contact_ao(0.2), 1.0);
        }
    "#
        ),
    );
}

#[test]
fn only_simulation_pipelines_bind_the_height_texture() {
    let sim = crate::pipeline::FORWARD_SHADER;
    let ordinary = crate::pipeline::without_sim(sim);
    assert!(sim.contains("@binding(1) var water_sim: texture_2d<f32>"));
    assert!(!ordinary.contains("var water_sim:"));
    assert!(sim.contains("textureLoad(water_sim"));
    check("interactive_water", sim);
    check("ordinary_water", &ordinary);
}
