// Skin: Blender's Skin modifier (the B-Mesh algorithm of Ji, Liu and Wang) over arrays. A skeleton graph, a pair of radii per joint and one root joint per
// connected piece become one closed surface of mostly quads, with clean junctions at every branch (hips, shoulders, fingers).
//
// The algorithm is the vendored, almost untouched modifications/intern/MOD_skin.cc; it is included here so that its static functions are visible to the entry point
// below, which replaces the Mesh-based wrappers (subdivide_base, base_skin) of the modifier with versions over plain arrays. Output vertices carry the skeleton
// joint they were made from (`vertex_origin`), so a later stage can weight and name regions by joint.
#include "modifiers/intern/MOD_skin.cc"

#include <algorithm>
#include <cmath>
#include <map>
#include <utility>
#include <vector>

#include "polymesh.hh"

namespace blender {

namespace {

constexpr size_t kMaxJoints = 4096;
constexpr size_t kMaxSkinVertices = 200'000;

struct SkinGraph {
  Array<float3> positions;
  Array<float2> radius;
  Array<bool> root;
  Array<bool> loose;
  Array<int2> edges;
  Array<int> owner;  // the joint of the caller's graph each joint came from
};

// ve_skin_subdivide: Blender's subdivide_base without vertex groups. Long edges get extra joints so the skin's rings stay close to each other.
SkinGraph subdivide_graph(const SkinGraph &orig, const bool enabled)
{
  const int orig_vert_num = int(orig.positions.size());
  const int orig_edge_num = int(orig.edges.size());
  Array<int> degree(orig_vert_num, 0);
  array_utils::count_indices(orig.edges.as_span().cast<int>(), degree);

  const VArray<float2> orig_radius = VArray<float2>::from_span(orig.radius.as_span());
  const VArray<bool> orig_loose = VArray<bool>::from_span(orig.loose.as_span());
  Array<int> edge_subd(orig_edge_num, 0);
  int subd_num = 0;
  if (enabled) {
    for (int i = 0; i < orig_edge_num; i++) {
      edge_subd[i] = calc_edge_subdivisions(
          orig.positions, orig_radius, orig_loose, orig.edges[i], degree);
      subd_num += edge_subd[i];
    }
  }

  SkinGraph out;
  out.positions = Array<float3>(orig_vert_num + subd_num);
  out.radius = Array<float2>(orig_vert_num + subd_num);
  out.root = Array<bool>(orig_vert_num + subd_num, false);
  out.loose = Array<bool>(orig_vert_num + subd_num, false);
  out.owner = Array<int>(orig_vert_num + subd_num, 0);
  out.edges = Array<int2>(orig_edge_num + subd_num);
  for (int i = 0; i < orig_vert_num; i++) {
    out.positions[i] = orig.positions[i];
    out.radius[i] = orig.radius[i];
    out.root[i] = orig.root[i];
    out.loose[i] = orig.loose[i];
    out.owner[i] = orig.owner[i];
  }

  int result_edge_i = 0;
  int v = orig_vert_num;
  for (int i = 0; i < orig_edge_num; i++) {
    const int2 &edge = orig.edges[i];
    int u = edge[0];
    float radrat = half_v2(out.radius[edge[1]]) / half_v2(out.radius[edge[0]]);
    radrat = std::isfinite(radrat) ? (radrat + 1.0F) / 2.0F : 1.0F;
    for (int j = 0; j < edge_subd[i]; j++, v++) {
      const float r = float(j + 1) / float(edge_subd[i] + 1);
      const float t = powf(r, radrat);
      interp_v3_v3v3(out.positions[v], out.positions[edge[0]], out.positions[edge[1]], t);
      interp_v2_v2v2(out.radius[v], orig.radius[edge[0]], orig.radius[edge[1]], t);
      out.owner[v] = t < 0.5F ? orig.owner[edge[0]] : orig.owner[edge[1]];
      out.edges[result_edge_i][0] = u;
      out.edges[result_edge_i][1] = v;
      result_edge_i++;
      u = v;
    }
    out.edges[result_edge_i][0] = u;
    out.edges[result_edge_i][1] = edge[1];
    result_edge_i++;
  }
  return out;
}

// The hull merging of the Skin algorithm can leave a few faces wound against their neighbours. This floods one orientation across every shared edge (two faces agree
// when they traverse a shared edge in opposite directions), then flips each connected piece whose signed volume is negative so it faces outward.
void make_winding_consistent(ve_polymesh *mesh)
{
  const size_t faces = mesh->face_sizes.size();
  std::vector<size_t> start(faces + 1, 0);
  for (size_t f = 0; f < faces; f++) {
    start[f + 1] = start[f] + mesh->face_sizes[f];
  }
  // directed edge (a, b) -> the faces that use it in that direction
  std::map<std::pair<uint32_t, uint32_t>, std::vector<size_t>> directed;
  for (size_t f = 0; f < faces; f++) {
    const uint32_t n = mesh->face_sizes[f];
    for (uint32_t k = 0; k < n; k++) {
      directed[{mesh->corners[start[f] + k], mesh->corners[start[f] + (k + 1) % n]}].push_back(f);
    }
  }
  std::vector<int> state(faces, -1);  // -1 unseen, 0 keep, 1 flip
  auto reverse_face = [&](size_t f) {
    std::reverse(mesh->corners.begin() + long(start[f]), mesh->corners.begin() + long(start[f + 1]));
  };
  for (size_t seed = 0; seed < faces; seed++) {
    if (state[seed] != -1) {
      continue;
    }
    std::vector<size_t> piece{seed};
    state[seed] = 0;
    for (size_t at = 0; at < piece.size(); at++) {
      const size_t f = piece[at];
      const uint32_t n = mesh->face_sizes[f];
      for (uint32_t k = 0; k < n; k++) {
        const uint32_t a = mesh->corners[start[f] + k];
        const uint32_t b = mesh->corners[start[f] + (k + 1) % n];
        // A consistent neighbour uses (b, a); one that uses (a, b) as well is wound the other way.
        for (const auto &want : {std::make_pair(std::make_pair(b, a), false), std::make_pair(std::make_pair(a, b), true)}) {
          const auto found = directed.find(want.first);
          if (found == directed.end()) {
            continue;
          }
          for (const size_t g : found->second) {
            if (g == f || state[g] != -1) {
              continue;
            }
            state[g] = want.second ? 1 - state[f] : state[f];
            piece.push_back(g);
          }
        }
      }
    }
    // Apply the flips to this piece, then check its volume.
    for (const size_t f : piece) {
      if (state[f] == 1) {
        reverse_face(f);
      }
    }
    // Rebuild the directed-edge map entries is unnecessary: later pieces share no edges with this one.
    double volume = 0.0;
    for (const size_t f : piece) {
      const float *a = &mesh->positions[3 * mesh->corners[start[f]]];
      for (uint32_t k = 1; k + 1 < mesh->face_sizes[f]; k++) {
        const float *b = &mesh->positions[3 * mesh->corners[start[f] + k]];
        const float *c = &mesh->positions[3 * mesh->corners[start[f] + k + 1]];
        volume += (double(a[0]) * (double(b[1]) * c[2] - double(b[2]) * c[1]) - double(a[1]) * (double(b[0]) * c[2] - double(b[2]) * c[0]) +
                   double(a[2]) * (double(b[0]) * c[1] - double(b[1]) * c[0])) / 6.0;
      }
    }
    if (volume < 0.0) {
      for (const size_t f : piece) {
        reverse_face(f);
      }
    }
  }
}

ve_status validate(const ve_skin_input &in)
{
  if (in.struct_size != sizeof(ve_skin_input) || in.positions == nullptr || in.radii == nullptr ||
      in.edges == nullptr || in.vertex_count == 0 || in.edge_count == 0)
  {
    return VE_ERR_INVALID_ARG;
  }
  if (in.vertex_count > kMaxJoints) {
    return VE_ERR_TOO_LARGE;
  }
  if (!std::isfinite(in.branch_smoothing) || in.branch_smoothing < 0.0F || in.branch_smoothing > 1.0F ||
      in.symmetry_axes > 7)
  {
    return VE_ERR_INVALID_ARG;
  }
  for (size_t i = 0; i < 3 * in.vertex_count; i++) {
    if (!std::isfinite(in.positions[i])) {
      return VE_ERR_NON_FINITE;
    }
  }
  for (size_t i = 0; i < 2 * in.vertex_count; i++) {
    if (!std::isfinite(in.radii[i]) || in.radii[i] < 0.0F) {
      return VE_ERR_INVALID_ARG;
    }
  }
  for (size_t i = 0; i < in.edge_count; i++) {
    const uint32_t a = in.edges[2 * i];
    const uint32_t b = in.edges[2 * i + 1];
    if (a >= in.vertex_count || b >= in.vertex_count) {
      return VE_ERR_OUT_OF_BOUNDS;
    }
    if (a == b) {
      return VE_ERR_INVALID_ARG;
    }
  }
  return VE_OK;
}

}  // namespace

}  // namespace blender

