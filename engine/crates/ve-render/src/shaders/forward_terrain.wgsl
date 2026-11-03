// Terrain layers, shared with the Bevy terrain extension. Colours are already linear.


fn terrain_noise(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = mix(mix(terrain_hash(i), terrain_hash(i + vec3<f32>(1.0, 0.0, 0.0)), u.x), mix(terrain_hash(i + vec3<f32>(0.0, 1.0, 0.0)), terrain_hash(i + vec3<f32>(1.0, 1.0, 0.0)), u.x), u.y);
    let b = mix(mix(terrain_hash(i + vec3<f32>(0.0, 0.0, 1.0)), terrain_hash(i + vec3<f32>(1.0, 0.0, 1.0)), u.x), mix(terrain_hash(i + vec3<f32>(0.0, 1.0, 1.0)), terrain_hash(i + vec3<f32>(1.0, 1.0, 1.0)), u.x), u.y);
    return mix(a, b, u.z);
}

fn terrain_hash(p: vec3<f32>) -> f32 {
    var q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    q += dot(q, q.yzx + 33.33);
    return fract((q.x + q.y) * q.z);
}

fn terrain_fbm(p: vec3<f32>) -> f32 {
    return terrain_noise(p) * 0.5 + terrain_noise(p * 2.03) * 0.3 + terrain_noise(p * 4.1) * 0.2;
}

fn terrain_rock(p: vec3<f32>) -> f32 {
    return (1.0 - abs(2.0 * terrain_noise(p) - 1.0)) * 0.65
        + (1.0 - abs(2.0 * terrain_noise(p * 2.1) - 1.0)) * 0.35;
}

fn terrain_raw_detail(p: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(terrain_fbm(p * 0.35), terrain_rock(p * 0.9), terrain_rock(p));
}

fn terrain_detail(p: vec3<f32>, scale: f32) -> vec3<f32> {
    if (scale <= 0.0) { return terrain_raw_detail(p); }
    // World XZ identifies vertices, so adjacent tiles use precisely the same shifted layers.
    let cells = stochastic_cells(p.xz, scale, 0u);
    return terrain_raw_detail(vec3<f32>(cells[0].uv.x, p.y, cells[0].uv.y)) * cells[0].weight
        + terrain_raw_detail(vec3<f32>(cells[1].uv.x, p.y, cells[1].uv.y)) * cells[1].weight
        + terrain_raw_detail(vec3<f32>(cells[2].uv.x, p.y, cells[2].uv.y)) * cells[2].weight;
}

fn terrain_surface(p: vec3<f32>, n: vec3<f32>, scale: f32, t: TerrainParams) -> vec4<f32> {
    if (t.c.y < 0.5) { return vec4<f32>(0.0); }
    let slope = acos(clamp(n.y, 0.0, 1.0)) / 1.5707963;
    let broad = terrain_fbm(vec3<f32>(p.x, 0.0, p.z) * 0.012);
    let detail = terrain_detail(p, scale);
    let drift = (broad - 0.5) * t.b.z;
    // Rock breaks through the grass along the crests of the rock noise, not along a clean slope line (the more variation, the more ragged).
    let rock = smoothstep(t.a.x - t.a.y, t.a.x + t.a.y, slope + drift * 0.3 + (detail.y - 0.5) * 0.3 * t.b.z);
    let snow = smoothstep(t.a.z - t.a.w, t.a.z + t.a.w, p.y + drift * 40.0) * (1.0 - smoothstep(t.a.x * 0.6, t.a.x * 1.1, slope));
    let sand = (1.0 - smoothstep(t.b.x - t.b.y, t.b.x + t.b.y, p.y + drift * 6.0)) * (1.0 - rock);
    // Two slow tints, strongest on flat ground, so a wide field does not read as one repeated colour.
    let macro_a = terrain_noise(vec3<f32>(p.x, 0.0, p.z) * 0.0045);
    let macro_b = terrain_noise(vec3<f32>(p.z, 0.0, p.x) * 0.0081 + vec3<f32>(7.3, 0.0, 0.0));
    let macro_tint = mix(vec3<f32>(1.0), vec3<f32>(1.14, 1.04, 0.84), macro_a) * mix(vec3<f32>(1.0), vec3<f32>(0.9, 1.0, 1.1), macro_b);
    let macro_color = mix(vec3<f32>(1.0), macro_tint, t.b.z * (1.0 - rock));
    let grass_c = t.layers[0].rgb * (0.78 + 0.44 * detail.x) * (0.9 + 0.2 * broad) * macro_color;
    let rock_c = t.layers[1].rgb * (0.7 + 0.6 * detail.y);
    let snow_c = t.layers[2].rgb * (0.94 + 0.06 * detail.x);
    let sand_c = t.layers[3].rgb * (0.88 + 0.24 * detail.x) * mix(vec3<f32>(1.0), macro_color, 0.5);
    let color = mix(mix(mix(grass_c, sand_c, sand), rock_c, rock), snow_c, snow);
    let roughness = mix(mix(mix(t.layers[0].w, t.layers[3].w, sand), t.layers[1].w, rock), t.layers[2].w, snow);
    return vec4<f32>(color, roughness);
}
