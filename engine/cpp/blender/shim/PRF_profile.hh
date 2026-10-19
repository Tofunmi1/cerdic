// Stand-in for Blender's profiler header: the profiling macros compile to nothing in the engine.
#pragma once
#define PRF_scope(...)
#define PRF_scope_with_name(...)
#define PRF_memory_alloc(...)
#define PRF_memory_free(...)
