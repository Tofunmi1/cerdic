#!/usr/bin/env python3
"""Converts the CC0 MPFB2 / Anny human into our compact `human.pack` (ADR 0055). Run once; the pack, not this script's input, is what ships.

    python3 -I convert.py <anny-checkout>/src <out>/human.pack [--rank-tolerance-mm 0.5]

Needs numpy, torch, roma, trimesh, zstandard on the path, and a `anny-*.dist-info/METADATA` on the path for `importlib.metadata` (see README.md).
The source is Anny (Apache-2.0 code) with MPFB2 and Face Units data (CC0). The output is little-endian: one zstd frame holding

    b"VEHUMAN\\x01" | u32 json_length | json header | binary payload

The JSON header lists `sections` ({name: {offset, dtype, shape}}, offsets from the start of the payload), the label lists and the phenotype anchors, see
docs/handoff-codex-human.md for what each section means. Axes are converted from Anny's (x right, y back, z up) to ours (x right, y up, z forward), metres.
"""
import json
import struct
import sys

import numpy as np
import torch
import zstandard

args = [a for a in sys.argv[1:] if not a.startswith("--")]
src, out = args[0], args[1]
tolerance_mm = 2.5
if "--rank-tolerance-mm" in sys.argv:
    tolerance_mm = float(sys.argv[sys.argv.index("--rank-tolerance-mm") + 1])
sys.path.insert(0, src)
from anny.models.model_data import PHENOTYPE_VARIATIONS  # noqa: E402
from anny.models.phenotype import Anny  # noqa: E402

model = Anny(rig="anny", topology="anny-quads", local_changes="all", facial_actions="all", phenotypes="all")


def np_(t, dtype=np.float64):
    return t.detach().cpu().numpy().astype(dtype)


# Anny (x right, y back, z up) -> ours (x right, y up, z forward): (x, y, z) -> (x, z, -y), a proper rotation.
def axes(a):
    a = np.asarray(a)
    return np.stack([a[..., 0], a[..., 2], -a[..., 1]], axis=-1)


template = axes(np_(model.template_vertices))
vertex_count = template.shape[0]
shapes = axes(np_(model.blendshapes))  # (1188, V, 3): 624 phenotype, 52 face, 512 local
phenotype = shapes[:624].reshape(624, -1)
face = shapes[624:676]
local = shapes[676:]

sections = {}
payload = bytearray()


def add(name, array, dtype):
    array = np.ascontiguousarray(np.asarray(array).astype(dtype))
    while len(payload) % 4:
        payload.append(0)
    sections[name] = {"offset": len(payload), "dtype": np.dtype(dtype).name, "shape": list(array.shape)}
    payload.extend(array.tobytes())


# Phenotype shapes are products of smooth anchors, so a low-rank basis carries them: delta = sum_r (sum_k w_k C[k, r]) basis[r].
u, s, vt = np.linalg.svd(phenotype, full_matrices=False)


def hat_weights(value, anchors):
    """Anny's piecewise linear interpolation weights on a feature's anchors (clamped, no extrapolation)."""
    value = min(max(value, anchors[0]), anchors[-1])
    weights = np.zeros(len(anchors))
    upper = min(max(int(np.searchsorted(anchors, value, side="left")), 1), len(anchors) - 1)
    alpha = (value - anchors[upper - 1]) / (anchors[upper] - anchors[upper - 1])
    weights[upper - 1], weights[upper] = 1.0 - alpha, alpha
    return weights


def phenotype_weights(params):
    """The 624 phenotype shape weights for a settings dict: products of per-feature weights picked by the mask, as in Anny."""
    mask = np_(model.stacked_phenotype_blend_shapes_mask)
    columns = {}
    for feature in ["age", "gender", "muscle", "weight", "height", "proportions", "cupsize", "firmness"]:
        hats = hat_weights(params.get(feature, 0.5), np_(model.anchors[feature]).tolist())
        for key, value in zip(PHENOTYPE_VARIATIONS[feature], hats):
            columns[key] = value
    race = np.array([params.get(k, 0.5) for k in ("african", "asian", "caucasian")])
    race = race / race.sum() if race.sum() > 0 else np.full(3, 1 / 3)
    for key, value in zip(("african", "asian", "caucasian"), race):
        columns[key] = value
    phens = np.array([columns[key] for keys in PHENOTYPE_VARIATIONS.values() for key in keys])
    return np.prod(phens[None, :] * mask + (1 - mask), axis=1)


# Pick the smallest rank whose error on random settings (what a person can actually ask for) stays within the tolerance.
rng = np.random.default_rng(7)
probes = np.array([phenotype_weights({k: rng.random() for k in model.phenotype_labels}) for _ in range(400)])
exact = probes @ phenotype
rank = 8
while rank < len(s):
    approx = (probes @ u[:, :rank] * s[:rank]) @ vt[:rank]
    if np.abs(approx - exact).max() * 1000.0 <= tolerance_mm:
        break
    rank += 8
coefficients = u[:, :rank] * s[:rank]  # (624, rank)
basis = vt[:rank]  # (rank, V*3)
scale = np.abs(basis).max(axis=1) / 32767.0
basis_q = np.round(basis / scale[:, None]).astype(np.int16)
print(f"phenotype rank {rank} of 624 (max error <= {tolerance_mm} mm before quantisation)")

