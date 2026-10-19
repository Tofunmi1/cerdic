// Automatic exposure: one pixel that measures the finished picture and writes the next exposure (see exposure.rs).
//
// The picture is the view's resolved colour texture, read as linear display brightness (it was tonemapped by the forward
// shader). The measurement is the geometric mean over a grid of samples, which is steady against a few very bright or very
// dark pixels. The exposure moves towards the value that would put that mean at the target, a share of the way per frame.
// `next_exposure` mirrors the Rust function of the same name; keep them in step.

struct View {
    clip_from_world: mat4x4<f32>,
    camera_position: vec4<f32>,
    // x is the manual exposure (unused here), y the share of the correction this frame applies, z the target brightness.
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
// The exposure now in use: a 1 by 1 float texture.
@group(0) @binding(1) var exposure_state: texture_2d<f32>;
@group(1) @binding(0) var colour: texture_2d<f32>;

// Samples per side of the measuring grid.
const GRID: i32 = 24;
const EV_LIMIT: f32 = 6.0;
const ERROR_LIMIT: f32 = 3.0;
const GAIN: f32 = 0.6;

@vertex
fn vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    // One triangle covering the target.
    let x = f32((index << 1u) & 2u);
    let y = f32(index & 2u);
    return vec4<f32>(x * 2.0 - 1.0, y * 2.0 - 1.0, 0.0, 1.0);
}

fn next_exposure(previous: f32, measured: f32, goal: f32, adapt: f32) -> f32 {
    let error = clamp(log2(goal / max(measured, 1.0e-4)), -ERROR_LIMIT, ERROR_LIMIT);
    let stops = clamp(log2(max(previous, 1.0e-4)) + error * GAIN * adapt, -EV_LIMIT, EV_LIMIT);
    return exp2(stops);
}

@fragment
fn fragment() -> @location(0) f32 {
    let previous = textureLoad(exposure_state, vec2<i32>(0, 0), 0).x;
    var sum = 0.0;
    for (var y = 0; y < GRID; y += 1) {
        for (var x = 0; x < GRID; x += 1) {
            let uv = (vec2<f32>(f32(x), f32(y)) + 0.5) / f32(GRID);
            let pixel = vec2<i32>(view.viewport.xy + uv * view.viewport.zw);
            let rgb = textureLoad(colour, pixel, 0).rgb;
            // Brightness is capped at 1 (a clipped highlight says nothing about how much too bright it is) and floored.
            let brightness = clamp(dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722)), 0.001, 1.0);
            sum += log2(brightness);
        }
    }
    let measured = exp2(sum / f32(GRID * GRID));
    return next_exposure(previous, measured, view.exposure.z, view.exposure.y);
}
