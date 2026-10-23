#!/usr/bin/env python3
"""Adds the CC0 MakeHuman clothes and hair to `human.pack` (ADR 0055, amended: clothes and hair are the second imported dataset).

    python3 -I outfits.py <assets-dir> <base.obj> <human.pack> <out.pack> [asset ...]

`<assets-dir>` holds `clothes/<name>/<name>.obj` and `hair/<name>/<name>.obj` from the MakeHuman system assets (CC0; every file's own header is checked and
anything not marked CC0 is refused). `<base.obj>` is MakeHuman's base mesh (CC0), only used to confirm that its body vertices ARE our template: MakeHuman's
base mesh in decimetres, times 0.1, coincides with the pack's `template` (Anny's body is built on it), so clothes made for MakeHuman's body fit ours.

Each asset becomes a set of sections in the pack: its triangles and per-corner UVs, and for every vertex a binding to the body: the template triangle nearest to it
(three vertex ids and barycentric weights) and the vertex's offset from that surface point in the triangle's own frame (edge, edge-normal, normal). The Rust side
evaluates `position = sum(weights * body vertices) + offset in the frame of the current body triangle`, so the garment follows every phenotype slider.
Needs numpy and zstandard. The images (diffuse, normal, ao) are converted to PNGs next to the pack by `outfit_textures.py`.
"""
import json
import struct
import sys
from pathlib import Path

import numpy as np
import zstandard

MAGIC = b"VEHUMAN\x01"
CC0 = "CC0"


def read_pack(path):
    raw = zstandard.ZstdDecompressor().decompressobj().decompress(Path(path).read_bytes())
    assert raw[:8] == MAGIC, "not a human pack"
    n = struct.unpack("<I", raw[8:12])[0]
    return json.loads(raw[12:12 + n]), bytearray(raw[12 + n:])


def section(header, payload, name):
    s = header["sections"][name]
    dtype = np.dtype(s["dtype"])
    count = int(np.prod(s["shape"]))
    return np.frombuffer(payload, dtype=dtype, count=count, offset=s["offset"]).reshape(s["shape"])


def check_cc0(path):
    head = "".join(line for line, _ in zip(open(path, errors="replace"), range(40)))
    if "CC0" not in head:
        raise SystemExit(f"{path}: the header does not say CC0, refusing it")
    if "AGPL" in head or "CC BY" in head or "CC-BY" in head:
        raise SystemExit(f"{path}: the header names another licence, refusing it")


def read_obj(path):
    positions, uvs, corners = [], [], []
    for line in open(path):
        p = line.split()
        if not p:
            continue
        if p[0] == "v":
            positions.append([float(x) for x in p[1:4]])
        elif p[0] == "vt":
            uvs.append([float(x) for x in p[1:3]])
        elif p[0] == "f":
            face = []
            for token in p[1:]:
                parts = token.split("/")
                face.append((int(parts[0]) - 1, int(parts[1]) - 1 if len(parts) > 1 and parts[1] else 0))
            for k in range(1, len(face) - 1):
                corners += [face[0], face[k], face[k + 1]]
    return np.array(positions, np.float64) * 0.1, np.array(uvs, np.float64), corners


def closest_on_triangle(p, a, b, c):
    """Barycentric weights of the point of triangle abc closest to p (Ericson, Real-Time Collision Detection)."""
    ab, ac, ap = b - a, c - a, p - a
    d1, d2 = ab @ ap, ac @ ap
    if d1 <= 0 and d2 <= 0:
        return np.array([1.0, 0, 0])
    bp = p - b
    d3, d4 = ab @ bp, ac @ bp
    if d3 >= 0 and d4 <= d3:
        return np.array([0, 1.0, 0])
    vc = d1 * d4 - d3 * d2
    if vc <= 0 and d1 >= 0 and d3 <= 0:
        v = d1 / (d1 - d3)
        return np.array([1 - v, v, 0])
    cp = p - c
    d5, d6 = ab @ cp, ac @ cp
    if d6 >= 0 and d5 <= d6:
        return np.array([0, 0, 1.0])
    vb = d5 * d2 - d1 * d6
    if vb <= 0 and d2 >= 0 and d6 <= 0:
        w = d2 / (d2 - d6)
        return np.array([1 - w, 0, w])
    va = d3 * d6 - d5 * d4
    if va <= 0 and (d4 - d3) >= 0 and (d5 - d6) >= 0:
        w = (d4 - d3) / ((d4 - d3) + (d5 - d6))
        return np.array([0, 1 - w, w])
    denom = 1 / (va + vb + vc)
    v, w = vb * denom, vc * denom
    return np.array([1 - v - w, v, w])


