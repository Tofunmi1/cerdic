// CPU-built tables: spatial = (kx, kz, amplitude, phase), temporal.x = omega * flow.
// No random numbers or wave directions are recomputed on the GPU.
const WAVE_COUNT: u32 = 12u;
@group(3) @binding(0) var<uniform> water: WaterWaves;

@group(3) @binding(1) var water_sim: texture_2d<f32>;

fn water_sim_height(p: vec2<f32>) -> f32 {
    if water.sim_enabled.x < 0.5 { return 0.0; }
    let uv = (p - water.sim_bounds.xy) / water.sim_bounds.zw;
    if any(uv < vec2<f32>(0.0)) || any(uv >= vec2<f32>(1.0)) { return 0.0; }
    let dims = vec2<i32>(textureDimensions(water_sim));
    let c = clamp(uv * vec2<f32>(dims) - vec2<f32>(0.5), vec2<f32>(0.0), vec2<f32>(dims - vec2<i32>(1)));
    let a = vec2<i32>(c);
    let b = min(a + vec2<i32>(1), dims - vec2<i32>(1));
    let t = fract(c);
    return mix(mix(textureLoad(water_sim, a, 0).x, textureLoad(water_sim, vec2<i32>(b.x, a.y), 0).x, t.x),
               mix(textureLoad(water_sim, vec2<i32>(a.x, b.y), 0).x, textureLoad(water_sim, b, 0).x, t.x), t.y);
}

fn water_sim_gradient(p: vec2<f32>) -> vec2<f32> {
    if water.sim_enabled.x < 0.5 { return vec2<f32>(0.0); }
    let cell = water.sim_bounds.zw / vec2<f32>(textureDimensions(water_sim));
    return vec2<f32>(water_sim_height(p + vec2<f32>(cell.x, 0.0)) - water_sim_height(p - vec2<f32>(cell.x, 0.0)),
                     water_sim_height(p + vec2<f32>(0.0, cell.y)) - water_sim_height(p - vec2<f32>(0.0, cell.y))) / (2.0 * cell);
}

fn water_wave_phase(wave: WaterWave, p: vec2<f32>, t: f32) -> f32 {
    return dot(wave.spatial.xy, p) - wave.temporal.x * t + wave.spatial.w;
}

fn water_wave_height(p: vec2<f32>, t: f32) -> f32 {
    var height = water_sim_height(p);
    for (var i = 0u; i < WAVE_COUNT; i += 1u) {
        let wave = water.waves[i];
        height += wave.spatial.z * sin(water_wave_phase(wave, p, t));
    }
    return height;
}

fn water_wave_normal(p: vec2<f32>, t: f32) -> vec3<f32> {
    var gradient = water_sim_gradient(p);
    for (var i = 0u; i < WAVE_COUNT; i += 1u) {
        let wave = water.waves[i];
        gradient += wave.spatial.xy * wave.spatial.z * cos(water_wave_phase(wave, p, t));
    }
    return normalize(vec3<f32>(-gradient.x, 1.0, -gradient.y));
}

fn water_detail_normal(p: vec2<f32>, t: f32) -> vec3<f32> {
    var gradient = vec2<f32>(0.0);
    for (var i = 0u; i < 4u; i += 1u) {
        let wave = water.detail[i];
        gradient += wave.spatial.xy * wave.spatial.z * cos(water_wave_phase(wave, p, t));
    }
    return normalize(vec3<f32>(-gradient.x, 1.0, -gradient.y));
}

// Sun highlights gather on sun-facing wave slopes, fading away on the flat sheet.
fn water_sun_glint(normal: vec3<f32>, sun: vec3<f32>) -> f32 {
    let gradient = normal.xz / max(normal.y, 0.001);
    let toward_sun = dot(gradient, sun.xz) / max(length(sun.xz), 0.001);
    let crest = smoothstep(0.025, 0.10, toward_sun);
    let steepness = smoothstep(0.03, 0.18, length(gradient));
    return crest * steepness * 0.4;
}
