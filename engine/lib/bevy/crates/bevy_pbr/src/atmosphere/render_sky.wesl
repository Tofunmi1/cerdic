import package::atmosphere::{
    bindings::{view, settings, lights},
    bruneton_functions::sample_transmittance_lut_segment,
    functions::{
        sample_transmittance_lut,
        sample_sky_view_lut, direction_world_to_atmosphere,
        uv_to_ray_direction, uv_to_ndc, sample_aerial_view_lut,
        sample_sun_radiance, ndc_to_camera_dist, raymarch_atmosphere, 
        get_view_position, max_atmosphere_distance
    },
};

import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput;

@if(DUAL_SOURCE_BLENDING)
enable dual_source_blending;

@if(MULTISAMPLED)
@group(0) @binding(13) var depth_texture: texture_depth_multisampled_2d;
@else
@group(0) @binding(13) var depth_texture: texture_depth_2d;

struct RenderSkyOutput {
    @if(DUAL_SOURCE_BLENDING)
    @location(0) @blend_src(0) inscattering: vec4<f32>,
    @if(DUAL_SOURCE_BLENDING)
    @location(0) @blend_src(1) transmittance: vec4<f32>,
    @else
    @location(0) inscattering: vec4<f32>,
}

// VE patch 5: procedural volumetric clouds. One flat layer over the viewer (the world is locally flat, sea level is y = 0), shaped by value noise in the way of
// Schneider's Horizon Zero Dawn clouds (a low-frequency weather map decides where cloud stands and how tall it is, fractal noise gives the body, finer noise erodes
// its edges), marched with a few steps jittered per pixel and lit by a short march toward the sun (Beer's law, a forward-heavy two-lobe phase, a powder term).
const CLOUD_PI: f32 = 3.14159265;

fn cloud_hash(p: vec3<f32>) -> f32 {
    var q = fract(p * 0.1031);
    q += dot(q, q.zyx + 31.32);
    return fract((q.x + q.y) * q.z);
}

fn cloud_noise(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = mix(mix(cloud_hash(i), cloud_hash(i + vec3(1.0, 0.0, 0.0)), u.x), mix(cloud_hash(i + vec3(0.0, 1.0, 0.0)), cloud_hash(i + vec3(1.0, 1.0, 0.0)), u.x), u.y);
    let b = mix(mix(cloud_hash(i + vec3(0.0, 0.0, 1.0)), cloud_hash(i + vec3(1.0, 0.0, 1.0)), u.x), mix(cloud_hash(i + vec3(0.0, 1.0, 1.0)), cloud_hash(i + vec3(1.0, 1.0, 1.0)), u.x), u.y);
    return mix(a, b, u.z);
}

fn cloud_fbm(p: vec3<f32>, octaves: i32) -> f32 {
    var amplitude = 0.5;
    var sum = 0.0;
    var q = p;
    for (var i = 0; i < octaves; i++) {
        sum += amplitude * cloud_noise(q);
        q = q * 2.03 + vec3(17.1, 3.7, 9.2);
        amplitude *= 0.5;
    }
    return sum;
}

/// Cloud density at a world position; `fine` adds the eroding detail noise (skipped on the light march).
fn cloud_density(pos: vec3<f32>, fine: bool) -> f32 {
    let cover = settings.clouds.x;
    let seed = vec3(settings.clouds.y * 13.37, settings.clouds.y * 7.11, settings.clouds.y * 3.91);
    let base = settings.clouds.z;
    let thick = settings.clouds.w;
    let hf = (pos.y - base) / thick;
    if hf <= 0.0 || hf >= 1.0 {
        return 0.0;
    }
    let drift = vec3(settings.cloud_drift.x, 0.0, settings.cloud_drift.y);
    let q = pos + drift;
    // the weather map: where cloud stands, from large soft patches
    let weather = cloud_noise(vec3(q.xz * (1.0 / 6500.0), 0.5) + seed) * 0.6 + cloud_noise(vec3(q.xz * (1.0 / 2300.0), 1.7) + seed * 1.7) * 0.4;
    let local_cover = smoothstep(0.86 - cover * 0.62, 1.02 - cover * 0.62, weather);
    if local_cover <= 0.0 {
        return 0.0;
    }
    // height profile: a flat rounded base, a top that rises with the local cover (towering where it is thick)
    let top = mix(0.34, 1.0, local_cover);
    let profile = smoothstep(0.0, 0.08, hf) * (1.0 - smoothstep(top * 0.5, top, hf));
    let body = cloud_fbm(q * (1.0 / 1700.0) + seed, 4);
    var d = smoothstep(0.32, 0.62, body * profile * (0.7 + local_cover * 0.9));
    if fine && d > 0.0 {
        // billowy edges: ridged detail noise bites into the thin parts first
        let ridge = 1.0 - abs(2.0 * cloud_fbm(q * (1.0 / 240.0) + seed * 2.3, 3) - 1.0);
        d = max(d - (1.0 - d) * ridge * 0.7 * (0.5 + 0.5 * (1.0 - hf)), 0.0);
    }
    return d * settings.cloud_look.x;
}

