// Shared colour, PBR lighting, environment and grading helpers.
fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let low = c / 12.92;
    let high = pow((c + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));
    return select(high, low, c <= vec3<f32>(0.04045));
}

// Perturbs `n` by a tangent-space normal `map_n`, building the tangent frame from screen-space derivatives
// of the position and the texture coordinate (no vertex tangents needed). Texture v grows downward and
// glTF's +Y is up, so the frame is taken on (u, -v).
fn perturb_normal(n: vec3<f32>, p: vec3<f32>, uv: vec2<f32>, map_n: vec3<f32>) -> vec3<f32> {
    let dp1 = dpdx(p);
    let dp2 = dpdy(p);
    let uvf = vec2<f32>(uv.x, -uv.y);
    let duv1 = dpdx(uvf);
    let duv2 = dpdy(uvf);
    let dp2perp = cross(dp2, n);
    let dp1perp = cross(n, dp1);
    let t = dp2perp * duv1.x + dp1perp * duv2.x;
    let b = dp2perp * duv1.y + dp1perp * duv2.y;
    let invmax = inverseSqrt(max(max(dot(t, t), dot(b, b)), 1.0e-20));
    return normalize(mat3x3<f32>(t * invmax, b * invmax, n) * map_n);
}

const PI: f32 = 3.14159265359;

fn distribution_ggx(n_dot_h: f32, alpha: f32) -> f32 {
    let a2 = alpha * alpha;
    let d = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
    return a2 / (PI * d * d);
}

// Height-correlated Smith visibility term (already divided by 4 * n_dot_l * n_dot_v).
fn visibility_smith(n_dot_v: f32, n_dot_l: f32, alpha: f32) -> f32 {
    let a2 = alpha * alpha;
    let gv = n_dot_l * sqrt(n_dot_v * n_dot_v * (1.0 - a2) + a2);
    let gl = n_dot_v * sqrt(n_dot_l * n_dot_l * (1.0 - a2) + a2);
    return 0.5 / max(gv + gl, 0.0001);
}

fn fresnel_schlick(v_dot_h: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (vec3<f32>(1.0) - f0) * pow(clamp(1.0 - v_dot_h, 0.0, 1.0), 5.0);
}

// Outgoing radiance from one light with incoming `radiance` arriving along `l` (toward the light).
fn shade(
    n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, radiance: vec3<f32>,
    diffuse_color: vec3<f32>, f0: vec3<f32>, alpha: f32,
) -> vec3<f32> {
    let n_dot_l = max(dot(n, l), 0.0);
    if (n_dot_l <= 0.0) {
        return vec3<f32>(0.0);
    }
    let h = normalize(v + l);
    let n_dot_v = max(dot(n, v), 0.0001);
    let n_dot_h = max(dot(n, h), 0.0);
    let v_dot_h = max(dot(v, h), 0.0);
    let f = fresnel_schlick(v_dot_h, f0);
    let specular = distribution_ggx(n_dot_h, alpha) * visibility_smith(n_dot_v, n_dot_l, alpha) * f;
    let diffuse = (vec3<f32>(1.0) - f) * diffuse_color / PI;
    return (diffuse + specular) * radiance * n_dot_l;
}

// The authored sky along `dir`: the horizon to zenith and horizon to ground gradients the sky pass draws (same constants), blurred toward
// the sky's average colour as the surface gets rougher. The sun itself is not here: it is a direct light with its own specular.
fn sky_environment(dir: vec3<f32>, roughness: f32) -> vec3<f32> {
    let up = dir.y;
    let upper = mix(view.horizon.rgb, view.zenith.rgb, smoothstep(0.0, 0.85, max(up, 0.0)));
    let lower = mix(view.horizon.rgb, view.ground.rgb, 1.0 - smoothstep(-0.35, 0.0, min(up, 0.0)));
    let sharp = select(lower, upper, up >= 0.0);
    let average = 0.5 * (mix(view.horizon.rgb, view.zenith.rgb, 0.5) + mix(view.horizon.rgb, view.ground.rgb, 0.5));
    return mix(sharp, average, roughness * roughness * 0.7);
}

