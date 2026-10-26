// Procedural recipes and shadow sampling.
fn hash3(p: vec3<f32>) -> f32 {
    return fract(sin(dot(p, vec3<f32>(127.1, 311.7, 74.7))) * 43758.5453);
}

fn noise3(p: vec3<f32>) -> f32 {
    let cell = floor(p);
    let f = fract(p);
    let blend = f * f * (vec3<f32>(3.0) - 2.0 * f);
    let x00 = mix(hash3(cell), hash3(cell + vec3<f32>(1.0, 0.0, 0.0)), blend.x);
    let x10 = mix(hash3(cell + vec3<f32>(0.0, 1.0, 0.0)), hash3(cell + vec3<f32>(1.0, 1.0, 0.0)), blend.x);
    let x01 = mix(hash3(cell + vec3<f32>(0.0, 0.0, 1.0)), hash3(cell + vec3<f32>(1.0, 0.0, 1.0)), blend.x);
    let x11 = mix(hash3(cell + vec3<f32>(0.0, 1.0, 1.0)), hash3(cell + vec3<f32>(1.0, 1.0, 1.0)), blend.x);
    return mix(mix(x00, x10, blend.y), mix(x01, x11, blend.y), blend.z);
}

// Returns albedo multiplier, roughness offset and a low-amplitude height field.
fn procedural_surface(recipe: f32, position: vec3<f32>, seed: f32) -> vec3<f32> {
    if (recipe < 0.5 || (recipe > 9.5 && recipe < 10.5)) {
        return vec3<f32>(1.0, 0.0, 0.0);
    }
    let fine = noise3(position * 3.0);
    let broad = noise3(position * 0.7);
    let instance = fract(sin(seed * 91.17 + 17.13) * 43758.5453);
    var shade = 0.92 + instance * 0.16;
    var roughness = 0.0;
    var height = (fine - 0.5) * 0.012;
    if (recipe < 1.5) {
        let grain = sin(position.z * 24.0 + broad * 5.0);
        shade *= 0.84 + 0.16 * (0.5 + 0.5 * grain);
        height += grain * 0.008;
        roughness = (fine - 0.5) * 0.12;
    } else if (recipe < 2.5) {
        let seam = step(0.965, fract(position.z * 1.4 + floor(position.x * 0.35) * 0.5));
        let board = 0.92 + 0.16 * fract(sin(floor(position.x * 0.35) * 19.19 + floor(position.z * 1.4) * 7.7) * 43758.5453);
        shade *= mix(board * (0.92 + 0.12 * fine), 0.28, seam);
        height += seam * -0.035;
        roughness = (fine - 0.5) * 0.16;
    } else if (recipe < 3.5) {
        let brush = noise3(position * vec3<f32>(2.0, 36.0, 2.0));
        shade *= 0.88 + 0.12 * brush;
        height += (brush - 0.5) * 0.003;
        roughness = (brush - 0.5) * 0.18;
    } else if (recipe < 4.5) {
        let pebble = smoothstep(0.38, 0.72, fine);
        shade *= 0.83 + 0.24 * pebble;
        height += (pebble - 0.5) * 0.018;
        roughness = (fine - 0.5) * 0.1;
    } else if (recipe < 5.5) {
        let weave = 0.5 + 0.25 * sin(position.x * 38.0) + 0.25 * sin(position.z * 38.0);
        shade *= 0.82 + weave * 0.22;
        height += (weave - 0.5) * 0.012;
        roughness = 0.08 + (fine - 0.5) * 0.08;
    } else if (recipe < 6.5) {
        let row = floor(position.y * 1.8);
        let seam_x = step(0.94, fract(position.x * 1.2 + fract(row * 0.5)));
        let seam_y = step(0.93, fract(position.y * 1.8));
        let mortar = max(seam_x, seam_y);
        shade *= mix(0.86 + fine * 0.22, 0.32, mortar);
        height += mortar * -0.028;
        roughness = 0.08 + fine * 0.12;
    } else if (recipe < 7.5) {
        shade *= 0.86 + 0.22 * fine + 0.08 * broad;
        height += (fine - 0.5) * 0.016;
        roughness = 0.12 + fine * 0.12;
    } else if (recipe < 8.5) {
        let vein = abs(sin(position.x * 5.0 + position.y * 2.0 + broad * 8.0));
        let marble = 1.0 - smoothstep(0.0, 0.16, vein);
        shade *= 0.82 + 0.22 * fine + 0.12 * marble;
        height += marble * 0.008;
        roughness = (fine - 0.5) * 0.08;
    } else {
        shade *= 0.96 + 0.08 * fine;
        height += (fine - 0.5) * 0.004;
        roughness = (fine - 0.5) * 0.05;
    }
    return vec3<f32>(shade, roughness, height);
}

