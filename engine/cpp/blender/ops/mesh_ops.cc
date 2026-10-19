// Blender's bmesh operators (inset, solidify, extrude, dissolve, poke, subdivide, wireframe,
// symmetrize) on a polygon mesh, selected by faces or edges.
//
// Each runs through Blender's own operator system: the input elements are marked selected, the
// operator is called by name with typed arguments, and the BMesh is read back with provenance. The
// operator system reports a refusal through an error stack; that becomes VE_ERR_OP_REFUSED.
#include <array>
#include <cmath>
#include <cstddef>
#include <unordered_set>
#include <vector>

#include "bmesh.hh"
#include "bmesh_decimate.hh"
#include "bmesh_error.hh"
#include "bmesh_operators.hh"
#include "polymesh.hh"

using namespace blender;
using namespace ve_blender;

namespace {

// Operators other than bevel hold more intermediate geometry, so their input is capped lower.
constexpr size_t kMaxOpFaces = 250'000;
constexpr double kMaxMagnitude = 1.0e6;
constexpr int kMaxCuts = 8;

bool finite_within(double value) {
  return std::isfinite(value) && std::fabs(value) <= kMaxMagnitude;
}

void select_face(BMFace* face) {
  BM_elem_flag_enable(face, BM_ELEM_SELECT);
  BMLoop* first = BM_FACE_FIRST_LOOP(face);
  BMLoop* loop = first;
  do {
    BM_elem_flag_enable(loop->v, BM_ELEM_SELECT);
    BM_elem_flag_enable(loop->e, BM_ELEM_SELECT);
    loop = loop->next;
  } while (loop != first);
}

ve_status select_faces(const ve_mesh_op_input& in, const LoadedMesh& mesh, bool empty_means_all) {
  if (in.face_count == 0) {
    if (!empty_means_all) {
      return VE_ERR_INVALID_ARG;
    }
    for (BMFace* face : mesh.faces) {
      select_face(face);
    }
    return VE_OK;
  }
  if (in.faces == nullptr) {
    return VE_ERR_INVALID_ARG;
  }
  for (size_t i = 0; i < in.face_count; ++i) {
    if (in.faces[i] >= mesh.faces.size()) {
      return VE_ERR_OUT_OF_BOUNDS;
    }
    select_face(mesh.faces[in.faces[i]]);
  }
  return VE_OK;
}

ve_status select_edges(const ve_mesh_op_input& in, const LoadedMesh& mesh) {
  if (in.edge_count == 0 || in.edges == nullptr) {
    return VE_ERR_INVALID_ARG;
  }
  for (size_t i = 0; i < in.edge_count; ++i) {
    const uint32_t a = in.edges[2 * i];
    const uint32_t b = in.edges[(2 * i) + 1];
    if (a >= mesh.verts.size() || b >= mesh.verts.size() || a == b) {
      return VE_ERR_OUT_OF_BOUNDS;
    }
    BMEdge* edge = BM_edge_exists(mesh.verts[a], mesh.verts[b]);
    if (edge == nullptr) {
      return VE_ERR_INVALID_ARG;
    }
    BM_elem_flag_enable(edge, BM_ELEM_SELECT);
    BM_elem_flag_enable(edge->v1, BM_ELEM_SELECT);
    BM_elem_flag_enable(edge->v2, BM_ELEM_SELECT);
  }
  return VE_OK;
}

ve_status select_vertices(const ve_mesh_op_input& in, const LoadedMesh& mesh) {
  if (in.vertex_count == 0) {
    BM_mesh_elem_hflag_enable_all(mesh.bm.get(), BM_VERT, BM_ELEM_SELECT, true);
    return VE_OK;
  }
  if (in.vertices == nullptr) {
    return VE_ERR_INVALID_ARG;
  }
  for (size_t i = 0; i < in.vertex_count; ++i) {
    if (in.vertices[i] >= mesh.verts.size()) {
      return VE_ERR_OUT_OF_BOUNDS;
    }
    BM_elem_flag_enable(mesh.verts[in.vertices[i]], BM_ELEM_SELECT);
  }
  return VE_OK;
}

bool flag(const ve_mesh_op_input& in, int bit) { return (in.flags & bit) != 0; }

// Each runner returns false when Blender rejected the call.
bool run_inset(BMesh* bm, const ve_mesh_op_input& in) {
  return BMO_op_callf(
      bm, BMO_FLAG_DEFAULTS,
      "inset_region faces=%hf use_boundary=%b use_even_offset=%b use_interpolate=%b "
      "use_relative_offset=%b use_edge_rail=%b thickness=%f depth=%f use_outset=%b",
      BM_ELEM_SELECT, flag(in, 4), flag(in, 1), true, flag(in, 2), flag(in, 16), in.params[0],
      in.params[1], flag(in, 8));
}

bool run_solidify(BMesh* bm, const ve_mesh_op_input& in) {
  return BMO_op_callf(bm, BMO_FLAG_DEFAULTS, "solidify geom=%hfev thickness=%f", BM_ELEM_SELECT,
                      in.params[0]);
}

using Normals = std::vector<std::array<float, 3>>;

void set_role(const LoadedMesh& mesh, BMFace* face, int role) {
  BM_ELEM_CD_SET_INT(face, mesh.role_layer, role);
}

// An extruded cap faces the way the face it came from did; a side wall turns away from it.
void tag_extrusion(const LoadedMesh& mesh, BMOperator* extrude, const Normals& normals) {
  BMOIter iter;
  BMFace* face;
  BMO_ITER(face, &iter, extrude->slots_out, "geom.out", BM_FACE) {
    const int parent = BM_ELEM_CD_GET_INT(face, mesh.face_layer);
    float normal[3];
    BM_face_calc_normal(face, normal);
    bool cap = false;
    if (parent > 0 && static_cast<size_t>(parent) <= normals.size()) {
      const auto& before = normals[static_cast<size_t>(parent) - 1];
      const float alignment =
          (normal[0] * before[0]) + (normal[1] * before[1]) + (normal[2] * before[2]);
      cap = alignment > 0.9F;
    }
    set_role(mesh, face, cap ? 1 : 2);
  }
}

bool run_extrude(BMesh* bm, const ve_mesh_op_input& in, const LoadedMesh& mesh,
                 const Normals& normals) {
  BMOperator extrude;
  if (!BMO_op_initf(bm, &extrude, BMO_FLAG_DEFAULTS,
                    "extrude_face_region geom=%hfev use_normal_from_adjacent=%b", BM_ELEM_SELECT,
                    true)) {
    return false;
  }
  BMO_op_exec(bm, &extrude);
  tag_extrusion(mesh, &extrude, normals);
  const float offset[3] = {static_cast<float>(in.params[0]), static_cast<float>(in.params[1]),
                           static_cast<float>(in.params[2])};
  const bool moved = BMO_op_callf(bm, BMO_FLAG_DEFAULTS, "translate vec=%v verts=%S", offset,
                                  &extrude, "geom.out");
  BMO_op_finish(bm, &extrude);
  return moved;
}

bool run_dissolve(BMesh* bm, const ve_mesh_op_input& in) {
  return BMO_op_callf(
      bm, BMO_FLAG_DEFAULTS,
      "dissolve_limit angle_limit=%f use_dissolve_boundaries=%b verts=%av edges=%ae "
      "delimit=%i",
      in.params[0], flag(in, 1), 0);
}

bool run_poke(BMesh* bm, const ve_mesh_op_input& in) {
  const int modes[3] = {BMOP_POKE_MEDIAN, BMOP_POKE_MEDIAN_WEIGHTED, BMOP_POKE_BOUNDS};
  return BMO_op_callf(
      bm, BMO_FLAG_DEFAULTS, "poke faces=%hf offset=%f center_mode=%i use_relative_offset=%b",
      BM_ELEM_SELECT, in.params[0], modes[static_cast<int>(in.params[1])], flag(in, 1));
}

bool run_subdivide(BMesh* bm, const ve_mesh_op_input& in) {
  return BMO_op_callf(bm, BMO_FLAG_DEFAULTS,
                      "subdivide_edges edges=%he cuts=%i smooth=%f use_grid_fill=%b "
                      "use_single_edge=%b use_only_quads=%b quad_corner_type=%i",
                      BM_ELEM_SELECT, static_cast<int>(in.params[0]), in.params[1], flag(in, 1),
                      false, false, static_cast<int>(SUBD_CORNER_INNERVERT));
}

bool run_wireframe(BMesh* bm, const ve_mesh_op_input& in) {
  return BMO_op_callf(bm, BMO_FLAG_DEFAULTS,
                      "wireframe faces=%hf thickness=%f offset=%f use_replace=%b use_boundary=%b "
                      "use_even_offset=%b use_relative_offset=%b use_crease=%b crease_weight=%f "
                      "material_offset=%i",
                      BM_ELEM_SELECT, in.params[0], in.params[1], flag(in, 8), flag(in, 4),
                      flag(in, 1), flag(in, 2), false, 0.0, 0);
}

bool run_symmetrize(BMesh* bm, const ve_mesh_op_input& in) {
  return BMO_op_callf(bm, BMO_FLAG_DEFAULTS, "symmetrize input=%hvef direction=%i dist=%f",
                      BM_ELEM_SELECT, static_cast<int>(in.params[0]), in.params[1]);
}

bool run_triangulate(BMesh* bm) {
  return BMO_op_callf(bm, BMO_FLAG_DEFAULTS, "triangulate faces=%hf quad_method=%i ngon_method=%i",
                      BM_ELEM_SELECT, 1, 1);
}

bool run_remove_doubles(BMesh* bm, const ve_mesh_op_input& in) {
  return BMO_op_callf(bm, BMO_FLAG_DEFAULTS, "remove_doubles verts=%hv dist=%f", BM_ELEM_SELECT,
                      in.params[0]);
}

bool run_bisect(BMesh* bm, const ve_mesh_op_input& in) {
  const float point[3] = {static_cast<float>(in.params[0]), static_cast<float>(in.params[1]),
                          static_cast<float>(in.params[2])};
  const float normal[3] = {static_cast<float>(in.params[3]), static_cast<float>(in.params[4]),
                           static_cast<float>(in.params[5])};
  BMOperator bisect;
  if (!BMO_op_initf(bm, &bisect, BMO_FLAG_DEFAULTS,
                    "bisect_plane geom=%avef dist=%f plane_co=%v plane_no=%v use_snap_center=%b "
                    "clear_inner=%b clear_outer=%b",
                    in.params[6], point, normal, false, flag(in, 1), flag(in, 2)))
    return false;
  BMO_op_exec(bm, &bisect);
  bool filled = true;
  if (flag(in, 4) && (flag(in, 1) || flag(in, 2))) {
    filled = BMO_op_callf(bm, BMO_FLAG_DEFAULTS, "holes_fill edges=%S sides=%i", &bisect,
                          "geom_cut.out", 0);
  }
  BMO_op_finish(bm, &bisect);
  return filled;
}

// Revolves the selection around an axis. Faces are swept by their boundary edges only: Blender's own face
// sweep leaves a copy of the region at every step (inside the result), which would make a ring non-manifold.
// Blender splices the first and last copy itself when `use_merge` is set, but only the caller knows the turn
// is a full one, so merge is passed on a full turn only; `distance` then merges vertices that land together.
bool run_spin(BMesh* bm, const ve_mesh_op_input& in) {
  const bool full_turn = std::fabs(in.params[6] - 6.283185307179586) < 1.0e-6;
  const bool merge = flag(in, 1) && full_turn;
  // The faces whose boundary is swept: after a closed ring is made they would sit inside it.
  std::vector<BMFace*> region;
  if (in.edge_count == 0) {
    BMIter region_iter;
    BMFace* region_face;
    BM_ITER_MESH(region_face, &region_iter, bm, BM_FACES_OF_MESH) {
      if (BM_elem_flag_test(region_face, BM_ELEM_SELECT)) region.push_back(region_face);
    }
    BMIter edge_iter;
    BMEdge* edge;
    BM_ITER_MESH(edge, &edge_iter, bm, BM_EDGES_OF_MESH) {
      BMIter face_iter;
      BMFace* face;
      int selected_faces = 0;
      BM_ITER_ELEM(face, &face_iter, edge, BM_FACES_OF_EDGE) {
        if (BM_elem_flag_test(face, BM_ELEM_SELECT)) ++selected_faces;
      }
      BM_elem_flag_set(edge, BM_ELEM_SELECT, selected_faces == 1);
    }
    BMFace* face;
    BMIter all_faces;
    BM_ITER_MESH(face, &all_faces, bm, BM_FACES_OF_MESH) { BM_elem_flag_disable(face, BM_ELEM_SELECT); }
    BMVert* vert;
    BMIter vert_iter;
    BM_ITER_MESH(vert, &vert_iter, bm, BM_VERTS_OF_MESH) {
      bool on_boundary = false;
      BMIter around;
      BMEdge* e;
      BM_ITER_ELEM(e, &around, vert, BM_EDGES_OF_VERT) {
        if (BM_elem_flag_test(e, BM_ELEM_SELECT)) on_boundary = true;
      }
      BM_elem_flag_set(vert, BM_ELEM_SELECT, on_boundary);
    }
  }
  const float center[3] = {static_cast<float>(in.params[0]), static_cast<float>(in.params[1]),
                           static_cast<float>(in.params[2])};
  const float axis[3] = {static_cast<float>(in.params[3]), static_cast<float>(in.params[4]),
                         static_cast<float>(in.params[5])};
  const float delta[3] = {0.0F, 0.0F, 0.0F};
  BMOperator spin;
  if (!BMO_op_initf(bm, &spin, BMO_FLAG_DEFAULTS,
                    "spin geom=%hef cent=%v axis=%v dvec=%v angle=%f steps=%i use_merge=%b "
                    "use_normal_flip=%b use_duplicate=%b",
                    BM_ELEM_SELECT, center, axis, delta, in.params[6], static_cast<int>(in.params[7]),
                    merge, false, false)) {
    return false;
  }
  BMO_op_exec(bm, &spin);
  BMO_op_finish(bm, &spin);
  if (!merge) return true;
  for (BMFace* inside : region) BM_face_kill(bm, inside);
  BMIter vert_iter;
  BMVert* vert;
  BM_ITER_MESH(vert, &vert_iter, bm, BM_VERTS_OF_MESH) { BM_elem_flag_enable(vert, BM_ELEM_SELECT); }
  return BMO_op_callf(bm, BMO_FLAG_DEFAULTS, "remove_doubles verts=%hv dist=%f", BM_ELEM_SELECT,
                      in.params[8]);
}

// Checks the numbers an operator takes and what it selects; returns VE_OK when the call may
// proceed.
ve_status validate_params(const ve_mesh_op_input& in) {
  const double* p = in.params;
  const int param_count = in.op == VE_MESH_OP_SPIN ? 9 : in.op == VE_MESH_OP_BISECT ? 7 : 4;
  for (int i = 0; i < param_count; ++i) {
    if (!finite_within(p[i])) {
      return VE_ERR_NON_FINITE;
    }
  }
  const auto in_range = [](double v, double low, double high) { return v >= low && v <= high; };
  switch (in.op) {
    case VE_MESH_OP_INSET:
      return p[0] > 0.0 ? VE_OK : VE_ERR_INVALID_ARG;
    case VE_MESH_OP_SOLIDIFY:
      return p[0] != 0.0 ? VE_OK : VE_ERR_INVALID_ARG;
    case VE_MESH_OP_EXTRUDE:
      return VE_OK;
    case VE_MESH_OP_DISSOLVE:
      return in_range(p[0], 0.0, 3.14159265358979) ? VE_OK : VE_ERR_INVALID_ARG;
    case VE_MESH_OP_POKE:
      return in_range(p[1], 0.0, 2.0) && p[1] == std::floor(p[1]) ? VE_OK : VE_ERR_INVALID_ARG;
    case VE_MESH_OP_SUBDIVIDE:
      return in_range(p[0], 1.0, kMaxCuts) && p[0] == std::floor(p[0]) && in_range(p[1], 0.0, 1.0)
                 ? VE_OK
                 : VE_ERR_INVALID_ARG;
    case VE_MESH_OP_WIREFRAME:
      return p[0] > 0.0 ? VE_OK : VE_ERR_INVALID_ARG;
    case VE_MESH_OP_SYMMETRIZE:
      return in_range(p[0], 0.0, 5.0) && p[0] == std::floor(p[0]) && p[1] >= 0.0
                 ? VE_OK
                 : VE_ERR_INVALID_ARG;
    case VE_MESH_OP_DECIMATE:
      return p[0] > 0.0 && p[0] <= 1.0 && in_range(p[1], -1.0, 2.0) && p[1] == std::floor(p[1])
                 ? VE_OK
                 : VE_ERR_INVALID_ARG;
    case VE_MESH_OP_UNSUBDIVIDE:
      return in_range(p[0], 1.0, 4.0) && p[0] == std::floor(p[0]) ? VE_OK : VE_ERR_INVALID_ARG;
    case VE_MESH_OP_TRIANGULATE:
      return VE_OK;
    case VE_MESH_OP_REMOVE_DOUBLES:
      return in_range(p[0], 1.0e-9, 1.0e3) ? VE_OK : VE_ERR_INVALID_ARG;
    case VE_MESH_OP_BISECT:
      return p[6] >= 0.0 && (p[3] != 0.0 || p[4] != 0.0 || p[5] != 0.0) ? VE_OK
                                                                        : VE_ERR_INVALID_ARG;
    case VE_MESH_OP_SPIN:
      return p[6] >= 0.001 && p[6] <= 6.283185307179586 && p[7] >= 2.0 && p[7] <= 256.0 &&
                     p[7] == static_cast<int>(p[7]) && p[8] > 0.0 &&
                     (p[3] != 0.0 || p[4] != 0.0 || p[5] != 0.0)
                 ? VE_OK : VE_ERR_INVALID_ARG;
    default:
      return VE_ERR_INVALID_ARG;
  }
}

// What each operator selects: faces (possibly all), edges, or the whole mesh.
ve_status select_for(const ve_mesh_op_input& in, const LoadedMesh& mesh) {
  switch (in.op) {
    case VE_MESH_OP_INSET:
    case VE_MESH_OP_EXTRUDE:
      return select_faces(in, mesh, false);
    case VE_MESH_OP_SOLIDIFY:
    case VE_MESH_OP_POKE:
    case VE_MESH_OP_WIREFRAME:
      return select_faces(in, mesh, true);
    case VE_MESH_OP_SUBDIVIDE:
      return select_edges(in, mesh);
    case VE_MESH_OP_SPIN:
      if (in.edge_count > 0) return select_edges(in, mesh);
      return select_faces(in, mesh, true);
    case VE_MESH_OP_TRIANGULATE:
      return select_faces(in, mesh, true);
    case VE_MESH_OP_REMOVE_DOUBLES:
      return select_vertices(in, mesh);
    default:
      BM_mesh_elem_hflag_enable_all(mesh.bm.get(), BM_VERT | BM_EDGE | BM_FACE, BM_ELEM_SELECT,
                                    false);
      return VE_OK;
  }
}

bool run_op(BMesh* bm, const ve_mesh_op_input& in, const LoadedMesh& mesh, const Normals& normals) {
  switch (in.op) {
    case VE_MESH_OP_INSET:
      return run_inset(bm, in);
    case VE_MESH_OP_SOLIDIFY:
      return run_solidify(bm, in);
    case VE_MESH_OP_EXTRUDE:
      return run_extrude(bm, in, mesh, normals);
    case VE_MESH_OP_DISSOLVE:
      return run_dissolve(bm, in);
    case VE_MESH_OP_POKE:
      return run_poke(bm, in);
    case VE_MESH_OP_SUBDIVIDE:
      return run_subdivide(bm, in);
    case VE_MESH_OP_WIREFRAME:
      return run_wireframe(bm, in);
    case VE_MESH_OP_DECIMATE:
      // The collapse indexes its quadric and heap tables by element index, which a mesh built
      // element by element does not have yet.
      BM_mesh_elem_index_ensure(bm, BM_VERT | BM_EDGE | BM_FACE);
      BM_mesh_decimate_collapse(bm, static_cast<float>(in.params[0]), nullptr, 0.0F, flag(in, 1),
                                static_cast<int>(in.params[1]), 1.0e-4F);
      return true;
    case VE_MESH_OP_UNSUBDIVIDE:
      BM_mesh_elem_index_ensure(bm, BM_VERT | BM_EDGE | BM_FACE);
      BM_mesh_decimate_unsubdivide(bm, static_cast<int>(in.params[0]));
      return true;
    case VE_MESH_OP_TRIANGULATE:
      return run_triangulate(bm);
    case VE_MESH_OP_REMOVE_DOUBLES:
      return run_remove_doubles(bm, in);
    case VE_MESH_OP_BISECT:
      return run_bisect(bm, in);
    case VE_MESH_OP_SPIN:
      return run_spin(bm, in);
    default:
      return run_symmetrize(bm, in);
  }
}

// An upper bound on the faces a subdivision can make, so a request is refused before it runs.
bool subdivision_fits(const ve_mesh_op_input& in) {
  if (in.op != VE_MESH_OP_SUBDIVIDE) {
    return true;
  }
  const double cuts = in.params[0] + 1.0;
  const double bound = (static_cast<double>(in.edge_count) * 2.0 * cuts * cuts) +
                       static_cast<double>(in.mesh.face_count);
  return bound <= static_cast<double>(kMaxFaces);
}

ve_polymesh* run(const ve_mesh_op_input& in, ve_status* status) {
  if (in.struct_size != sizeof(ve_mesh_op_input)) {
    *status = VE_ERR_INVALID_ARG;
    return nullptr;
  }
  if (in.mesh.face_count > kMaxOpFaces) {
    *status = VE_ERR_TOO_LARGE;
    return nullptr;
  }
  if (*status = validate_params(in); *status != VE_OK) {
    return nullptr;
  }
  if (!subdivision_fits(in)) {
    *status = VE_ERR_TOO_LARGE;
    return nullptr;
  }
  LoadedMesh mesh;
  if (*status = load_mesh(in.mesh, &mesh, true); *status != VE_OK) {
    return nullptr;
  }
  if (*status = select_for(in, mesh); *status != VE_OK) {
    return nullptr;
  }
  BMesh* bm = mesh.bm.get();
  // A BMesh built element by element has no normals yet; inset depth, poke, solidify and dissolve
  // read them.
  BM_mesh_normals_update(bm);
  Normals normals;
  normals.reserve(mesh.faces.size());
  for (const BMFace* face : mesh.faces) {
    normals.push_back({face->no[0], face->no[1], face->no[2]});
  }
  // Faces of an inset that were selected stay as the inner faces; every face the operator creates
  // is part of the ring around them.
  std::unordered_set<const BMFace*> selected;
  std::unordered_set<const BMFace*> original(mesh.faces.begin(), mesh.faces.end());
  if (in.op == VE_MESH_OP_INSET) {
    for (const BMFace* face : mesh.faces) {
      if (BM_elem_flag_test(face, BM_ELEM_SELECT)) {
        selected.insert(face);
      }
    }
  }
  const bool ran = run_op(bm, in, mesh, normals);
  if (!ran || BMO_error_get(bm, nullptr, nullptr, nullptr)) {
    *status = VE_ERR_OP_REFUSED;
    return nullptr;
  }
  if (in.op == VE_MESH_OP_INSET) {
    BMIter iter;
    BMFace* face;
    BM_ITER_MESH(face, &iter, bm, BM_FACES_OF_MESH) {
      if (selected.count(face) != 0) {
        set_role(mesh, face, 1);
      } else if (original.count(face) == 0) {
        set_role(mesh, face, 2);
      }
    }
  }
  auto result = std::make_unique<ve_polymesh>();
  if (*status = store_mesh(mesh, result.get(), true); *status != VE_OK) {
    return nullptr;
  }
  *status = VE_OK;
  return result.release();
}

}  // namespace

extern "C" ve_polymesh* ve_mesh_op(const ve_mesh_op_input* input, ve_status* status) {
  ve_status local = VE_ERR_INVALID_ARG;
  ve_status* out = status != nullptr ? status : &local;
  *out = VE_ERR_INVALID_ARG;
  if (input == nullptr) {
    return nullptr;
  }
  return run(*input, out);
}