add("template", template, np.float32)
add("phenotype_basis", basis_q.reshape(rank, vertex_count, 3), np.int16)
add("phenotype_basis_scale", scale, np.float32)
add("phenotype_coefficients", coefficients, np.float32)
add("phenotype_mask", np_(model.stacked_phenotype_blend_shapes_mask), np.uint8)


def sparse(name, group):
    """Shapes that touch few vertices: per shape the vertex indices and i16 deltas with one scale each."""
    offsets, indices, deltas, scales = [0], [], [], []
    for shape in group:
        touched = np.nonzero(np.abs(shape).max(axis=1) > 1e-7)[0]
        peak = float(np.abs(shape[touched]).max()) if len(touched) else 0.0
        scale_i = peak / 32767.0 if peak > 0 else 1.0
        indices.append(touched.astype(np.uint32))
        deltas.append(np.round(shape[touched] / scale_i).astype(np.int16))
        scales.append(scale_i)
        offsets.append(offsets[-1] + len(touched))
    add(f"{name}_offsets", np.array(offsets), np.uint32)
    add(f"{name}_indices", np.concatenate(indices), np.uint32)
    add(f"{name}_deltas", np.concatenate(deltas), np.int16)
    add(f"{name}_scales", np.array(scales), np.float32)


sparse("face", face)
sparse("local", local)

# The surface the renderer draws: Anny's quads with their UVs, so a vertex is split where the UV seams are.
quads = np_(model.faces, np.int64)
uv = np_(model.texture_coordinates)
uv_quads = np_(model.face_texture_coordinate_indices, np.int64)
corner_key = {}
render_source, render_uv, triangles = [], [], []
for quad, uv_quad in zip(quads, uv_quads):
    ids = []
    for v, t in zip(quad, uv_quad):
        key = (int(v), int(t))
        if key not in corner_key:
            corner_key[key] = len(render_source)
            render_source.append(int(v))
            render_uv.append([uv[t][0], 1.0 - uv[t][1]])
        ids.append(corner_key[key])
    triangles.append([ids[0], ids[1], ids[2]])
    triangles.append([ids[0], ids[2], ids[3]])
add("render_source", render_source, np.uint32)
add("render_uv", render_uv, np.float32)
add("triangles", triangles, np.uint32)
add("quads", quads, np.uint32)

# Skin weights: up to nine bones per vertex.
add("skin_bones", np_(model.vertex_bone_indices, np.int64), np.uint8)
add("skin_weights", np.round(np_(model.vertex_bone_weights) * 65535.0), np.uint16)

# Skeleton: heads and tails move with the shape, with the same coefficients as the surface.
heads = axes(np_(model.template_bone_heads))
add("bone_heads", heads, np.float32)
add("bone_head_shapes", axes(np_(model.bone_heads_blendshapes)), np.float32)
add("bone_parents", np.asarray(model.bone_parents), np.int32)

header = {
    "format": "ve-human-pack",
    "version": 1,
    "source": "NAVER Anny (Apache-2.0) with MPFB2 and Face Units data (CC0)",
    "vertex_count": int(vertex_count),
    "render_vertex_count": len(render_source),
    "rank": int(rank),
    "phenotype_labels": list(model.phenotype_labels),
    "phenotype_variations": {k: list(v) for k, v in PHENOTYPE_VARIATIONS.items()},
    "anchors": {k: np_(model.anchors[k]).tolist() for k in ["age", "gender", "muscle", "weight", "height", "proportions", "cupsize", "firmness"]},
    "local_change_labels": list(model.local_change_labels),
    "facial_action_labels": list(model.facial_action_labels),
    "bone_labels": list(model.bone_labels),
    "sections": sections,
}
blob = json.dumps(header).encode()
raw = b"VEHUMAN\x01" + struct.pack("<I", len(blob)) + blob + bytes(payload)
packed = zstandard.ZstdCompressor(level=19).compress(raw)
with open(out, "wb") as f:
    f.write(packed)
print(f"raw {len(raw) / 1e6:.1f} MB, zstd {len(packed) / 1e6:.1f} MB")

# Golden values for the Rust evaluator: the reference model at a few settings, ours axes, sampled vertices.
golden = []
sample = list(range(0, vertex_count, 997))
settings = [
    {},
    {"gender": 0.0, "age": 0.5, "height": 0.8},
    {"gender": 1.0, "age": 0.9, "weight": 0.9, "muscle": 0.2},
    {"age": 0.1, "height": 0.2},
    {"african": 1.0, "asian": 0.0, "caucasian": 0.0, "weight": 0.2},
]
for pheno in settings:
    result = model(phenotype_kwargs=pheno or None, local_changes_kwargs={model.local_change_labels[6]: 0.7}, facial_actions={"jawOpen": 0.8})
    # `rest_vertices` is template + blend shapes; `vertices` is also posed by the reference model's own rest pose, which we do not use.
    verts = axes(result["rest_vertices"][0].detach().cpu().numpy().astype(np.float64))
    golden.append({"phenotype": pheno, "local": {model.local_change_labels[6]: 0.7}, "face": {"jawOpen": 0.8}, "sample": sample, "positions": verts[sample].tolist()})
golden_path = out.rsplit(".", 1)[0] + ".golden.json"
with open(golden_path, "w") as f:
    json.dump(golden, f)
print("golden written", golden_path)
