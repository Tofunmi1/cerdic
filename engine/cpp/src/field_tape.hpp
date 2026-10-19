#pragma once

#include <cstddef>
#include <cstdint>
#include <vector>

#include "manifold/manifold.h"
#include "ve_kernel.h"

namespace ve::field {

constexpr size_t kMaxFieldInstructions = 4096;

struct Frame {
  uint32_t instruction{};
  manifold::vec3 point;
  double left = 0.0;
  uint8_t state = 0;
};

class TapeEvaluator {
 public:
  double Evaluate(const ve_field_instruction* instructions, size_t root, manifold::vec3 point);

 private:
  std::vector<Frame> frames_;
};

bool valid_tape(const ve_field_instruction* instructions, size_t count);

}  // namespace ve::field
