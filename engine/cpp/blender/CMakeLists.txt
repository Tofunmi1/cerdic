# Blender code compiled into the engine (see ../../third_party/blender/VENDOR.md).
#
# Blender is built with its own flags: C++20, RTTI on, no exceptions (try/catch/throw are macro-neutralised),
# unsigned char, and no fused multiply-add, so native and wasm give the same bits. Its sin/cos/tan/asin/
# acos/atan/atan2 calls are renamed to the deterministic versions in shim/det_libm.cc; the rest of the
# engine keeps the platform libm. Blender's own warnings are not ours to fix, so they are switched off.
include("${CMAKE_CURRENT_SOURCE_DIR}/sources.cmake")

set(BLENDER "${CMAKE_CURRENT_SOURCE_DIR}/../../third_party/blender")
set(EIGEN "${CMAKE_CURRENT_SOURCE_DIR}/../../third_party/eigen")
set(FMT "${CMAKE_CURRENT_SOURCE_DIR}/../../third_party/fmt")
set(SHIM "${CMAKE_CURRENT_SOURCE_DIR}/shim")

set(VE_BLENDER_INCLUDES
  "${SHIM}"
  "${BLENDER}/source/blender/blenkernel/intern"
  "${BLENDER}/source/blender/nodes"
  "${BLENDER}/source/blender/bmesh/tools"
  "${BLENDER}/source/blender/bmesh/intern"
  "${BLENDER}/source/blender/blenlib"
  "${BLENDER}/source/blender/bmesh"
  "${BLENDER}/source/blender/blenkernel"
  "${BLENDER}/source/blender/geometry"
  "${BLENDER}/source/blender/makesdna"
  "${BLENDER}/source/blender/makesrna"
  "${BLENDER}/source/blender/blentranslation"
  "${BLENDER}/source/blender/blenloader"
  "${BLENDER}/source/blender/blenloader_core"
  "${BLENDER}/source/blender/imbuf"
  "${BLENDER}/source/blender/depsgraph"
  "${BLENDER}/source/blender/functions"
  "${BLENDER}/source/blender/gpu"
  "${BLENDER}/intern/guardedalloc"
  "${BLENDER}/intern/guardedalloc/intern"
  "${BLENDER}/intern/eigen"
  "${BLENDER}/intern/clog"
  "${BLENDER}/intern/atomic"
  "${BLENDER}/intern/utfconv"
  "${BLENDER}/extern/wcwidth"
  "${BLENDER}/source/blender"
  "${BLENDER}/source"
  "${EIGEN}"
  "${FMT}/include"
)

# The libm functions whose results differ between platforms and that Blender's code uses.
set(VE_DET_LIBM_FUNCTIONS sin cos tan asin acos atan atan2)
set(VE_BLENDER_DEFINITIONS EIGEN_DONT_VECTORIZE FMT_HEADER_ONLY)
# Exception-safe containers still contain try/catch/throw; with exceptions off they become plain control flow.
# (Passed as options, because CMake would quote parentheses in a definition and change the macro.)
set(VE_BLENDER_EXCEPTION_MACROS "SHELL:-Dtry=if(true)" "SHELL:-Dcatch(...)=else" "SHELL:-Dthrow=abort()")
foreach(fn IN LISTS VE_DET_LIBM_FUNCTIONS)
  list(APPEND VE_BLENDER_DEFINITIONS
    "${fn}=ve_det_${fn}" "${fn}f=ve_det_${fn}f"
    "__builtin_${fn}=ve_det_${fn}" "__builtin_${fn}f=ve_det_${fn}f")
endforeach()
if(CMAKE_SYSTEM_NAME STREQUAL "WASI")
  list(APPEND VE_BLENDER_DEFINITIONS _WASI_EMULATED_SIGNAL _WASI_EMULATED_PROCESS_CLOCKS memalign=aligned_alloc)
endif()

function(ve_blender_options name)
  set_target_properties(${name} PROPERTIES CXX_STANDARD 20 C_STANDARD 11)
  target_include_directories(${name} SYSTEM PRIVATE ${VE_BLENDER_INCLUDES})
  target_compile_definitions(${name} PRIVATE ${VE_BLENDER_DEFINITIONS})
  target_compile_options(${name} PRIVATE
    -w -frtti -funsigned-char -ffp-contract=off ${VE_BLENDER_EXCEPTION_MACROS}
    "$<$<COMPILE_LANGUAGE:CXX>:SHELL:-include cmath>"
    "$<$<COMPILE_LANGUAGE:CXX>:SHELL:-include memory>"
    "$<$<COMPILE_LANGUAGE:CXX>:SHELL:-include array>")
