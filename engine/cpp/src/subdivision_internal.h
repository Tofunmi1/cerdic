#ifndef VE_SUBDIVISION_INTERNAL_H
#define VE_SUBDIVISION_INTERNAL_H

#include <opensubdiv/far/topologyDescriptor.h>
#include <opensubdiv/far/topologyRefiner.h>

#include <cstdint>
#include <vector>

#include "ve_kernel.h"

struct ve_subdivision {
  std::vector<float> positions;
  std::vector<uint32_t> face_sizes;
  std::vector<uint32_t> corners;
  std::vector<uint32_t> vertex_origin_kind;
  std::vector<uint32_t> vertex_origin_index;
  std::vector<uint32_t> face_parent;
  size_t point_channels{};
  std::vector<float> point_values;
  uint32_t fvar_width{};
  std::vector<float> fvar_values;
  std::vector<uint32_t> fvar_corner_values;
};

struct ve_subdivision_prepared {
  std::vector<int> face_sizes;
  std::vector<int> corners;
  std::vector<int> crease_pairs;
  std::vector<int> corner_vertices;
  std::vector<int> fvar_corner_values;
  OpenSubdiv::Far::TopologyDescriptor::FVarChannel fvar_channel;
  size_t predicted_vertices{};
  size_t predicted_faces{};
  size_t predicted_corners{};
};

ve_status ve_subdivision_interpolate_channels(const ve_subdivide_input& input,
                                              const OpenSubdiv::Far::TopologyRefiner& refiner,
                                              ve_subdivision* output);
ve_status ve_subdivision_prepare_channels(const ve_subdivide_input& input,
                                          ve_subdivision_prepared* prepared);

#endif
