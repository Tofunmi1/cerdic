"""Headless Blender asset job: parametric locker with named parts -> UV -> bake PBR -> GLB.
Run: Blender -b --factory-startup --python job.py -- out/locker.glb
The job spec is what an LLM would emit (data, not code)."""
import bpy, bmesh, json, sys, time
from mathutils import Vector

SPEC = {
    "archetype": "locker",
    "params": {"width": 0.6, "depth": 0.5, "height": 1.8, "door_gap": 0.006, "vent_rows": 5},
    "material": {"kind": "painted_metal", "base_color": [0.18, 0.32, 0.42], "wear": 0.35, "roughness": 0.45},
    "texture_size": 512,
}
out_path = sys.argv[sys.argv.index("--") + 1]
t = {}
def mark(name, start): t[name] = round(time.perf_counter() - start, 3)

start = time.perf_counter()
bpy.ops.wm.read_factory_settings(use_empty=True)
p = SPEC["params"]

def box(name, size, loc, parent=None):
    mesh = bpy.data.meshes.new(name)
    bm = bmesh.new()
    bmesh.ops.create_cube(bm, size=1.0)
    bmesh.ops.scale(bm, vec=Vector(size), verts=bm.verts)
    bm.to_mesh(mesh); bm.free()
    obj = bpy.data.objects.new(name, mesh)
    bpy.context.scene.collection.objects.link(obj)
    obj.location = loc
    if parent: obj.parent = parent
    return obj

# Body: 5-sided shell (open front) built from boxes; door is a separate named part with its pivot at the hinge.
w, d, h, g = p["width"], p["depth"], p["height"], p["door_gap"]
th = 0.02
root = bpy.data.objects.new("locker", None); bpy.context.scene.collection.objects.link(root)
parts = [
    box("body_back", (w, th, h), (0, -d / 2 + th / 2, h / 2), root),
    box("body_left", (th, d, h), (-w / 2 + th / 2, 0, h / 2), root),
    box("body_right", (th, d, h), (w / 2 - th / 2, 0, h / 2), root),
    box("body_top", (w, d, th), (0, 0, h - th / 2), root),
    box("body_bottom", (w, d, th), (0, 0, th / 2), root),
]
hinge = bpy.data.objects.new("door_hinge", None); bpy.context.scene.collection.objects.link(hinge)
hinge.parent = root; hinge.location = (-w / 2, d / 2, 0)
door = box("door", (w - 2 * g, th, h - 2 * g), (w / 2, -th / 2, h / 2), hinge)
for i in range(p["vent_rows"]):
    box(f"door_vent_{i}", (w * 0.5, 0.004, 0.012), (w / 2, 0.002, h * 0.78 + i * 0.03), hinge)
handle = box("door_handle", (0.02, 0.03, 0.12), (w * 0.85, 0.02, h * 0.5), hinge)
meshes = [o for o in bpy.data.objects if o.type == "MESH"]
for o in meshes:
    bev = o.modifiers.new("bevel", "BEVEL"); bev.width = 0.003; bev.segments = 2
mark("build_parametric", start)

# Procedural material (what the LLM chose) -> nodes.
s = time.perf_counter()
m = SPEC["material"]
mat = bpy.data.materials.new("painted_metal"); mat.use_nodes = True
nt = mat.node_tree; bsdf = nt.nodes["Principled BSDF"]
noise = nt.nodes.new("ShaderNodeTexNoise"); noise.inputs["Scale"].default_value = 18
ramp = nt.nodes.new("ShaderNodeValToRGB")
ramp.color_ramp.elements[0].position = 1 - m["wear"]
ramp.color_ramp.elements[0].color = (*m["base_color"], 1)
ramp.color_ramp.elements[1].color = (0.55, 0.55, 0.55, 1)
nt.links.new(noise.outputs["Fac"], ramp.inputs["Fac"])
nt.links.new(ramp.outputs["Color"], bsdf.inputs["Base Color"])
bsdf.inputs["Roughness"].default_value = m["roughness"]
bsdf.inputs["Metallic"].default_value = 0.6
for o in meshes: o.data.materials.append(mat)
mark("material_nodes", s)

# Join into one bake target per logical part group, UV unwrap, bake base color to an image.
s = time.perf_counter()
bpy.context.view_layer.update()
for o in meshes:
    bpy.context.view_layer.objects.active = o
    o.select_set(True)
    bpy.ops.object.modifier_apply(modifier="bevel")
    o.select_set(False)
bpy.ops.object.select_all(action="DESELECT")
for o in meshes: o.select_set(True)
bpy.context.view_layer.objects.active = door
bpy.ops.object.mode_set(mode="EDIT"); bpy.ops.mesh.select_all(action="SELECT")
bpy.ops.uv.smart_project(angle_limit=1.15, island_margin=0.02)
bpy.ops.object.mode_set(mode="OBJECT")
mark("apply_bevel_and_uv", s)

s = time.perf_counter()
size = SPEC["texture_size"]
img = bpy.data.images.new("locker_basecolor", size, size)
tex = nt.nodes.new("ShaderNodeTexImage"); tex.image = img; nt.nodes.active = tex
sc = bpy.context.scene; sc.render.engine = "CYCLES"; sc.cycles.device = "CPU"; sc.cycles.samples = 4
bpy.ops.object.bake(type="DIFFUSE", pass_filter={"COLOR"}, margin=4)
img.filepath_raw = out_path.replace(".glb", "_basecolor.png"); img.file_format = "PNG"; img.save()
# Swap the procedural graph for the baked texture so the GLB carries a real PBR texture.
nt.links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
mark("bake_basecolor", s)

s = time.perf_counter()
bpy.ops.export_scene.gltf(filepath=out_path, export_format="GLB", use_selection=False, export_apply=True)
mark("export_glb", s)

import os
tris = sum(sum(len(p.vertices) - 2 for p in o.data.polygons) for o in meshes)
print("RESULT " + json.dumps({"timings_s": t, "total_s": round(time.perf_counter() - start, 3),
      "objects": [o.name for o in bpy.data.objects], "triangles": tris,
      "glb_bytes": os.path.getsize(out_path)}))