endfunction()

# A library of Blender sources, named relative to third_party/blender.
function(ve_blender_library name)
  set(sources "")
  foreach(source IN LISTS ARGN)
    list(APPEND sources "${BLENDER}/${source}")
  endforeach()
  add_library(${name} STATIC ${sources})
  ve_blender_options(${name})
endfunction()

# The deterministic maths and Blender's logging/global stubs are ours: normal engine flags.
add_library(ve_blender_math STATIC shim/det_libm.cc)
target_compile_options(ve_blender_math PRIVATE -Wall -Wextra -Wpedantic -Werror -fno-rtti -ffp-contract=off)

ve_blender_library(ve_blender_core ${VE_BLENDER_CORE_SOURCES} extern/wcwidth/wcwidth.c)
target_sources(ve_blender_core PRIVATE shim/ve_stubs.cc shim/memory_usage.cc)
ve_blender_library(ve_blender_bmesh ${VE_BLENDER_BMESH_SOURCES})
ve_blender_library(ve_blender_geometry ${VE_BLENDER_GEOMETRY_SOURCES})
target_link_libraries(ve_blender_bmesh PUBLIC ve_blender_core ve_blender_math)
target_link_libraries(ve_blender_geometry PUBLIC ve_blender_core ve_blender_math)

# Blender functions the engine deliberately does not link (file I/O, undo, editors, ...). Each one named in
# unresolved.txt is defined as a trap, so a call that does reach one stops the program instead of
# jumping to nothing. The list is exact: a missing name fails the link, and a stub that no code path
# references is dropped by the final garbage-collecting link.
file(STRINGS "${CMAKE_CURRENT_SOURCE_DIR}/unresolved.txt" VE_UNRESOLVED)
set(VE_UNRESOLVED_C "${CMAKE_CURRENT_BINARY_DIR}/unresolved_stubs.c")
set(content "/* Generated from unresolved.txt by CMake. */\n")
set(index 0)
# unresolved.txt holds Itanium names without their leading underscore (ZN7blender...). An asm label is
# used verbatim, so the stub adds that underscore, and Mach-O one more.
set(symbol_prefix "_")
if(APPLE)
  set(symbol_prefix "__")
endif()
foreach(symbol IN LISTS VE_UNRESOLVED)
  string(APPEND content "void ve_unresolved_${index}(void) __asm__(\"${symbol_prefix}${symbol}\");\n")
  string(APPEND content "void ve_unresolved_${index}(void) { __builtin_trap(); }\n")
  math(EXPR index "${index}+1")
endforeach()
file(WRITE "${VE_UNRESOLVED_C}" "${content}")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${CMAKE_CURRENT_SOURCE_DIR}/unresolved.txt")
add_library(ve_blender_unresolved STATIC "${VE_UNRESOLVED_C}")
target_compile_options(ve_blender_unresolved PRIVATE -w)
# WebAssembly gives every unresolved function a stub of exactly the right type when the link ignores
# unresolved symbols (.cargo/config.toml), which hand-written stubs cannot match, so only native links
# these. `cargo xtask cpp` checks that the wasm build leaves exactly the symbols listed here.
if(NOT CMAKE_SYSTEM_NAME STREQUAL "WASI")
  target_link_libraries(ve_blender_core PUBLIC ve_blender_unresolved)
endif()

# Our operators over Blender's data structures (C ABI in ve_kernel.h). They include Blender headers, so
# they are built with Blender's flags, not the kernel's.
add_library(ve_blender_ops STATIC ops/polymesh.cc ops/bevel.cc ops/mesh_ops.cc ops/uv_unwrap.cc ops/easing.cc)
ve_blender_options(ve_blender_ops)
target_include_directories(ve_blender_ops PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}/ops"
  "${CMAKE_CURRENT_SOURCE_DIR}/../include")
target_link_libraries(ve_blender_ops PUBLIC ve_blender_bmesh ve_blender_geometry)
