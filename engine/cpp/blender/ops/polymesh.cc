#include "polymesh.hh"

#include <algorithm>
#include <cmath>

#include "BKE_customdata.hh"
#include "bmesh.hh"

using namespace blender;

namespace ve_blender {

namespace {

constexpr const char* kVertexLayer = "ve_origin";
constexpr const char* kFaceLayer = "ve_parent";
constexpr const char* kRoleLayer = "ve_role";

bool distinct_corners(const uint32_t* face, uint32_t size) {
  std::vector<uint32_t> sorted(face, face + size);
  std::sort(sorted.begin(), sorted.end());
  return std::adjacent_find(sorted.begin(), sorted.end()) == sorted.end();
}

ve_status validate(const ve_polymesh_input& in) {
  if (in.struct_size != sizeof(ve_polymesh_input) || in.positions == nullptr ||
      in.face_sizes == nullptr || in.corners == nullptr || in.vertex_count == 0 ||
      in.face_count == 0) {
    return VE_ERR_INVALID_ARG;
  }
  if (in.vertex_count > kMaxVertices || in.face_count > kMaxFaces ||
      in.corner_count > kMaxCorners) {
    return VE_ERR_TOO_LARGE;
  }
  for (size_t i = 0; i < 3 * in.vertex_count; ++i) {
    if (!std::isfinite(in.positions[i])) {
      return VE_ERR_NON_FINITE;
    }
  }
  size_t offset = 0;
  for (size_t f = 0; f < in.face_count; ++f) {
    const uint32_t size = in.face_sizes[f];
    if (size < 3 || offset + size > in.corner_count) {
      return VE_ERR_INVALID_ARG;
    }
    for (uint32_t k = 0; k < size; ++k) {
      if (in.corners[offset + k] >= in.vertex_count) {
        return VE_ERR_OUT_OF_BOUNDS;
      }
    }
    if (!distinct_corners(in.corners + offset, size)) {
      return VE_ERR_INVALID_ARG;
    }
    offset += size;
  }
  return offset == in.corner_count ? VE_OK : VE_ERR_INVALID_ARG;
}

}  // namespace

ve_status validate_mesh(const ve_polymesh_input& input) { return validate(input); }

void BMeshDeleter::operator()(BMesh* bm) const {
  if (bm != nullptr) {
    BM_mesh_free(bm);
  }
}

ve_status load_mesh(const ve_polymesh_input& input, LoadedMesh* out, bool toolflags) {
  if (const ve_status status = validate(input); status != VE_OK) {
    return status;
  }
  const BMAllocTemplate allocsize = {int(input.vertex_count), int(input.corner_count),
                                     int(input.corner_count), int(input.face_count)};
  const BMeshCreateParams params = {toolflags};
  out->bm.reset(BM_mesh_create(&allocsize, &params));
  BMesh* bm = out->bm.get();
  BM_data_layer_add_named(bm, &bm->vdata, CD_PROP_INT32, kVertexLayer);
  BM_data_layer_add_named(bm, &bm->pdata, CD_PROP_INT32, kFaceLayer);
  out->vertex_layer = CustomData_get_offset_named(&bm->vdata, CD_PROP_INT32, kVertexLayer);
  out->face_layer = CustomData_get_offset_named(&bm->pdata, CD_PROP_INT32, kFaceLayer);
  BM_data_layer_add_named(bm, &bm->pdata, CD_PROP_INT32, kRoleLayer);
  out->role_layer = CustomData_get_offset_named(&bm->pdata, CD_PROP_INT32, kRoleLayer);

  out->verts.resize(input.vertex_count);
  out->faces.reserve(input.face_count);
  for (size_t i = 0; i < input.vertex_count; ++i) {
    BMVert* vert = BM_vert_create(bm, input.positions + (3 * i), nullptr, BM_CREATE_NOP);
    BM_ELEM_CD_SET_INT(vert, out->vertex_layer, int(i) + 1);
    out->verts[i] = vert;
  }
  std::vector<BMVert*> face_verts;
  size_t offset = 0;
  for (size_t f = 0; f < input.face_count; ++f) {
    const uint32_t size = input.face_sizes[f];
    face_verts.assign(size, nullptr);
    for (uint32_t k = 0; k < size; ++k) {
      face_verts[k] = out->verts[input.corners[offset + k]];
    }
    BMFace* face =
        BM_face_create_verts(bm, face_verts.data(), int(size), nullptr, BM_CREATE_NOP, true);
    if (face == nullptr) {
      return VE_ERR_INTERNAL;
    }
    BM_ELEM_CD_SET_INT(face, out->face_layer, int(f) + 1);
    out->faces.push_back(face);
    offset += size;
  }
  return VE_OK;
}

ve_status store_mesh(const LoadedMesh& loaded, ve_polymesh* out, bool canonical) {
  BMesh* bm = loaded.bm.get();
  if (size_t(bm->totvert) > kMaxVertices || size_t(bm->totface) > kMaxFaces) {
    return VE_ERR_TOO_LARGE;
  }
  BM_mesh_elem_index_ensure(bm, BM_VERT);
  out->positions.clear();
  out->face_sizes.clear();
  out->corners.clear();
  out->vertex_origin.clear();
  out->face_parent.clear();
  out->face_role.clear();
  BMIter iter;
  BMVert* vert;
  BM_ITER_MESH(vert, &iter, bm, BM_VERTS_OF_MESH) {
    out->positions.insert(out->positions.end(), vert->co, vert->co + 3);
    const int origin = BM_ELEM_CD_GET_INT(vert, loaded.vertex_layer);
    out->vertex_origin.push_back(origin > 0 ? uint32_t(origin - 1) : kNoOrigin);
  }
  BMFace* face;
  BM_ITER_MESH(face, &iter, bm, BM_FACES_OF_MESH) {
    out->face_sizes.push_back(uint32_t(face->len));
    const int parent = BM_ELEM_CD_GET_INT(face, loaded.face_layer);
    out->face_parent.push_back(parent > 0 ? uint32_t(parent - 1) : kNoOrigin);
    const int role = BM_ELEM_CD_GET_INT(face, loaded.role_layer);
    out->face_role.push_back(role > 0 ? uint32_t(role) : 0U);
    BMLoop* first = BM_FACE_FIRST_LOOP(face);
    BMLoop* loop = first;
    do {
      out->corners.push_back(uint32_t(BM_elem_index_get(loop->v)));
      loop = loop->next;
    } while (loop != first);
    if (out->corners.size() > kMaxCorners) {
      return VE_ERR_TOO_LARGE;
    }
  }
  if (canonical) {
    canonicalize(out);
  }
  return VE_OK;
}

// Blender's operator system keeps some maps keyed by pointer, so the order in which an operator
// emits its new elements can follow memory addresses and differ between runs and platforms. Sorting
// vertices by position (then origin) and faces by their vertex lists makes the same geometry always
// come out with the same indices.
void canonicalize(ve_polymesh* mesh) {
  const size_t vertex_count = mesh->positions.size() / 3;
  std::vector<uint32_t> order(vertex_count);
  for (size_t i = 0; i < vertex_count; ++i) {
    order[i] = static_cast<uint32_t>(i);
  }
  const auto position_less = [mesh](uint32_t a, uint32_t b) {
    for (size_t axis = 0; axis < 3; ++axis) {
      const float x = mesh->positions[(3 * a) + axis];
      const float y = mesh->positions[(3 * b) + axis];
      if (x != y) {
        return x < y;
      }
    }
    return mesh->vertex_origin[a] < mesh->vertex_origin[b];
  };
  std::stable_sort(order.begin(), order.end(), position_less);
  std::vector<uint32_t> rank(vertex_count);
  std::vector<float> positions(mesh->positions.size());
  std::vector<uint32_t> origin(vertex_count);
  for (size_t new_index = 0; new_index < vertex_count; ++new_index) {
    const size_t old_index = order[new_index];
    rank[old_index] = static_cast<uint32_t>(new_index);
    std::copy_n(&mesh->positions[3 * old_index], 3, &positions[3 * new_index]);
    origin[new_index] = mesh->vertex_origin[old_index];
  }
  // Faces: remap corners, start each loop at its smallest vertex (keeping its direction), then
  // order the faces by their vertex lists.
  struct Face {
    std::vector<uint32_t> corners;
    uint32_t parent;
    uint32_t role;
  };
  std::vector<Face> faces;
  faces.reserve(mesh->face_sizes.size());
  size_t offset = 0;
  for (size_t f = 0; f < mesh->face_sizes.size(); ++f) {
    const uint32_t size = mesh->face_sizes[f];
    Face face{{}, mesh->face_parent[f], mesh->face_role[f]};
    face.corners.reserve(size);
    for (uint32_t k = 0; k < size; ++k) {
      face.corners.push_back(rank[mesh->corners[offset + k]]);
    }
    std::rotate(face.corners.begin(), std::min_element(face.corners.begin(), face.corners.end()),
                face.corners.end());
    faces.push_back(std::move(face));
    offset += size;
  }
  std::stable_sort(faces.begin(), faces.end(), [](const Face& a, const Face& b) {
    return a.corners != b.corners ? a.corners < b.corners : a.parent < b.parent;
  });
  mesh->positions = std::move(positions);
  mesh->vertex_origin = std::move(origin);
  mesh->corners.clear();
  for (size_t f = 0; f < faces.size(); ++f) {
    mesh->face_sizes[f] = static_cast<uint32_t>(faces[f].corners.size());
    mesh->face_parent[f] = faces[f].parent;
    mesh->face_role[f] = faces[f].role;
    mesh->corners.insert(mesh->corners.end(), faces[f].corners.begin(), faces[f].corners.end());
  }
}

}  // namespace ve_blender

