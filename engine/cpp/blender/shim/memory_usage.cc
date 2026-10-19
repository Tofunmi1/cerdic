// Memory accounting for Blender's guarded allocator, replacing guardedalloc/intern/memory_usage.cc.
//
// Blender's version keeps per-thread counters in a 128-byte-aligned thread_local, which sanitizer
// builds on macOS reject (the runtime does not honour that alignment for thread-local storage) and
// which is pointless in the engine: kernel work runs on one thread. Plain atomics give the same
// numbers, and `ve_blender_memory_in_use` lets the engine read them.
#include <algorithm>
#include <atomic>
#include <cstddef>

#include "MEM_guardedalloc.h"
#include "mallocn_intern.hh"

namespace {
std::atomic<long long> g_blocks{0};
std::atomic<long long> g_bytes{0};
std::atomic<long long> g_peak{0};
}  // namespace

void memory_usage_init() {}

void memory_usage_block_alloc(const size_t size) {
  g_blocks.fetch_add(1, std::memory_order_relaxed);
  const long long now = g_bytes.fetch_add(static_cast<long long>(size), std::memory_order_relaxed) +
                        static_cast<long long>(size);
  long long peak = g_peak.load(std::memory_order_relaxed);
  while (now > peak && !g_peak.compare_exchange_weak(peak, now, std::memory_order_relaxed)) {
  }
}

void memory_usage_block_free(const size_t size) {
  g_blocks.fetch_sub(1, std::memory_order_relaxed);
  g_bytes.fetch_sub(static_cast<long long>(size), std::memory_order_relaxed);
}

size_t memory_usage_block_num() { return static_cast<size_t>(g_blocks.load()); }

size_t memory_usage_current() { return static_cast<size_t>(g_bytes.load()); }

size_t memory_usage_peak() { return static_cast<size_t>(g_peak.load()); }

void memory_usage_peak_reset() { g_peak.store(g_bytes.load()); }

extern "C" size_t ve_blender_memory_in_use() { return memory_usage_current(); }
