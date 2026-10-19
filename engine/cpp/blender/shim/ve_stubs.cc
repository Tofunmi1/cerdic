// Blender's logging and global state, minimal and silent: the engine reports problems through typed
// diagnostics, never through Blender's logger.
#include <cstdarg>

#include "BKE_global.hh"
#include "CLG_log.h"

namespace blender {
Global G;
}  // namespace blender

void CLG_logf(const CLG_LogType*, CLG_Level, const char*, const char*, const char*, ...) {}
void CLG_log_str(const CLG_LogType*, CLG_Level, const char*, const char*, const char*) {}
void CLG_log_raw(const CLG_LogType*, const char*) {}
void CLG_level_set(CLG_Level) {}
void CLG_logref_init(CLG_LogRef*) {}
void CLG_logref_register(CLG_LogRef*) {}
