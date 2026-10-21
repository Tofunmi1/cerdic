// Applies the occlusion to the lit picture: the average of the 4 by 4 pixels around each pixel (which cancels the `ao` pass's jitter
// pattern), weighted down where a neighbour's depth differs from the centre's so occlusion does not bleed across a silhouette. The
// pipeline's blend multiplies the colour already in the target by this output.

@group(0) @binding(0) var normals: texture_2d<f32>;
@group(0) @binding(1) var occlusion: texture_2d<f32>;

struct Region {
    viewport: vec4<f32>,
};
@group(0) @binding(2) var<uniform> region: Region;

// How much the occlusion darkens the picture (1 would black out a fully closed-in pixel).
const AO_STRENGTH: f32 = 0.9;

@vertex
fn vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let x = f32((index << 1u) & 2u);
    let y = f32(index & 2u);
    return vec4<f32>(x * 2.0 - 1.0, y * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn fragment(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let center = vec2<i32>(frag.xy);
    let vmin = vec2<i32>(region.viewport.xy);
    let vmax = vec2<i32>(region.viewport.xy + region.viewport.zw) - vec2<i32>(1);
    let inside = all(center >= vmin) && all(center <= vmax);
    let center_depth = textureLoad(normals, center, 0).w;
    if (center_depth <= 0.0 || !inside) {
        return vec4<f32>(1.0);
    }
    var sum = 0.0;
    var total = 0.0;
    for (var y = -2; y < 2; y += 1) {
        for (var x = -2; x < 2; x += 1) {
            let pos = clamp(center + vec2<i32>(x, y), vmin, vmax);
            let d = textureLoad(normals, pos, 0).w;
            // Reverse-Z depth is inversely proportional to distance, so the ratio is the relative distance.
            let weight = select(0.0, clamp(1.0 - abs(center_depth / d - 1.0) * 20.0, 0.0, 1.0), d > 0.0);
            sum += textureLoad(occlusion, pos, 0).x * weight;
            total += weight;
        }
    }
    let visibility = select(1.0, sum / total, total > 0.0);
    return vec4<f32>(vec3<f32>(mix(1.0, visibility, AO_STRENGTH)), 1.0);
}
