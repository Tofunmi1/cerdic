// The one wind function, put in front of the forward, shadow and depth shaders when they are built (see `wind.rs`), so the picture and
// the shadows bend the same way. It reads `view.wind`, which each of those shaders declares as part of its view uniform.
//
// How far the wind moves a foliage vertex, in world metres. `origin` is the instance's origin, `world` the vertex, `local` its
// position in the shape's own space and `foliage` the instance's (cutoff, sway, flutter). Sway bends the whole shape downwind
// more the higher the vertex is above the origin (a gust travels across the ground, so neighbours are out of step); flutter
// trembles it at a small scale. No sway and no flutter, or no wind, gives zero.
fn wind_displacement(origin: vec3<f32>, world: vec3<f32>, local: vec3<f32>, foliage: vec4<f32>) -> vec3<f32> {
    let strength = view.wind.z;
    if ((foliage.y <= 0.0 && foliage.z <= 0.0) || strength <= 0.0) {
        return vec3<f32>(0.0);
    }
    let t = view.wind.w;
    let direction = vec3<f32>(view.wind.x, 0.0, view.wind.y);
    let height = clamp(world.y - origin.y, 0.0, 6.0);
    let phase = dot(origin.xz, view.wind.xy) * 0.35;
    let gust = 0.6 + 0.4 * sin(t * 1.3 + phase) + 0.25 * sin(t * 2.9 + phase * 1.7);
    let side = vec3<f32>(-direction.z, 0.0, direction.x) * 0.3 * sin(t * 1.9 + phase * 0.6);
    var offset = (direction * gust + side) * (foliage.y * strength * 0.06 * height * height);
    // The shape swings rather than shears: the more it leans, the lower it sits.
    offset.y = -dot(offset, offset) * 0.25 / max(height, 0.5);
    let tremble = vec3<f32>(
        sin(t * 9.0 + local.x * 7.0 + local.z * 3.0),
        sin(t * 11.0 + local.y * 6.0 + local.x * 2.0),
        sin(t * 10.0 + local.z * 8.0 + local.y * 4.0),
    );
    return offset + tremble * (foliage.z * 0.03 * (0.3 + strength));
}
