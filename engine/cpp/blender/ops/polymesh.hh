// Polygon meshes crossing the C ABI, and their conversion to and from Blender's BMesh.
//
// Every operator built on Blender's bmesh goes through this bridge: the input is validated here
// (finite positions, in-range and distinct corners, size caps), loaded into a BMesh that carries
// two integer layers recording which input vertex and face each element descends from, and read
// back out with the same provenance. Blender's operators assume valid input and assert on bad
// input, so nothing reaches them unchecked.
#pragma once

#include <cstddef>
#include <cstdint>
#include <memory>
#include <vector>

#include "bmesh.hh"
#include "ve_kernel.h"

// The result of a bmesh operator, in the layout of the input.
struct ve_polymesh {
  std::vector<float> positions;         // 3 floats per vertex
  std::vector<uint32_t> face_sizes;     // corners per face
  std::vector<uint32_t> corners;        // vertex index per corner, face after face
  std::vector<uint32_t> vertex_origin;  // input vertex a vertex descends from, or kNoOrigin
  std::vector<uint32_t> face_parent;    // input face a face inherits attributes from, or kNoOrigin
  std::vector<uint32_t> face_role;      // 0 none, 1 inner or moved cap, 2 ring or side wall
};

namespace ve_blender {

constexpr uint32_t kNoOrigin = 0xFFFFFFFFU;
constexpr size_t kMaxVertices = 1'000'000;
constexpr size_t kMaxFaces = 1'000'000;
constexpr size_t kMaxCorners = 4'000'000;

// Checks an input mesh without building anything (finite positions, in-range and distinct corners,
// size caps): VE_OK or the status describing the first problem.
[[nodiscard]] ve_status validate_mesh(const ve_polymesh_input& input);

struct BMeshDeleter {
  void operator()(blender::BMesh* bm) const;
};
using BMeshPtr = std::unique_ptr<blender::BMesh, BMeshDeleter>;

// A BMesh loaded from the input, with the provenance layers filled in (1-based; 0 means none).
struct LoadedMesh {
  BMeshPtr bm;
  std::vector<blender::BMVert*> verts;  // by input index
  std::vector<blender::BMFace*> faces;  // by input index
  int vertex_layer = -1;
  int face_layer = -1;
  int role_layer = -1;  // what an operator made a face: see ve_polymesh::face_role
};

// Validates `input` and builds the BMesh. Returns VE_OK, or the status describing the first
// problem. Operators run through Blender's operator system (BMO) need `toolflags`.
[[nodiscard]] ve_status load_mesh(const ve_polymesh_input& input, LoadedMesh* out,
                                  bool toolflags = false);

// Reads the BMesh back out (indices re-numbered in iteration order). Fails with VE_ERR_TOO_LARGE
// past the caps.
// With `canonical`, vertices and faces are put in an order that depends only on the geometry.
[[nodiscard]] ve_status store_mesh(const LoadedMesh& loaded, ve_polymesh* out,
                                   bool canonical = false);

// Sorts a mesh's vertices and faces into the canonical order.
void canonicalize(ve_polymesh* mesh);

}  // namespace ve_blender
