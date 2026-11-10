// What the vendored Skin modifier (lib/bl/source/blender/modifiers/intern/MOD_skin.cc) needs from Blender's modifier, mesh and UI headers.
// The algorithm only reads three settings of the modifier, so they are a plain struct here; the Mesh-based entry points are replaced in ops/skin.cc.
#pragma once

#include <cfloat>
#include <cmath>

#include "BLI_array.hh"
#include "BLI_math_vector.hh"
#include "BLI_math_vector_types.hh"
#include "BLI_virtual_array.hh"

#include "DNA_meshdata_types.h"

#include "BKE_deform.hh"
#include "BKE_mesh_mapping.hh"

/* bke::mesh::edge_other_vert from BKE_mesh.hh (not vendored): the vertex of an edge that is not `vert`. */
namespace blender::bke::mesh {
inline int edge_other_vert(const int2 edge, const int vert)
{
  return edge[0] == vert ? edge[1] : edge[0];
}
}  // namespace blender::bke::mesh

/* SkinModifierData in Blender: symmetry axes (bit 0 x, 1 y, 2 z), flags and the branch smoothing factor. */
struct SkinModifierData {
  int symmetry_axes = 0;
  int flag = 0;
  float branch_smoothing = 0.0F;
};
enum {
  MOD_SKIN_SMOOTH_SHADING = (1 << 0),
};
