struct TerrainParams {
    layers: array<vec4<f32>, 4>,
    a: vec4<f32>, // rock slope/blend, snow line/blend
    b: vec4<f32>, // sand line/blend, variation, relief
    c: vec4<f32>, // relief scale, enabled
};
