// Baked maps of textured surfaces, one layer each: albedo (sRGB), normal, metallic-roughness, emissive (sRGB). Three arrays by
// size: 1024, 512 and 256 texels square. A map is named by an id, tier * 4096 + layer (tier 0 is the 1024 array).
@group(2) @binding(0) var maps_large: texture_2d_array<f32>;
@group(2) @binding(1) var maps_medium: texture_2d_array<f32>;
@group(2) @binding(2) var maps_small: texture_2d_array<f32>;
@group(2) @binding(3) var material_sampler: sampler;

// Reads a baked map by id with the given texture-coordinate derivatives, so it is safe in non-uniform control flow (the id differs
// between instances). A negative id (no map) gives zero without a fetch.
fn sample_map(id: f32, uv: vec2<f32>, dx: vec2<f32>, dy: vec2<f32>) -> vec4<f32> {
    if (id < 0.0) {
        return vec4<f32>(0.0);
    }
    let tier = floor(id / 4096.0);
    let layer = i32(id - tier * 4096.0);
    if (tier < 0.5) {
        return textureSampleGrad(maps_large, material_sampler, uv, layer, dx, dy);
    }
    if (tier < 1.5) {
        return textureSampleGrad(maps_medium, material_sampler, uv, layer, dx, dy);
    }
    return textureSampleGrad(maps_small, material_sampler, uv, layer, dx, dy);
}

