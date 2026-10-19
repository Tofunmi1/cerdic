// Per-eye fullscreen analytic sky. No sampled images or imported assets.
struct View {
    clip_from_world: mat4x4<f32>,
    camera_position: vec4<f32>,
    exposure: vec4<f32>,
    grading: vec4<f32>,
    tint: vec4<f32>,
    effects: vec4<f32>,
    world_from_clip: mat4x4<f32>,
    viewport: vec4<f32>,
    horizon: vec4<f32>,
    zenith: vec4<f32>,
    ground: vec4<f32>,
    sun: vec4<f32>,
    cloud: vec4<f32>,
};
@group(0) @binding(0) var<uniform> view: View;

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let low = c / 12.92;
    let high = pow((c + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));
    return select(high, low, c <= vec3<f32>(0.04045));
}

@vertex
fn vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let positions = array<vec2<f32>, 3>(vec2<f32>(-1.0,-1.0), vec2<f32>(3.0,-1.0), vec2<f32>(-1.0,3.0));
    return vec4<f32>(positions[index], 0.0, 1.0);
}

fn mul32_high(a: u32, b: u32) -> u32 {
    let a0 = a & 0xffffu; let a1 = a >> 16u;
    let b0 = b & 0xffffu; let b1 = b >> 16u;
    let p0 = a0*b0;
    let t1 = a1*b0 + (p0 >> 16u);
    let t2 = a0*b1 + (t1 & 0xffffu);
    return a1*b1 + (t1 >> 16u) + (t2 >> 16u);
}
// u64 represented as [low32, high32]; WGSL/WebGPU has no native u64 integer.
fn mul64(a: vec2<u32>, b: vec2<u32>) -> vec2<u32> {
    let low = a.x*b.x;
    let high = mul32_high(a.x,b.x) + a.y*b.x + a.x*b.y;
    return vec2<u32>(low,high);
}
fn shr64(value: vec2<u32>, bits: u32) -> vec2<u32> {
    return vec2<u32>((value.x >> bits) | (value.y << (32u-bits)), value.y >> bits);
}
fn hash_cell(cell: vec3<i32>, seed: u32) -> u32 {
    var hash = mul64(vec2<u32>(seed,0u), vec2<u32>(0x7f4a7c15u,0x9e3779b9u));
    for (var axis = 0u; axis < 3u; axis += 1u) {
        let signed_cell = cell[axis];
        hash = hash ^ vec2<u32>(u32(signed_cell),select(0xffffffffu,0u,signed_cell >= 0));
        hash = mul64(hash,vec2<u32>(0x1ce4e5b9u,0xbf58476du));
        hash = hash ^ shr64(hash,31u);
    }
    hash = mul64(hash,vec2<u32>(0x133111ebu,0x94d049bbu));
    hash = hash ^ shr64(hash,29u);
    return hash.y >> 8u;
}
fn noise2(p: vec2<f32>, seed: u32) -> f32 {
    let cell = vec2<i32>(floor(p)); let f = fract(p); let w = f*f*(vec2<f32>(3.0)-2.0*f);
    let a = f32(hash_cell(vec3<i32>(cell,0),seed) >> 8u) / 16777216.0;
    let b = f32(hash_cell(vec3<i32>(cell+vec2<i32>(1,0),0),seed) >> 8u) / 16777216.0;
    let c = f32(hash_cell(vec3<i32>(cell+vec2<i32>(0,1),0),seed) >> 8u) / 16777216.0;
    let d = f32(hash_cell(vec3<i32>(cell+vec2<i32>(1,1),0),seed) >> 8u) / 16777216.0;
    return mix(mix(a,b,w.x),mix(c,d,w.x),w.y);
}

@fragment
fn fragment(@builtin(position) pixel: vec4<f32>) -> @location(0) vec4<f32> {
    let local = (pixel.xy - view.viewport.xy) / max(view.viewport.zw, vec2<f32>(1.0));
    let ndc = vec4<f32>(local.x*2.0-1.0, 1.0-local.y*2.0, 0.000001, 1.0);
    let point = view.world_from_clip * ndc;
    let ray = normalize(point.xyz / max(abs(point.w), 0.000001) - view.camera_position.xyz);
    let up = clamp(ray.y, -1.0, 1.0);
    var color = select(mix(view.horizon.rgb, view.ground.rgb, 1.0-smoothstep(-0.35,0.0,up)), mix(view.horizon.rgb, view.zenith.rgb, smoothstep(0.0,0.85,up)), up >= 0.0);
    let to_sun = normalize(-view.sun.xyz);
    let sun_dot = dot(ray,to_sun);
    let disc = smoothstep(cos(0.012),cos(0.007),sun_dot);
    let glow = exp(-max(0.0,1.0-sun_dot)*180.0);
    color = color + vec3<f32>(1.0,0.75,0.43) * (disc*4.0 + glow*view.sun.w*0.18);
    if view.cloud.w > 0.5 && up > 0.0 {
        let coord = ray.xz / max(up,0.08) * 2.2;
        let n = noise2(coord, u32(view.cloud.x)) * 0.65 + noise2(coord*2.03, u32(view.cloud.x)+101u)*0.35;
        let cloud = smoothstep(0.82-view.cloud.y*0.58,0.92-view.cloud.y*0.45,n);
        color = mix(color,vec3<f32>(0.90,0.92,0.96),cloud*0.78);
    }
    return vec4<f32>(srgb_to_linear(color),1.0);
}
