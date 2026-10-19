// Present pass: copies the view's resolved main texture to its output with a fullscreen triangle.
// Outputs that are not sRGB views (for example a WebXR layer framebuffer) need the sRGB transfer
// function applied by hand, because the hardware does not encode on write.

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;

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

fn srgb_encode(linear: vec3<f32>) -> vec3<f32> {
    let low = linear * 12.92;
    let high = 1.055 * pow(max(linear, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(high, low, linear <= vec3<f32>(0.0031308));
}

// Lens warp (only the lens entry points use this binding): x is the mode (1 fisheye, 2 headset),
// y the tangent of half the source's vertical field of view.
struct Lens {
    params: vec4<f32>,
};
@group(0) @binding(2) var<uniform> lens: Lens;
@group(0) @binding(3) var depth_source: texture_2d<f32>;

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
@group(0) @binding(4) var<uniform> view: View;
@group(0) @binding(5) var occlusion_source: texture_2d<f32>;

// The reverse-Z depth at a pixel (1 is the near plane, 0 the far plane and the sky), from the depth pass's float texture.
fn depth_at(pixel: vec2<i32>) -> f32 {
    return textureLoad(depth_source, pixel, 0).x;
}

const PI: f32 = 3.14159265;
// The headset's barrel coefficients (the common two-term radial model).
const BARREL_K1: f32 = 0.22;
const BARREL_K2: f32 = 0.24;

// Where in the source picture the output pixel `uv` comes from; `valid` is false where the lens
// shows nothing (outside the source, or beyond 90 degrees off axis).
struct Warped {
    uv: vec2<f32>,
    valid: bool,
};

fn fisheye(uv: vec2<f32>, aspect: f32) -> Warped {
    // Centre of the frame is the origin, the top edge is y = 1, up is positive.
    let p = vec2<f32>((uv.x * 2.0 - 1.0) * aspect, 1.0 - uv.y * 2.0);
    let r = length(p);
    let tan_half = lens.params.y;
    // Equidistant: the angle off axis is proportional to the distance from the centre, scaled so
    // the top edge looks at the source's own top edge.
    let theta = r * atan(tan_half);
    if (r < 1e-5) {
        return Warped(vec2<f32>(0.5, 0.5), true);
    }
    if (theta >= PI * 0.5 - 0.02) {
        return Warped(uv, false);
    }
    let scale = tan(theta) / tan_half / r;
    let source = p * scale;
    // The source's own x range is +-aspect, its y range +-1, both in the units of `tan_half`.
    let ndc = vec2<f32>(source.x / aspect, source.y);
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
    let source = vec2<f32>(eye.x * 0.5 + select(0.0, 0.5, right), eye.y);
    return Warped(source, valid);
}

// Where the output pixel `uv` reads the source picture, under the active lens.
fn warp_source(uv: vec2<f32>) -> Warped {
    let size = vec2<f32>(textureDimensions(source));
    let mode = lens.params.x;
    var w = Warped(uv, true);
    if (mode > 1.5) {
        w = barrel(uv);
    } else if (mode > 0.5) {
        w = fisheye(uv, size.x / size.y);
    }
    return w;
}

fn warp(uv: vec2<f32>) -> vec4<f32> {
    let w = warp_source(uv);
    if (!w.valid) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    return textureSampleLevel(source, source_sampler, w.uv, 0.0);
}

// ---- Ambient occlusion from the `ao` pass (see ao.wgsl) ----
// How much the occlusion darkens the picture (1 would black out a fully closed-in pixel).
const AO_STRENGTH: f32 = 0.9;

// The occlusion at a pixel: the average of the 4 by 4 pixels around it. The `ao` pass jitters its samples with a 4 by 4
// pattern, and this average cancels it. A neighbour is weighted down where its depth differs from the centre's (relative
// to the centre's distance), so occlusion does not bleed across a silhouette.
fn occlusion_at(center: vec2<i32>, center_depth: f32, vmin: vec2<i32>, vmax: vec2<i32>) -> f32 {
    var sum = 0.0;
    var total = 0.0;
    for (var y = -2; y < 2; y += 1) {
        for (var x = -2; x < 2; x += 1) {
            let pos = clamp(center + vec2<i32>(x, y), vmin, vmax);
            let d = depth_at(pos);
            // Reverse-Z depth is inversely proportional to distance, so the ratio is the relative distance.
            let weight = select(0.0, clamp(1.0 - abs(center_depth / d - 1.0) * 20.0, 0.0, 1.0), d > 0.0);
            sum += textureLoad(occlusion_source, pos, 0).x * weight;
            total += weight;
        }
    }
    return select(1.0, sum / total, total > 0.0);
}

fn post_color(output_uv: vec2<f32>) -> vec4<f32> {
    let warped = warp_source(output_uv);
    if (!warped.valid) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    let dimensions = vec2<f32>(textureDimensions(source));
    let viewport_min = vec2<i32>(view.viewport.xy);
    let viewport_max = vec2<i32>(view.viewport.xy + view.viewport.zw) - vec2<i32>(1);
    let center = clamp(vec2<i32>(warped.uv * dimensions), viewport_min, viewport_max);
    var color = textureSampleLevel(source, source_sampler, warped.uv, 0.0);

    if (view.effects.y > 0.5) {
        let center_depth = depth_at(center);
        if (center_depth > 0.0) {
            let visibility = occlusion_at(center, center_depth, viewport_min, viewport_max);
            color = vec4<f32>(color.rgb * mix(1.0, visibility, AO_STRENGTH), color.a);
        }
    }

    if (view.effects.z > 0.5) {
        let bright_offsets = array<vec2<i32>, 5>(
            vec2<i32>(0, 0), vec2<i32>(-3, 0), vec2<i32>(3, 0),
            vec2<i32>(0, -3), vec2<i32>(0, 3)
        );
        var glow = vec3<f32>(0.0);
        for (var i = 0u; i < 5u; i += 1u) {
            let sample_coord = clamp(center + bright_offsets[i], viewport_min, viewport_max);
            let sample_color = textureLoad(source, sample_coord, 0).rgb;
            let weight = select(0.11, 0.24, i == 0u);
            glow += max(sample_color - vec3<f32>(0.78), vec3<f32>(0.0)) * weight;
        }
        color = vec4<f32>(color.rgb + glow, color.a);
    }
    return color;
}

@fragment
fn fragment_lens(input: VertexOutput) -> @location(0) vec4<f32> {
    return warp(input.uv);
}

@fragment
fn fragment_lens_srgb(input: VertexOutput) -> @location(0) vec4<f32> {
    let color = warp(input.uv);
    return vec4<f32>(srgb_encode(color.rgb), color.a);
}

@fragment
fn fragment_copy(input: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(source, source_sampler, input.uv);
}

@fragment
fn fragment_srgb(input: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(source, source_sampler, input.uv);
    return vec4<f32>(srgb_encode(color.rgb), color.a);
}

@fragment
fn fragment_post(input: VertexOutput) -> @location(0) vec4<f32> {
    let viewport_uv = (view.viewport.xy + input.uv * view.viewport.zw) / vec2<f32>(textureDimensions(source));
    return post_color(viewport_uv);
}

@fragment
fn fragment_post_srgb(input: VertexOutput) -> @location(0) vec4<f32> {
    let viewport_uv = (view.viewport.xy + input.uv * view.viewport.zw) / vec2<f32>(textureDimensions(source));
    let color = post_color(viewport_uv);
    return vec4<f32>(srgb_encode(color.rgb), color.a);
}