// Fraction of the sun that reaches this point: 1 in the open, 0 in full shadow. Looks up the sun's
// shadow map with a comparison sampler (a hardware-filtered tap, or a grid of them), pushing the
// lookup along the normal so surfaces do not shadow themselves. Level 0 sampling needs no derivatives,
// so the early returns are fine.
fn cascade_split(index: i32) -> f32 {
    if (index == 0) { return lights.shadow_splits.x; }
    if (index == 1) { return lights.shadow_splits.y; }
    return lights.shadow_splits.z;
}

fn cascade_bias(index: i32) -> f32 {
    if (index == 0) { return lights.shadow_bias.x; }
    if (index == 1) { return lights.shadow_bias.y; }
    return lights.shadow_bias.z;
}

// Visibility of one point in one cascade of the atlas (the cascades sit side by side), or 1 outside its box.
fn cascade_visibility(index: i32, count: f32, world_position: vec3<f32>, normal: vec3<f32>) -> f32 {
    let biased = world_position + normal * cascade_bias(index);
    let clip = lights.sun_shadow_from_world[index] * vec4<f32>(biased, 1.0);
    let ndc = clip.xyz / clip.w;
    if (abs(ndc.x) > 1.0 || abs(ndc.y) > 1.0 || ndc.z > 1.0) {
        return 1.0;
    }
    let uv = vec2<f32>((ndc.x * 0.5 + 0.5 + f32(index)) / count, 0.5 - ndc.y * 0.5);
    let radius = i32(lights.shadow_params.y);
    // Keep the filter taps inside this cascade's tile, so they never read a neighbour's depths.
    let low = vec2<f32>(f32(index) / count + lights.shadow_params.z * f32(radius), lights.shadow_params.w * f32(radius));
    let high = vec2<f32>(f32(index + 1) / count - lights.shadow_params.z * f32(radius), 1.0 - lights.shadow_params.w * f32(radius));
    let centre = clamp(uv, low, high);
    var lit = 0.0;
    for (var dy = -radius; dy <= radius; dy = dy + 1) {
        for (var dx = -radius; dx <= radius; dx = dx + 1) {
            let offset = vec2<f32>(f32(dx) * lights.shadow_params.z, f32(dy) * lights.shadow_params.w);
            lit += textureSampleCompareLevel(shadow_map, shadow_sampler, centre + offset, ndc.z);
        }
    }
    let side = f32(2 * radius + 1);
    return lit / (side * side);
}

// Cascaded sun shadow. The cascade is chosen by distance along the view; the last tenth of each cascade blends into the next, and the
// last cascade fades to light at its far edge, so no seam or cut-off shows. Level 0 sampling needs no derivatives, so the early
// returns are fine.
fn sun_visibility(world_position: vec3<f32>, normal: vec3<f32>) -> f32 {
    let count = lights.shadow_params.x;
    if (count < 0.5) {
        return 1.0;
    }
    let depth = dot(world_position - view.camera_position.xyz, lights.shadow_forward.xyz);
    let last = i32(count) - 1;
    var cascade = 0;
    for (var i = 0; i < last; i = i + 1) {
        if (depth > cascade_split(i)) {
            cascade = i + 1;
        }
    }
    var visibility = cascade_visibility(cascade, count, world_position, normal);
    let far = cascade_split(cascade);
    let blend = smoothstep(far * 0.9, far, depth);
    if (cascade < last) {
        if (blend > 0.0) {
            visibility = mix(visibility, cascade_visibility(cascade + 1, count, world_position, normal), blend);
        }
    } else {
        visibility = mix(visibility, 1.0, blend);
    }
    return visibility;
}

// Cel lighting: how far the surface is into the lit side, in `bands` flat steps. `x` is N.L plus the authored shift, from -1
// to 1; the first edge sits where light just grazes the surface (x = 0) and the others are evenly spaced toward full light.
// `feather` softens each edge. Returns 0 (all shadow colour) to 1 (all lit colour).
fn toon_step(x: f32, bands: f32, feather: f32) -> f32 {
    let half_width = max(feather * 0.25, 0.004);
    var total = 0.0;
    for (var k = 0; k < 4; k = k + 1) {
        if (f32(k) < bands) {
            let edge = f32(k) / bands;
            total = total + smoothstep(edge - half_width, edge + half_width, x);
        }
    }
    return total / max(bands, 1.0);
}


fn sample_stochastic_map(id: f32, uv: vec2<f32>, dx: vec2<f32>, dy: vec2<f32>, cells: array<StochasticSample, 3>, enabled: bool) -> vec4<f32> {
    if (!enabled) { return sample_map(id, uv, dx, dy); }
    // Every shifted read uses the original footprint, including at triangle boundaries.
    return sample_map(id, cells[0].uv, dx, dy) * cells[0].weight
        + sample_map(id, cells[1].uv, dx, dy) * cells[1].weight
        + sample_map(id, cells[2].uv, dx, dy) * cells[2].weight;
}