extern "C" {

size_t ve_polymesh_num_vertices(const ve_polymesh* mesh) {
  return mesh == nullptr ? 0 : mesh->positions.size() / 3;
}

size_t ve_polymesh_num_faces(const ve_polymesh* mesh) {
  return mesh == nullptr ? 0 : mesh->face_sizes.size();
}

size_t ve_polymesh_num_corners(const ve_polymesh* mesh) {
  return mesh == nullptr ? 0 : mesh->corners.size();
}

ve_status ve_polymesh_copy(const ve_polymesh* mesh, float* positions, size_t position_capacity,
                           uint32_t* face_sizes, size_t face_capacity, uint32_t* corners,
                           size_t corner_capacity, uint32_t* vertex_origin, size_t vertex_capacity,
                           uint32_t* face_parent, size_t parent_capacity) {
  if (mesh == nullptr) {
    return VE_ERR_INVALID_ARG;
  }
  if (position_capacity < mesh->positions.size() || face_capacity < mesh->face_sizes.size() ||
      corner_capacity < mesh->corners.size() || vertex_capacity < mesh->vertex_origin.size() ||
      parent_capacity < mesh->face_parent.size()) {
    return VE_ERR_BUFFER_TOO_SMALL;
  }
  std::copy(mesh->positions.begin(), mesh->positions.end(), positions);
  std::copy(mesh->face_sizes.begin(), mesh->face_sizes.end(), face_sizes);
  std::copy(mesh->corners.begin(), mesh->corners.end(), corners);
  std::copy(mesh->vertex_origin.begin(), mesh->vertex_origin.end(), vertex_origin);
  std::copy(mesh->face_parent.begin(), mesh->face_parent.end(), face_parent);
  return VE_OK;
}

ve_status ve_polymesh_copy_roles(const ve_polymesh* mesh, uint32_t* roles, size_t capacity) {
  if (mesh == nullptr || roles == nullptr) {
    return VE_ERR_INVALID_ARG;
  }
  if (capacity < mesh->face_role.size()) {
    return VE_ERR_BUFFER_TOO_SMALL;
  }
  std::copy(mesh->face_role.begin(), mesh->face_role.end(), roles);
  return VE_OK;
}

void ve_polymesh_free(ve_polymesh* mesh) { const std::unique_ptr<ve_polymesh> owner(mesh); }

}  // extern "C"
