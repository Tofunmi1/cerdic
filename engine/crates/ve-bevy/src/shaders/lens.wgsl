// Lens warp: resamples the finished picture (the views render into an image of the window's size) onto the window the way a lens would.
// Ported from the forward renderer's present pass (`ve-render`'s present.wgsl): a fisheye over the whole frame, or a barrel distortion on
// each half of the frame for a split-screen headset view.

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;

// x is the mode (1 fisheye, 2 headset), y the tangent of half the source's vertical field of view.
struct Lens {
    params: vec4<f32>,
};
@group(0) @binding(2) var<uniform> lens: Lens;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vertex(@builtin(vertex_index) index: u32) -> VertexOutput {
    // One triangle covering the screen: (0,0), (2,0), (0,2) in uv space.
    let x = f32((index << 1u) & 2u);
    let y = f32(index & 2u);
    var out: VertexOutput;
    out.clip_position = vec4<f32>(x * 2.0 - 1.0, y * 2.0 - 1.0, 0.0, 1.0);
    // Texture rows run top to bottom while clip-space y runs up.
    out.uv = vec2<f32>(x, 1.0 - y);
    return out;
}

const PI: f32 = 3.14159265;
// The headset's barrel coefficients (the common two-term radial model).
const BARREL_K1: f32 = 0.22;
const BARREL_K2: f32 = 0.24;

// Where in the source picture the output pixel `uv` comes from; `valid` is false where the lens shows nothing (outside the source, or
// beyond 90 degrees off axis).
struct Warped {
    uv: vec2<f32>,
    valid: bool,
};

fn fisheye(uv: vec2<f32>, aspect: f32) -> Warped {
    // Centre of the frame is the origin, the top edge is y = 1, up is positive.
    let p = vec2<f32>((uv.x * 2.0 - 1.0) * aspect, 1.0 - uv.y * 2.0);
    let r = length(p);
    let tan_half = lens.params.y;
    // Equidistant: the angle off axis is proportional to the distance from the centre, scaled so the top edge looks at the source's own
    // top edge.
    let theta = r * atan(tan_half);
    if (r < 1e-5) {
        return Warped(vec2<f32>(0.5, 0.5), true);
    }
    if (theta >= PI * 0.5 - 0.02) {
        return Warped(uv, false);
    }
    let scale = tan(theta) / tan_half / r;
    let source_point = p * scale;
    // The source's own x range is +-aspect, its y range +-1, both in the units of `tan_half`.
    let ndc = vec2<f32>(source_point.x / aspect, source_point.y);
    let valid = abs(ndc.x) <= 1.0 && abs(ndc.y) <= 1.0;
    return Warped(vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5), valid);
}

fn barrel(uv: vec2<f32>) -> Warped {
    // Each half of the frame is one eye; warp it around its own centre.
    let right = uv.x >= 0.5;
    let local = vec2<f32>(uv.x * 2.0 - select(0.0, 1.0, right), uv.y);
    let p = vec2<f32>(local.x * 2.0 - 1.0, 1.0 - local.y * 2.0);
    let r2 = dot(p, p);
    let q = p * (1.0 + BARREL_K1 * r2 + BARREL_K2 * r2 * r2);
    let valid = abs(q.x) <= 1.0 && abs(q.y) <= 1.0;
    let eye = vec2<f32>(q.x * 0.5 + 0.5, 0.5 - q.y * 0.5);
    let source_point = vec2<f32>(eye.x * 0.5 + select(0.0, 0.5, right), eye.y);
    return Warped(source_point, valid);
}

@fragment
fn fragment(input: VertexOutput) -> @location(0) vec4<f32> {
    let size = vec2<f32>(textureDimensions(source));
    var warped = Warped(input.uv, true);
    if (lens.params.x > 1.5) {
        warped = barrel(input.uv);
    } else if (lens.params.x > 0.5) {
        warped = fisheye(input.uv, size.x / size.y);
    }
    if (!warped.valid) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    return textureSampleLevel(source, source_sampler, warped.uv, 0.0);
}