def vertex_normals(positions, triangles):
    """Area-weighted outward vertex normals (the pack's triangles wind outward); the Rust side computes them the same way."""
    n = np.zeros_like(positions)
    a, b, c = (positions[triangles[:, k]] for k in range(3))
    face = np.cross(b - a, c - a)
    for k in range(3):
        np.add.at(n, triangles[:, k], face)
    return n / np.maximum(np.linalg.norm(n, axis=1, keepdims=True), 1e-12)


def frame(a, b, c, normals_at):
    """The frame of a bound point: the interpolated vertex normal, the first edge made square to it, and their cross product."""
    n = normals_at
    n = n / np.linalg.norm(n)
    e1 = b - a
    e1 = e1 - (e1 @ n) * n
    e1 = e1 / np.linalg.norm(e1)
    e2 = np.cross(n, e1)
    return e1, e2, n


def bind(points, template, triangles, point_normals):
    from scipy.spatial import cKDTree
    tree = cKDTree(template)
    normals = vertex_normals(template, triangles)
    incident = [[] for _ in range(len(template))]
    for t, tri in enumerate(triangles):
        for v in tri:
            incident[v].append(t)
    ids = np.zeros((len(points), 3), np.uint32)
    bary = np.zeros((len(points), 3), np.float32)
    local = np.zeros((len(points), 3), np.float32)
    k = min(12, len(template))
    _, near = tree.query(points, k=k)
    for i, p in enumerate(points):
        best = None
        for v in near[i]:
            for t in incident[v]:
                a, b, c = (template[j] for j in triangles[t])
                w = closest_on_triangle(p, a, b, c)
                q = w[0] * a + w[1] * b + w[2] * c
                d = np.linalg.norm(p - q)
                # a garment faces away from the body: skip the body surface that faces the other way (a sleeve is not bound to the chest behind it)
                facing = (w @ normals[list(triangles[t])]) @ point_normals[i]
                d += 0.03 * max(0.0, -facing)
                if best is None or d < best[0]:
                    best = (d, t, w, q)
        _, t, w, q = best
        a, b, c = (template[j] for j in triangles[t])
        e1, e2, n = frame(a, b, c, w @ normals[list(triangles[t])])
        off = p - q
        ids[i] = triangles[t]
        bary[i] = w
        local[i] = [off @ e1, off @ e2, off @ n]
    return ids, bary, local


def skin_for(points, point_normals, template, template_normals, bones, weights):
    """Nine bone influences per asset vertex: the body's own, blended over the nearest body vertices that face the way the garment does
    (a sleeve takes its arm's weights, not the ribs behind it)."""
    from scipy.spatial import cKDTree
    tree = cKDTree(template)
    _, near = tree.query(points, k=16)
    out_bones = np.zeros((len(points), 9), np.uint8)
    out_weights = np.zeros((len(points), 9), np.uint16)
    for i, p in enumerate(points):
        acc = {}
        total = 0.0
        for v in near[i]:
            d = np.linalg.norm(p - template[v])
            facing = template_normals[v] @ point_normals[i]
            w = np.exp(-((d / 0.02) ** 2)) * max(facing, 0.0) ** 2
            if w <= 1e-9:
                continue
            total += w
            for b, bw in zip(bones[v], weights[v]):
                if bw > 0:
                    acc[int(b)] = acc.get(int(b), 0.0) + w * bw / 65535.0
        if not acc:  # nothing faces the garment: fall back to the nearest vertex
            v = near[i][0]
            for b, bw in zip(bones[v], weights[v]):
                if bw > 0:
                    acc[int(b)] = acc.get(int(b), 0.0) + bw / 65535.0
            total = 1.0
        top = sorted(acc.items(), key=lambda kv: -kv[1])[:9]
        norm = sum(w for _, w in top)
        for slot, (b, w) in enumerate(top):
            out_bones[i, slot] = b
            out_weights[i, slot] = int(round(w / norm * 65535.0))
    return out_bones, out_weights


