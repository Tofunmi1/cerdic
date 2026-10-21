// Ground-truth ambient occlusion (Jimenez et al. 2016), structured after Intel's XeGTAO (MIT). It is the engine's own pass from
// `ve-render` moved onto Bevy's prepass for the WebGL2 build, where Bevy's compute-based SSAO does not exist.
//
// A full-screen pass: per pixel, a few screen-space slices through the pixel, each searched for its highest horizon on both sides, and
// the cosine-weighted visible arc between them. The prepass's normal texture holds the world normal in rgb and the fragment depth in alpha
// (Bevy patch 4), 0 where only the sky is. The slice angle and step offsets are jittered per pixel with a 4 by 4 pattern, which the apply
// pass's 4 by 4 average cancels.
//
// The result is one float per pixel: 1 where the sky is fully visible, down to 0 where the surface is closed in.

@group(0) @binding(0) var normals: texture_2d<f32>;

struct AoView {
    clip_from_world: mat4x4<f32>,
    world_from_clip: mat4x4<f32>,
    camera_position: vec4<f32>,
    viewport: vec4<f32>,
};
@group(0) @binding(1) var<uniform> view: AoView;

const PI: f32 = 3.14159265;
const AO_SLICES: i32 = 3;
const AO_STEPS: i32 = 6;
// World radius in metres inside which geometry occludes, and how much of the radius fades out at the edge.
const AO_RADIUS: f32 = 1.2;
const AO_FADE: f32 = 0.4;
const AO_POWER: f32 = 1.4;
const AO_MAX_PIXELS: f32 = 96.0;

@vertex
fn vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    // One triangle covering the screen.
    let x = f32((index << 1u) & 2u);
    let y = f32(index & 2u);
    return vec4<f32>(x * 2.0 - 1.0, y * 2.0 - 1.0, 0.0, 1.0);
}

// The reverse-Z depth at a pixel (1 is the near plane, 0 the far plane and the sky).
fn depth_at(pixel: vec2<i32>) -> f32 {
    return textureLoad(normals, pixel, 0).w;
}

// The world position of a pixel at a reverse-Z depth, inside the view's viewport.
fn world_at(pixel: vec2<i32>, depth: f32) -> vec3<f32> {
    let uv = (vec2<f32>(pixel) + 0.5 - view.viewport.xy) / view.viewport.zw;
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let h = view.world_from_clip * vec4<f32>(ndc, depth, 1.0);
    return h.xyz / h.w;
}

// The pixel (in target coordinates) a world position lands on.
fn pixel_of(world: vec3<f32>) -> vec2<f32> {
    let c = view.clip_from_world * vec4<f32>(world, 1.0);
    let ndc = c.xy / c.w;
    return view.viewport.xy + vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5) * view.viewport.zw;
}

