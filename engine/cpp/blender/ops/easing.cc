// Blender's easing curves (BLI_easing) on a 0 to 1 fraction, so the engine's own copies of them can be checked
// against the real code. Kinds: 0 back, 1 bounce, 2 elastic (all "ease out", with Blender's keyframe defaults).
#include <cstdint>

#include "BLI_easing.hh"

extern "C" float ve_blender_ease(int32_t kind, float fraction) {
  switch (kind) {
    case 0:
      return blender::BLI_easing_back_ease_out(fraction, 0.0F, 1.0F, 1.0F, 1.70158F);
    case 1:
      return blender::BLI_easing_bounce_ease_out(fraction, 0.0F, 1.0F, 1.0F);
    case 2:
      return blender::BLI_easing_elastic_ease_out(fraction, 0.0F, 1.0F, 1.0F, 0.8F, 0.3F);
    default:
      return fraction;
  }
}