extern "C" ve_polymesh *ve_skin(const ve_skin_input *input, ve_status *status)
{
  using namespace blender;
  ve_status local = VE_ERR_INVALID_ARG;
  ve_status *out_status = status != nullptr ? status : &local;
  *out_status = VE_ERR_INVALID_ARG;
  if (input == nullptr) {
    return nullptr;
  }
  *out_status = validate(*input);
  if (*out_status != VE_OK) {
    return nullptr;
  }

  const int joints = int(input->vertex_count);
  SkinGraph graph;
  graph.positions = Array<float3>(joints);
  graph.radius = Array<float2>(joints);
  graph.root = Array<bool>(joints, false);
  graph.loose = Array<bool>(joints, false);
  graph.owner = Array<int>(joints, 0);
  graph.edges = Array<int2>(int(input->edge_count));
  for (int i = 0; i < joints; i++) {
    graph.positions[i] = float3(input->positions[3 * i], input->positions[3 * i + 1], input->positions[3 * i + 2]);
    graph.radius[i] = float2(input->radii[2 * i], input->radii[2 * i + 1]);
    graph.owner[i] = i;
    if (input->flags != nullptr) {
      graph.root[i] = (input->flags[i] & 1) != 0;
      graph.loose[i] = (input->flags[i] & 2) != 0;
    }
  }
  for (int i = 0; i < int(input->edge_count); i++) {
    graph.edges[i] = int2(int(input->edges[2 * i]), int(input->edges[2 * i + 1]));
  }

  const SkinGraph g = subdivide_graph(graph, input->subdivide != 0);
  const int verts_num = int(g.positions.size());

  SkinModifierData smd;
  smd.symmetry_axes = int(input->symmetry_axes);
  smd.branch_smoothing = input->branch_smoothing;
  smd.flag = input->smooth_shading != 0 ? MOD_SKIN_SMOOTH_SHADING : 0;

  Array<int> offsets;
  Array<int> indices;
  const GroupedSpan<int> vert_to_edge = bke::mesh::build_vert_to_edge_map(
      g.edges, verts_num, offsets, indices);

  bool has_valid_root = false;
  EMat *emat = build_edge_mats(
      VArray<bool>::from_span(g.root.as_span()), g.positions, verts_num, g.edges, vert_to_edge, &has_valid_root);
  if (!has_valid_root) {
    MEM_delete(emat);
    *out_status = VE_ERR_INVALID_ARG;  // every connected piece needs a root joint
    return nullptr;
  }
  SkinNode *skin_nodes = build_frames(
      g.positions, verts_num, VArray<float2>::from_span(g.radius.as_span()), vert_to_edge, emat);
  MEM_delete(emat);

  eSkinErrorFlag error = eSkinErrorFlag(0);
  BMesh *bm = build_skin(skin_nodes, verts_num, vert_to_edge, g.edges, nullptr, &smd, &error);
  if (bm == nullptr || size_t(bm->totvert) > kMaxSkinVertices) {
    if (bm != nullptr) {
      BM_mesh_free(bm);
    }
    MEM_delete(skin_nodes);
    *out_status = VE_ERR_TOO_LARGE;
    return nullptr;
  }

  // Which skeleton joint made each output vertex (merged frame corners were already re-pointed by build_skin).
  BM_mesh_elem_index_ensure(bm, BM_VERT);
  std::vector<uint32_t> owner_of(size_t(bm->totvert), ve_blender::kNoOrigin);
  for (int v = 0; v < verts_num; v++) {
    for (int f = 0; f < skin_nodes[v].totframe; f++) {
      for (int k = 0; k < 4; k++) {
        const BMVert *bv = skin_nodes[v].frames[f].verts[k];
        if (bv != nullptr) {
          owner_of[size_t(BM_elem_index_get(bv))] = uint32_t(g.owner[v]);
        }
      }
    }
  }
  MEM_delete(skin_nodes);

  // Vertices that no face uses are left over from the hull merging (frame corners that ended up inside a hull); they are not part of the surface, so they are not output.
  auto *result = new ve_polymesh();
  std::vector<uint32_t> new_index(size_t(bm->totvert), ve_blender::kNoOrigin);
  BMIter iter;
  BMFace *face;
  BM_ITER_MESH (face, &iter, bm, BM_FACES_OF_MESH) {
    BMLoop *first = BM_FACE_FIRST_LOOP(face);
    BMLoop *loop = first;
    do {
      new_index[size_t(BM_elem_index_get(loop->v))] = 0;
      loop = loop->next;
    } while (loop != first);
  }
  BMVert *vert;
  uint32_t next = 0;
  BM_ITER_MESH (vert, &iter, bm, BM_VERTS_OF_MESH) {
    uint32_t &slot = new_index[size_t(BM_elem_index_get(vert))];
    if (slot == ve_blender::kNoOrigin) {
      continue;
    }
    slot = next++;
    result->positions.insert(result->positions.end(), vert->co, vert->co + 3);
    result->vertex_origin.push_back(owner_of[size_t(BM_elem_index_get(vert))]);
  }
  BM_ITER_MESH (face, &iter, bm, BM_FACES_OF_MESH) {
    result->face_sizes.push_back(uint32_t(face->len));
    result->face_parent.push_back(ve_blender::kNoOrigin);
    result->face_role.push_back(0U);
    BMLoop *first = BM_FACE_FIRST_LOOP(face);
    BMLoop *loop = first;
    do {
      result->corners.push_back(new_index[size_t(BM_elem_index_get(loop->v))]);
      loop = loop->next;
    } while (loop != first);
  }
  BM_mesh_free(bm);
  make_winding_consistent(result);
  *out_status = VE_OK;
  return result;
}