@fragment
fn fragment(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let center = vec2<i32>(frag.xy);
    let sample = textureLoad(normals, center, 0);
    let depth = sample.w;
    // Outside this view's viewport (the other eye) and the sky stay open.
    let inside = all(frag.xy >= view.viewport.xy) && all(frag.xy < view.viewport.xy + view.viewport.zw);
    if (depth <= 0.0 || !inside) {
        return vec4<f32>(1.0);
    }
    let vmin = vec2<i32>(view.viewport.xy);
    let vmax = vec2<i32>(view.viewport.xy + view.viewport.zw) - vec2<i32>(1);
    let p = world_at(center, depth);
    let v = normalize(view.camera_position.xyz - p);
    var n = normalize(sample.xyz * 2.0 - 1.0);
    n = select(-n, n, dot(n, v) > 0.0);

    // The camera's right and up directions in the world, from two points on one plane of constant depth.
    let a = view.world_from_clip * vec4<f32>(-1.0, 0.0, 0.5, 1.0);
    let b = view.world_from_clip * vec4<f32>(1.0, 0.0, 0.5, 1.0);
    let right = normalize(b.xyz / b.w - a.xyz / a.w);
    let c = view.world_from_clip * vec4<f32>(0.0, -1.0, 0.5, 1.0);
    let e = view.world_from_clip * vec4<f32>(0.0, 1.0, 0.5, 1.0);
    let up = normalize(e.xyz / e.w - c.xyz / c.w);

    // How far the world radius reaches on screen, in pixels.
    let centre_px = vec2<f32>(center) + 0.5;
    let radius_px = clamp(length(pixel_of(p + right * AO_RADIUS) - centre_px), 3.0, AO_MAX_PIXELS);
    // A 4 by 4 pattern of slice angles and step offsets that the apply pass's 4 by 4 average cancels exactly.
    let cell = vec2<u32>(vec2<i32>(center.x & 3, center.y & 3));
    let index = cell.y * 4u + cell.x;
    let noise = f32((index * 5u) % 16u) / 16.0 + 0.03125;
    let jitter = f32((index * 7u + 3u) % 16u) / 16.0 + 0.03125;

    var visibility = 0.0;
    for (var slice = 0; slice < AO_SLICES; slice += 1) {
        let phi = (f32(slice) + noise) / f32(AO_SLICES) * PI;
        let screen = vec2<f32>(cos(phi), sin(phi));
        // The slice direction in the world, and the pixel step (screen y runs down).
        let direction = right * screen.x + up * screen.y;
        let step_px = vec2<f32>(screen.x, -screen.y);
        // The plane of the slice holds the view vector and the direction; the normal projected into it gives the
        // angle of the surface within the slice.
        let axis = normalize(cross(direction, v));
        let projected = n - axis * dot(n, axis);
        let projected_length = length(projected);
        let ortho = direction - v * dot(direction, v);
        let cos_n = clamp(dot(projected, v) / max(projected_length, 1e-5), 0.0, 1.0);
        let angle_n = select(-1.0, 1.0, dot(ortho, projected) > 0.0) * acos(cos_n);
        let low_0 = cos(angle_n + PI * 0.5);
        let low_1 = cos(angle_n - PI * 0.5);
        var horizon_0 = low_0;
        var horizon_1 = low_1;
        for (var k = 0; k < AO_STEPS; k += 1) {
            let t = (f32(k) + jitter) / f32(AO_STEPS);
            let distance_px = max(t * t * radius_px, 1.5 + f32(k));
            let offset = vec2<i32>(round(step_px * distance_px));
            let pos_0 = clamp(center + offset, vmin, vmax);
            let pos_1 = clamp(center - offset, vmin, vmax);
            let d_0 = depth_at(pos_0);
            let d_1 = depth_at(pos_1);
            if (d_0 > 0.0) {
                let delta = world_at(pos_0, d_0) - p;
                let len = max(length(delta), 1e-4);
                let weight = clamp((AO_RADIUS - len) / (AO_RADIUS * AO_FADE), 0.0, 1.0);
                horizon_0 = max(horizon_0, mix(low_0, dot(delta, v) / len, weight));
            }
            if (d_1 > 0.0) {
                let delta = world_at(pos_1, d_1) - p;
                let len = max(length(delta), 1e-4);
                let weight = clamp((AO_RADIUS - len) / (AO_RADIUS * AO_FADE), 0.0, 1.0);
                horizon_1 = max(horizon_1, mix(low_1, dot(delta, v) / len, weight));
            }
        }
        // The two horizon angles, limited to the hemisphere around the surface normal, and the visible arc between.
        let h_0 = angle_n + clamp(-acos(clamp(horizon_1, -1.0, 1.0)) - angle_n, -PI * 0.5, PI * 0.5);
        let h_1 = angle_n + clamp(acos(clamp(horizon_0, -1.0, 1.0)) - angle_n, -PI * 0.5, PI * 0.5);
        let arc_0 = (cos_n + 2.0 * h_0 * sin(angle_n) - cos(2.0 * h_0 - angle_n)) * 0.25;
        let arc_1 = (cos_n + 2.0 * h_1 * sin(angle_n) - cos(2.0 * h_1 - angle_n)) * 0.25;
        visibility += projected_length * (arc_0 + arc_1);
    }
    return vec4<f32>(pow(clamp(visibility / f32(AO_SLICES), 0.0, 1.0), AO_POWER));
}
