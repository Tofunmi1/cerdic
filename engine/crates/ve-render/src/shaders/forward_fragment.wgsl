@fragment
fn fragment(input: VertexOutput, @builtin(front_facing) front_facing: bool) -> @location(0) vec4<f32> {
    // Foliage is drawn two-sided (leaves, grass, fences), so its back faces are lit with the normal turned toward the viewer.
    let two_sided = water.grass.look.x > 0.5 || input.foliage.x > 0.0 || input.foliage.y > 0.0 || input.foliage.z > 0.0;
    let surface_normal = select(input.world_normal, -input.world_normal, two_sided && !front_facing);
    let pattern = procedural_surface(
        input.params.z,
        input.object_position * clamp(input.params.w, 0.1, 50.0),
        input.emissive.a,
    );
    let bump_gradient = dpdx(input.world_position) * dpdx(pattern.z)
        + dpdy(input.world_position) * dpdy(pattern.z);
    var n = normalize(surface_normal - bump_gradient * 0.035);
    let v = normalize(view.camera_position.xyz - input.world_position);
    // Baked maps: the derivatives are taken here, in uniform control flow, and handed to every read, so each read can pick its
    // array by the instance's map id. A surface without a map skips the fetch.
    let uv_dx = dpdx(input.uv);
    let uv_dy = dpdy(input.uv);
    let no_repeat = input.foliage.w > 0.0 && view.effects.w > 0.5;
    var cells: array<StochasticSample, 3>;
    if (no_repeat) { cells = stochastic_cells(input.uv, input.foliage.w, 0u); }
    let albedo_map = sample_stochastic_map(input.maps.x, input.uv, uv_dx, uv_dy, cells, no_repeat);
    let normal_map = sample_stochastic_map(input.maps.y, input.uv, uv_dx, uv_dy, cells, no_repeat);
    let mr_map = sample_stochastic_map(input.maps.z, input.uv, uv_dx, uv_dy, cells, no_repeat);
    let emissive_map = sample_stochastic_map(input.maps.w, input.uv, uv_dx, uv_dy, cells, no_repeat);
    // Derivatives must be taken in uniform control flow, so the mapped normal is always computed.
    let mapped_normal = perturb_normal(normalize(surface_normal), input.world_position, input.uv, normalize(normal_map.xyz * 2.0 - vec3<f32>(1.0)));
    n = select(n, mapped_normal, input.maps.y >= 0.0);
    let grass_ratio = grass_height_ratio(input.object_position.y, water.grass.shape.z, water.grass.shape.w);
    if (water.grass.look.x > 0.5) { n = grass_upward_normal(n, grass_ratio); }
    var metallic = input.params.x;
    if input.params.z > 9.5 && input.params.z < 10.5 { metallic = 0.0; }
    var surface_roughness = input.params.y;
    if (input.maps.z >= 0.0) {
        // glTF packing: green is roughness, blue is metallic, both multiplying the factors.
        surface_roughness = surface_roughness * mr_map.g;
        metallic = metallic * mr_map.b;
    }
    var water_color = vec3<f32>(0.0);
    var water_foam = 0.0;
    var water_edge = 1.0;
    let is_water = input.params.z > 9.5 && input.params.z < 10.5;
    if (is_water) {
        let data = input.water_data;
        let flow = data.xy * 2.0 - vec2<f32>(1.0);
        let t = view.wind.w;
        let drift = flow * t * water.params.x * 0.4;
        let detail = water_detail_normal(input.world_position.xz - drift, t);
        let wave = water_wave_normal(input.world_position.xz, t);
        if (normalize(surface_normal).y > 0.7) {
            n = normalize(normalize(surface_normal) + vec3<f32>(wave.x / wave.y + detail.x / detail.y, 0.0, wave.z / wave.y + detail.z / detail.y));
        }
        let depth_mix = smoothstep(0.0, 1.0, clamp(data.z * 6.0 / water.shallow.w, 0.0, 1.0));
        let breakup = noise3(vec3<f32>((input.world_position.xz - drift) * 5.0, 0.37));
        water_foam = data.w * smoothstep(0.28, 0.72, breakup) * water.params.z * (1.0 - 0.5 * depth_mix);
        water_color = mix(mix(water.shallow.rgb, water.deep.rgb, depth_mix), vec3<f32>(0.9, 0.93, 0.95), water_foam);
        water_edge = smoothstep(0.0, 0.12, data.z);
        surface_roughness = 0.08 + water_foam * 0.55;
    }
    let terrain = terrain_surface(input.world_position, normalize(surface_normal), input.foliage.w * view.effects.w, water.terrain);
    var terrain_height = 0.0;
    if (water.terrain.c.y > 0.5) {
        terrain_height = terrain_detail(input.world_position * water.terrain.c.x, input.foliage.w * view.effects.w).z;
    }
    let terrain_gradient = dpdx(input.world_position) * dpdx(terrain_height) + dpdy(input.world_position) * dpdy(terrain_height);
    if (water.terrain.c.y > 0.5) {
        surface_roughness = terrain.w;
        n = normalize(n - terrain_gradient * water.terrain.b.w * 0.035);
    }
    let base_roughness = clamp(surface_roughness + pattern.y, 0.045, 1.0);
    let normal_dx = dpdx(n);
    let normal_dy = dpdy(n);
    let variance = max(dot(normal_dx, normal_dx) + dot(normal_dy, normal_dy), 0.0);
    let alpha = clamp(base_roughness * base_roughness + variance * view.effects.x, 0.045 * 0.045, 1.0);
    let roughness = sqrt(alpha);
    var recipe_color = input.base_color.rgb;
    if (input.params.z > 0.5) {
        let hue_shift = fract(sin(input.emissive.a * 23.17 + 3.11) * 43758.5453) * 0.08;
        recipe_color = mix(recipe_color, recipe_color.gbr, hue_shift);
    }
    if (input.maps.x >= 0.0) {
        recipe_color = recipe_color * srgb_to_linear(albedo_map.rgb);
    }
    var emitted = input.emissive.rgb;
    if (input.maps.w >= 0.0) {
        emitted = emitted + srgb_to_linear(emissive_map.rgb);
    }
    var base = select(select(recipe_color * pattern.x, terrain.rgb, water.terrain.c.y > 0.5), water_color, is_water);
    if (water.grass.look.x > 0.5) {
        base = grass_ground_blend(base, water.grass.ground.rgb, grass_ratio, water.grass.ground.w, water.grass.look.z);
    }
    let diffuse_color = base * (1.0 - metallic);
    let f0 = mix(select(vec3<f32>(0.04), vec3<f32>(0.02), is_water), base, metallic);

    var radiance = vec3<f32>(0.0);
    if (dot(lights.sun_color.rgb, vec3<f32>(1.0)) > 0.0) {
        var sun_radiance = shade(n, v, normalize(lights.sun_direction.xyz), lights.sun_color.rgb, diffuse_color, f0, alpha);
        if (is_water) {
            let sun = normalize(lights.sun_direction.xyz);
            let specular = shade(n, v, sun, lights.sun_color.rgb, vec3<f32>(0.0), f0, alpha);
            sun_radiance -= specular * (1.0 - water_sun_glint(n, sun));
        }
        if (water.grass.look.x > 0.5 && view.grading.w > 0.5) {
            let back_wrap = clamp((-dot(n, normalize(lights.sun_direction.xyz)) + 0.5) / 2.25, 0.0, 1.0);
            sun_radiance += diffuse_color * lights.sun_color.rgb / PI * back_wrap * water.grass.look.y * grass_ratio;
        }
        radiance += sun_radiance * sun_visibility(input.world_position, n);
    }
    let count = i32(lights.counts.x);
    for (var i = 0; i < count; i = i + 1) {
        let light = lights.points[i];
        let to_light = light.position_range.xyz - input.world_position;
        let distance_squared = max(dot(to_light, to_light), 0.0001);
        let range = max(light.position_range.w, 0.0001);
        // Inverse-square falloff windowed so the light reaches exactly zero at its range.
        let window = clamp(1.0 - pow(distance_squared / (range * range), 2.0), 0.0, 1.0);
        let attenuation = window * window / distance_squared;
        radiance += shade(n, v, to_light * inverseSqrt(distance_squared), light.color.rgb * attenuation, diffuse_color, f0, alpha);
    }
    let spot_count = i32(lights.counts.y);
    for (var i = 0; i < spot_count; i = i + 1) {
        let light = lights.spots[i];
        let to_light = light.position_range.xyz - input.world_position;
        let distance_squared = max(dot(to_light, to_light), 0.0001);
        let range = max(light.position_range.w, 0.0001);
        let window = clamp(1.0 - pow(distance_squared / (range * range), 2.0), 0.0, 1.0);
        let attenuation = window * window / distance_squared;
        let toward_light = to_light * inverseSqrt(distance_squared);
        let cone_cosine = dot(normalize(light.direction_outer.xyz), -toward_light);
        let cone = smoothstep(light.direction_outer.w, light.color_inner.w, cone_cosine);
        radiance += shade(n, v, toward_light, light.color_inner.rgb * attenuation * cone, diffuse_color, f0, alpha);
    }

    // Low-order (L1) spherical-harmonic ambient irradiance. A two-colour sky/ground hemisphere is
    // exactly the first-order band along Y, so this reproduces the sky-to-ground gradient exactly;
    // it is a bounded approximation of an environment, not global illumination.
    var ambient_irradiance = lights.ambient_sh0.rgb + lights.ambient_sh1.rgb * clamp(n.y, -1.0, 1.0);
    var bounce_strength = lights.ambient_bounce.w;
    if (lights.gi_dims.w > 0.5) {
        // Light probes replace the constant sky with traced irradiance: the sky as this place sees it (sheltered corners are
        // darker), and sun and emission bounced off other surfaces. They already include the ground bounce.
        ambient_irradiance = probe_irradiance(input.world_position, n);
        bounce_strength = 0.0;
    }
    // Optional warm ground bounce: light leaving a warm floor reaches downward-facing surfaces.
    let bounce_term = lights.ambient_bounce.rgb * (bounce_strength * max(-n.y, 0.0))
        * diffuse_color * input.occlusion;
    var ambient = ambient_irradiance * (diffuse_color + f0 * 0.2) * input.occlusion + bounce_term;
    if (lights.ambient.w > 0.0) {
        // The reflected sky replaces the flat specular ambient, so metal shows the studio around it.
        let n_dot_v = max(dot(n, v), 0.0001);
        let facing_up = clamp(n.y * 0.5 + 0.5, 0.6, 1.0);
        let sky = environment(reflect(-v, n), roughness) * env_brdf(f0, roughness, n_dot_v);
        ambient = (ambient_irradiance * diffuse_color + sky * facing_up * lights.ambient.w) * input.occlusion + bounce_term;
    }
    var grass_occlusion = 1.0;
    if (water.grass.look.x > 0.5 && view.grading.w > 0.5) {
        grass_occlusion = grass_sky_occlusion(grass_ratio) * grass_contact_ao(grass_ratio);
    }
    var color = (radiance + ambient) * grass_occlusion + emitted;
    if (is_water) {
        let fresnel = fresnel_schlick(max(dot(n, v), 0.0), vec3<f32>(0.02));
        let reflected = max(environment(reflect(-v, n), roughness), lights.ambient_sh0.rgb);
        // Direct sun glints are restricted to sun-facing wave crests above.
        color = radiance + mix(ambient + emitted, reflected, fresnel * (1.0 - water_foam));
    }
    if (input.toon_a.x > 0.5) {
        // Cel shading replaces the physically based response: one lit colour, one tinted shadow colour, flat bands between.
        let bands = input.toon_a.x;
        let feather = input.toon_a.y;
        let sun_light = normalize(lights.sun_direction.xyz);
        let sun_power = lights.sun_color.rgb / PI;
        let flat_ambient = lights.ambient_sh0.rgb + lights.ambient_sh1.rgb * 0.25;
        let n_dot_sun = dot(n, sun_light);
        // Cast shadows are all-or-nothing in a cel look: the shadow map's soft edge is snapped.
        let unshadowed = smoothstep(0.4, 0.6, sun_visibility(input.world_position, n));
        let sun_step = toon_step(n_dot_sun + input.toon_a.z, bands, feather) * unshadowed;
        let lit = base * (flat_ambient + sun_power);
        let shadowed = base * input.toon_b.rgb * (flat_ambient + sun_power * 0.12);
        var toon = mix(shadowed, lit, sun_step);
        // Other lights add their own banded light on top.
        for (var i = 0; i < count; i = i + 1) {
            let light = lights.points[i];
            let to_light = light.position_range.xyz - input.world_position;
            let distance_squared = max(dot(to_light, to_light), 0.0001);
            let range = max(light.position_range.w, 0.0001);
            let window = clamp(1.0 - pow(distance_squared / (range * range), 2.0), 0.0, 1.0);
            let attenuation = window * window / distance_squared;
            let step_light = toon_step(dot(n, to_light * inverseSqrt(distance_squared)) + input.toon_a.z, bands, feather);
            toon = toon + base * light.color.rgb * attenuation / PI * step_light;
        }
        // Rim: a bright edge where the surface turns away from the viewer, only on the side the sun reaches.
        let rim_mask = pow(1.0 - clamp(dot(n, v), 0.0, 1.0), 3.0) * smoothstep(-0.1, 0.3, n_dot_sun) * unshadowed;
        toon = toon + base * (sun_power + flat_ambient) * rim_mask * input.toon_b.w;
        // Highlight: a small hard-edged blob where the half vector meets the normal.
        if (input.toon_a.w > 0.0) {
            let half_vector = normalize(v + sun_light);
            let threshold = 1.0 - input.toon_a.w * 0.06;
            let blob = smoothstep(threshold, threshold + 0.004, dot(n, half_vector)) * step(0.0, n_dot_sun) * unshadowed;
            toon = toon + lights.sun_color.rgb * 0.18 * blob;
        }
        // The baked ambient occlusion is a smooth gradient, which a cel look does not have: it is left out here.
        color = toon * grass_occlusion + emitted;
    }
    // Cutout (leaves, grass, fences): the albedo map's alpha under the cutoff drops the pixel. This comes last, after every
    // texture lookup and derivative, so the discard cannot disturb them.
    if (input.foliage.x > 0.0 && input.maps.x >= 0.0 && albedo_map.a < input.foliage.x) {
        discard;
    }
    let auto_exposure = textureLoad(exposure_state, vec2<i32>(0, 0), 0).x;
    return vec4<f32>(grade(tonemap_aces(color * view.exposure.x * auto_exposure)), input.base_color.a * water_edge);
}