fn cloud_phase(cos_theta: f32) -> f32 {
    let g1 = 0.62;
    let g2 = -0.22;
    let hg1 = (1.0 - g1 * g1) / (4.0 * CLOUD_PI * pow(1.0 + g1 * g1 - 2.0 * g1 * cos_theta, 1.5));
    let hg2 = (1.0 - g2 * g2) / (4.0 * CLOUD_PI * pow(1.0 + g2 * g2 - 2.0 * g2 * cos_theta, 1.5));
    return mix(hg2, hg1, 0.7);
}

struct CloudResult {
    /// Light the clouds send toward the viewer (before atmospheric attenuation).
    radiance: vec3<f32>,
    /// Share of the background still visible through the clouds.
    transmittance: f32,
    /// Distance to the middle of the cloud layer along the ray, for the haze in front of it.
    distance: f32,
}

fn march_clouds(origin: vec3<f32>, dir: vec3<f32>, jitter: f32, t_scene: f32) -> CloudResult {
    var result = CloudResult(vec3(0.0), 1.0, 0.0);
    let base = settings.clouds.z;
    let thick = settings.clouds.w;
    // the layer is a slab above the viewer; below the horizon there is none
    if dir.y < 0.015 || origin.y > base {
        return result;
    }
    let t0 = (base - origin.y) / dir.y;
    let t1 = min(min((base + thick - origin.y) / dir.y, 90000.0), t_scene);
    if t0 >= t1 {
        return result;
    }
    // sun
    var to_sun = vec3(0.0, 1.0, 0.0);
    var sun_color = vec3(1.0);
    // the brightest directional light is the sun (whether or not it draws a disc)
    var brightest = -1.0;
    for (var i: u32 = 0u; i < lights.n_directional_lights; i++) {
        let light = &lights.directional_lights[i];
        let strength = dot((*light).color.rgb, vec3(0.3, 0.59, 0.11));
        if strength > brightest {
            brightest = strength;
            to_sun = (*light).direction_to_light;
            sun_color = (*light).color.rgb;
        }
    }
    // the light's colour is already warm at low sun (reddened below), so keep only a little of it; its strength is scaled to read as cloud-white after exposure
    let sun_lum = max(dot(sun_color, vec3(0.3, 0.59, 0.11)), 1.0e-4);
    sun_color = mix(vec3(1.0), sun_color / sun_lum, 0.3) * sun_lum * 0.35;
    let cos_theta = dot(dir, to_sun);
    let phase = cloud_phase(cos_theta);
    let steps = i32(clamp(settings.cloud_look.z, 8.0, 96.0));
    let dt = (t1 - t0) / f32(steps);
    var t = t0 + jitter * dt;
    var trans = 1.0;
    var lum = vec3(0.0);
    let extinction = 0.0055;
    let gain = settings.cloud_look.y;
    // sunlight reaching the layer is reddened by the atmosphere at low sun
    let sun_height = clamp(to_sun.y, 0.0, 1.0);
    let sun_tint = mix(vec3(1.0, 0.55, 0.3), vec3(1.0), smoothstep(0.0, 0.35, sun_height));
    for (var i = 0; i < steps; i++) {
        let pos = origin + dir * t;
        let d = cloud_density(pos, true);
        if d > 0.002 {
            var optical = 0.0;
            for (var j = 0; j < 4; j++) {
                let lp = pos + to_sun * (f32(j) + 0.5) * thick * 0.07 * (1.0 + f32(j));
                optical += cloud_density(lp, false) * thick * 0.07 * (1.0 + f32(j));
            }
            // three octaves of Beer's law, each with less extinction and less weight: light that scatters on inside the cloud keeps the thick parts from going black
            let beer = exp(-optical * extinction) * 0.6 + exp(-optical * extinction * 0.3) * 0.3 + exp(-optical * extinction * 0.09) * 0.1;
            let powder = 1.0 - exp(-d * 3.0);
            let hf = clamp((pos.y - base) / thick, 0.0, 1.0);
            let direct = sun_color * sun_tint * (beer * mix(1.0, powder, 0.5) * phase * 7.0 + beer * 0.12);
            let ambient = sun_color * vec3(0.55, 0.65, 0.85) * (0.10 + 0.32 * hf) * (0.4 + 0.6 * sun_height);
            let l = (direct + ambient) * gain;
            let step_t = exp(-d * extinction * dt);
            lum += trans * l * (1.0 - step_t);
            trans *= step_t;
            if trans < 0.02 {
                break;
            }
        }
        t += dt;
    }
    result.radiance = lum;
    result.transmittance = trans;
    result.distance = (t0 + t1) * 0.5;
    return result;
}