// A procedural studio, seen from the middle: a blue-grey sky that brightens toward the horizon, a
// dim floor, a large soft light box up and to the front, and a cooler strip light low at the side.
// Rough surfaces blur the boxes (wider, dimmer) toward the average colour, standing in for a prefiltered
// environment map.
fn environment(dir: vec3<f32>, roughness: f32) -> vec3<f32> {
    let blur = roughness * roughness;
    // With an authored sky (the view carries its colours, flagged in horizon.w) surfaces reflect that sky, not the studio.
    if (view.horizon.w > 0.5) {
        return sky_environment(dir, roughness);
    }
    let sky = mix(vec3<f32>(0.62, 0.68, 0.76), vec3<f32>(0.20, 0.32, 0.58), clamp(dir.y, 0.0, 1.0));
    let floor_color = vec3<f32>(0.34, 0.31, 0.28);
    var env = mix(floor_color, sky, smoothstep(-0.2, 0.2, dir.y));
    let key = normalize(vec3<f32>(-0.4, 0.75, 0.55));
    let key_edge = mix(0.93, 0.35, blur);
    env += vec3<f32>(5.0, 4.8, 4.5) * mix(1.0, 0.25, blur) * smoothstep(key_edge, key_edge + 0.05, dot(dir, key));
    let rim = normalize(vec3<f32>(0.85, 0.25, -0.45));
    let rim_edge = mix(0.90, 0.30, blur);
    env += vec3<f32>(1.2, 1.5, 2.0) * mix(1.0, 0.25, blur) * smoothstep(rim_edge, rim_edge + 0.05, dot(dir, rim));
    return mix(env, vec3<f32>(0.40, 0.44, 0.52), blur * 0.6);
}

// The irradiance over pi that the light probes give a surface at `position` facing `n`. The grid holds probe i at texel centre
// (i + 0.5) / dims, so one trilinear sample per coefficient texture blends the eight probes around the point. The lookup is nudged
// off the surface along its normal, so a wall does not borrow light from the probe on its far side, and clamped to the grid.
fn probe_irradiance(position: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
    let spacing = lights.gi_origin_spacing.w;
    let dims = lights.gi_dims.xyz;
    let cell = (position + n * spacing * 0.3 - lights.gi_origin_spacing.xyz) / spacing;
    let uvw = clamp((cell + vec3<f32>(0.5)) / dims, vec3<f32>(0.5) / dims, vec3<f32>(1.0) - vec3<f32>(0.5) / dims);
    let dc = textureSampleLevel(probe_dc, probe_sampler, uvw, 0.0).rgb;
    let x = textureSampleLevel(probe_x, probe_sampler, uvw, 0.0).rgb;
    let y = textureSampleLevel(probe_y, probe_sampler, uvw, 0.0).rgb;
    let z = textureSampleLevel(probe_z, probe_sampler, uvw, 0.0).rgb;
    return max(dc + x * n.x + y * n.y + z * n.z, vec3<f32>(0.0));
}

// Split-sum environment BRDF, Karis's analytic fit: the scale and bias on f0 for a given roughness.
fn env_brdf(f0: vec3<f32>, roughness: f32, n_dot_v: f32) -> vec3<f32> {
    let c0 = vec4<f32>(-1.0, -0.0275, -0.572, 0.022);
    let c1 = vec4<f32>(1.0, 0.0425, 1.04, -0.04);
    let r = roughness * c0 + c1;
    let a004 = min(r.x * r.x, exp2(-9.28 * n_dot_v)) * r.x + r.y;
    let ab = vec2<f32>(-1.04, 1.04) * a004 + r.zw;
    return f0 * ab.x + vec3<f32>(ab.y);
}

// Narkowicz ACES approximation.
fn tonemap_aces(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

// Viewer grading on the tonemapped display-referred colour: contrast around mid grey, saturation
// around Rec. 709 luminance, then a per-channel tint. Mirrors `Grading::grade` in `grade.rs`.
fn grade(color: vec3<f32>) -> vec3<f32> {
    var c = (color - vec3<f32>(0.5)) * view.grading.x + vec3<f32>(0.5);
    let luminance = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
    c = mix(vec3<f32>(luminance), c, view.grading.y);
    return c * view.tint.rgb;
}


// The per-vertex occlusion slot of a static mesh carries three things in one exactly-representable float (an integer
// above 2^14): occlusion in 4 bits and the corner-welded smooth normal as two 10-bit octahedral coordinates. The outline
// pass offsets along that smooth normal so hull faces meet at hard corners. Plain occlusion (0..1, skinned meshes) is
// left as it is and the outline falls back to the face normal.
fn unpack_occlusion(packed: f32) -> f32 {
    if (packed < 2.0) {
        return packed;
    }
    return f32(u32(packed) & 15u) / 15.0;
}

fn outline_normal(packed: f32, normal: vec3<f32>) -> vec3<f32> {
    if (packed < 2.0) {
        return normal;
    }
    let bits = u32(packed) >> 4u;
    let oct = vec2<f32>(f32((bits & 1023u) - 1u), f32(((bits >> 10u) & 1023u) - 1u)) / 1021.0 * 2.0 - 1.0;
    var n = vec3<f32>(oct.x, oct.y, 1.0 - abs(oct.x) - abs(oct.y));
    if (n.z < 0.0) {
        let folded = (vec2<f32>(1.0) - abs(vec2<f32>(n.y, n.x))) * vec2<f32>(select(-1.0, 1.0, n.x >= 0.0), select(-1.0, 1.0, n.y >= 0.0));
        n = vec3<f32>(folded.x, folded.y, n.z);
    }
    return normalize(n);
}

