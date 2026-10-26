struct TerrainParams {
    layers: array<vec4<f32>, 4>,
    a: vec4<f32>, // rock slope/blend, snow line/blend
    b: vec4<f32>, // sand line/blend, variation, relief
    c: vec4<f32>, // relief scale, enabled
};
struct WaterWave {
    spatial: vec4<f32>,
    temporal: vec4<f32>,
};
struct WaterWaves {
    sim_bounds: vec4<f32>,
    sim_enabled: vec4<f32>,
    waves: array<WaterWave, 12>,
    detail: array<WaterWave, 4>,
    shallow: vec4<f32>,
    deep: vec4<f32>,
    params: vec4<f32>,
    terrain: TerrainParams,
    grass: GrassParams,
};
