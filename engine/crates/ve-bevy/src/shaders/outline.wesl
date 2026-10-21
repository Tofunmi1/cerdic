// The toon outline: the same mesh drawn again with front faces culled, every vertex pushed outward along its smooth normal by a fixed
// number of screen pixels, so the line keeps its width at any distance. Ported from the forward renderer's `vertex_outline`.

import bevy_pbr::render::{
    mesh_functions,
    mesh_view_bindings::view,
    skinning,
};

// x,y,z the line colour (linear), w its width in screen pixels.
struct Outline {
    color: vec4<f32>,
};
@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> outline: Outline;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    // Static meshes carry the corner-welded smooth normal here, packed with the baked occlusion; skinned ones carry plain occlusion.
    @location(2) occlusion: f32,
    // A GPU skinned mesh also carries its joints and weights (the same locations Bevy's own mesh vertex uses).
    @if(SKINNED)
    @location(6) joint_indices: vec4<u32>,
    @if(SKINNED)
    @location(7) joint_weights: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
};

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

@vertex
fn vertex(input: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let width = outline.color.w;
@if(SKINNED)
    let world_from_local = skinning::skin_model(input.joint_indices, input.joint_weights, input.instance_index);
@else
    let world_from_local = mesh_functions::get_world_from_local(input.instance_index);
    let world = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(input.position, 1.0));
@if(SKINNED)
    let world_normal = skinning::skin_normals(world_from_local, outline_normal(input.occlusion, input.normal));
@else
    let world_normal = normalize(mesh_functions::mesh_normal_local_to_world(outline_normal(input.occlusion, input.normal), input.instance_index));
    let clip = view.clip_from_world * world;
    // A vertex at or behind the camera has no sensible screen offset (it scales with w and flips sign); drop it, so a part passing close to
    // the camera loses its outline there instead of throwing a huge black hull across the screen.
    if (clip.w < 0.15) {
        out.clip_position = vec4<f32>(2.0, 2.0, 2.0, 1.0);
        return out;
    }
    let size = max(view.viewport.zw, vec2<f32>(1.0));
    let direction_clip = (view.clip_from_world * vec4<f32>(world_normal, 0.0)).xy;
    let direction_pixels = direction_clip * size;
    let length_pixels = length(direction_pixels);
    var offset = vec2<f32>(0.0);
    if (length_pixels > 1.0e-6) {
        offset = direction_pixels / length_pixels / size * 2.0 * width * clip.w;
    }
    out.clip_position = vec4<f32>(clip.xy + offset, clip.z, clip.w);
    return out;
}

@fragment
fn fragment(input: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(outline.color.rgb, 1.0);
}