def main():
    assets_dir, base_obj, pack_in, pack_out = sys.argv[1:5]
    names = sys.argv[5:] or ["clothes/female_casualsuit01", "clothes/shoes02", "hair/long01", "hair/bob01", "hair/bob02", "hair/short01", "hair/short02", "hair/short03", "hair/short04", "hair/ponytail01"]
    header, payload = read_pack(pack_in)
    template = section(header, payload, "template").astype(np.float64)  # (astype copies)
    # the pack's triangles index its render vertices (UV seams split vertices); `render_source` names the template vertex of each
    triangles = section(header, payload, "render_source").astype(np.int64)[section(header, payload, "triangles").astype(np.int64)]
    # the base mesh check: MakeHuman's body vertices (decimetres) are our template (metres)
    base = np.array([[float(x) for x in l.split()[1:4]] for l in open(base_obj) if l.startswith("v ")])[:13380] * 0.1
    from scipy.spatial import cKDTree
    d, _ = cKDTree(template).query(base)
    print(f"base mesh against template: mean {d.mean() * 1000:.3f} mm, max {d.max() * 1000:.2f} mm")
    assert d.max() < 0.01, "the base mesh is not the template"
    skin_bones = section(header, payload, "skin_bones").copy()  # copies: views of the payload would stop it growing
    skin_weights = section(header, payload, "skin_weights").copy()
    header.pop("outfits", None)
    for k in [k for k in header["sections"] if k.startswith("outfit_")]:
        del header["sections"][k]
    outfits = {}

    def add(name, array, dtype):
        a = np.ascontiguousarray(array, dtype)
        while len(payload) % 4:
            payload.append(0)
        header["sections"][name] = {"offset": len(payload), "dtype": np.dtype(dtype).name, "shape": list(a.shape)}
        payload.extend(a.tobytes())

    for name in names:
        kind, short = name.split("/")
        obj = Path(assets_dir) / name / f"{short}.obj"
        check_cc0(obj)
        check_cc0(Path(assets_dir) / name / f"{short}.mhclo")
        positions, uvs, corners = read_obj(obj)
        used = sorted({v for v, _ in corners})
        remap = {v: i for i, v in enumerate(used)}
        points = positions[used]
        asset_triangles = np.array([remap[v] for v, _ in corners], np.int64).reshape(-1, 3)
        point_normals = vertex_normals(points, asset_triangles)
        ids, bary, local = bind(points, template, triangles, point_normals)
        skin_b, skin_w = skin_for(points, point_normals, template, vertex_normals(template, triangles), skin_bones, skin_weights)
        residual = np.linalg.norm(points - (np.einsum("ij,ijk->ik", bary, template[ids]) + 0), axis=1)
        indices = np.array([remap[v] for v, _ in corners], np.uint32)
        corner_uv = np.array([uvs[t] for _, t in corners], np.float32)
        key = f"outfit_{short}"
        add(f"{key}_ids", ids, np.uint32)
        add(f"{key}_bary", bary, np.float32)
        add(f"{key}_local", local, np.float32)
        add(f"{key}_skin_bones", skin_b, np.uint8)
        add(f"{key}_skin_weights", skin_w, np.uint16)
        add(f"{key}_indices", indices, np.uint32)
        add(f"{key}_uv", corner_uv, np.float32)
        outfits[short] = {"kind": kind, "vertices": len(used), "triangles": len(indices) // 3,
                          "reach_cm": float(np.abs(local[:, 2]).max() * 100), "source": str(name), "license": CC0}
        print(f"{name}: {len(used)} vertices, {len(indices) // 3} triangles, farthest from the skin {np.abs(local[:, 2]).max() * 100:.1f} cm")
    header["outfits"] = outfits
    head = json.dumps(header, separators=(",", ":")).encode()
    raw = MAGIC + struct.pack("<I", len(head)) + head + bytes(payload)
    Path(pack_out).write_bytes(zstandard.ZstdCompressor(level=19).compress(raw))
    print("wrote", pack_out, Path(pack_out).stat().st_size, "bytes")


if __name__ == "__main__":
    main()
