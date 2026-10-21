// Automatic exposure for the WebGL2 build (Bevy's own auto exposure is a compute shader): one pixel that measures the lit picture and
// writes the exposure scale the next pass multiplies it by (see exposure.rs).
//
// The picture is the view's HDR colour before tone mapping, read as scene brightness. The measurement is the geometric mean over a grid
// of samples, which is steady against a few very bright or very dark pixels. The scale that would put that mean (times the manual
// exposure, which tone mapping applies later) at the target is approached by a share per frame, in stops. Because the scale is applied
// to a picture that is redrawn every frame, the measurement never sees it, so this is a smoothing filter and cannot oscillate.

@group(0) @binding(0) var colour: texture_2d<f32>;
// The scale now in use: a 1 by 1 float texture.
@group(0) @binding(1) var scale_state: texture_2d<f32>;

struct Params {
    viewport: vec4<f32>,
    // x the manual exposure, y the share of the correction this frame applies, z the target brightness, w 1 to forget the old scale.
    control: vec4<f32>,
};
@group(0) @binding(2) var<uniform> params: Params;

// Samples per side of the measuring grid.
const GRID: i32 = 24;
// The scale may change by this many stops either way.
const EV_LIMIT: f32 = 6.0;

@vertex
fn vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let x = f32((index << 1u) & 2u);
    let y = f32(index & 2u);
    return vec4<f32>(x * 2.0 - 1.0, y * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn fragment() -> @location(0) vec4<f32> {
    let previous = select(textureLoad(scale_state, vec2<i32>(0, 0), 0).x, 1.0, params.control.w > 0.5);
    var sum = 0.0;
    for (var y = 0; y < GRID; y += 1) {
        for (var x = 0; x < GRID; x += 1) {
            let uv = (vec2<f32>(f32(x), f32(y)) + 0.5) / f32(GRID);
            let pixel = vec2<i32>(params.viewport.xy + uv * params.viewport.zw);
            let rgb = textureLoad(colour, pixel, 0).rgb;
            // Capped, so a few blown highlights do not say the whole picture is far too bright; floored, so black does not either.
            let brightness = clamp(dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722)) * params.control.x, 0.001, 4.0);
            sum += log2(brightness);
        }
    }
    let measured = exp2(sum / f32(GRID * GRID));
    let wanted = clamp(log2(params.control.z / measured), -EV_LIMIT, EV_LIMIT);
    let stops = mix(log2(max(previous, 1.0e-4)), wanted, params.control.y);
    return vec4<f32>(exp2(stops), 0.0, 0.0, 1.0);
}