@fragment
fn main(in: FullscreenVertexOutput) -> RenderSkyOutput {
    let depth = textureLoad(depth_texture, vec2<i32>(in.position.xy), 0);

    let ray_dir_ws = uv_to_ray_direction(in.uv);
    let world_pos = get_view_position();
    let r = length(world_pos);
    let up = normalize(world_pos);
    let mu = dot(ray_dir_ws, up);
    let max_samples = settings.sky_max_samples;
    let should_raymarch = settings.rendering_method == 1u;

    var transmittance: vec3<f32>;
    var inscattering: vec3<f32>;

    let sun_radiance = sample_sun_radiance(ray_dir_ws);

    if depth == 0.0 {
        let ray_dir_as = direction_world_to_atmosphere(ray_dir_ws);
        transmittance = sample_transmittance_lut(r, mu);
        inscattering = sample_sky_view_lut(r, ray_dir_as);
        if should_raymarch {
            let t_max = max_atmosphere_distance(r, mu);
            let result = raymarch_atmosphere(world_pos, ray_dir_ws, t_max, max_samples, in.uv, true);
            inscattering = result.inscattering;
            transmittance = result.transmittance;
        }
        inscattering += sun_radiance * transmittance;
    } else {
        let t = ndc_to_camera_dist(vec3(uv_to_ndc(in.uv), depth));
        inscattering = sample_aerial_view_lut(in.uv, t);
        transmittance = sample_transmittance_lut_segment(r, mu, t);
        if should_raymarch {
            let result = raymarch_atmosphere(world_pos, ray_dir_ws, t, max_samples, in.uv, false);
            inscattering = result.inscattering;
            transmittance = result.transmittance;
        }
    }

    // VE patch 5: clouds in front of the sky, and in front of any scene far enough behind them
    if settings.clouds.x > 0.0 {
        var t_scene = 1.0e9;
        if depth != 0.0 {
            t_scene = ndc_to_camera_dist(vec3(uv_to_ndc(in.uv), depth));
        }
        let clouds = march_clouds(view.world_position, ray_dir_ws, cloud_hash(vec3(in.position.xy, 1.0)), t_scene);
        if clouds.transmittance < 1.0 {
            // lit by the sun and seen through the air between: far ones fade into the haze
            let air = sample_transmittance_lut_segment(r, mu, min(clouds.distance, 60000.0));
            inscattering = inscattering * mix(1.0, clouds.transmittance, 0.6) + clouds.radiance * air;
            transmittance *= clouds.transmittance;
        }
    }

    // exposure compensation
    inscattering *= view.exposure;
    
    @if(DUAL_SOURCE_BLENDING)
    return RenderSkyOutput(vec4(inscattering, 0.0), vec4(transmittance, 1.0));
    @if(!DUAL_SOURCE_BLENDING)
    let mean_transmittance = (transmittance.r + transmittance.g + transmittance.b) / 3.0;
    @if(!DUAL_SOURCE_BLENDING)
    return RenderSkyOutput(vec4(inscattering, mean_transmittance));
    
}
