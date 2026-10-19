# Geometry kernel build: the single source of truth for flags across native, wasm and sanitizer builds.
cmake_minimum_required(VERSION 3.20)
project(ve_kernel LANGUAGES C CXX)

option(VE_BUILD_TESTS "Build the C++ kernel tests" OFF)
option(VE_SANITIZE "Build with ASan and UBSan" OFF)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)
set(CMAKE_POSITION_INDEPENDENT_CODE ON)
set(BUILD_SHARED_LIBS OFF CACHE BOOL "" FORCE)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)
set(CMAKE_ARCHIVE_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/lib")

# Same flags everywhere so native and wasm behave alike: no exceptions, no iostream.
set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} -fno-exceptions -include iterator -include cstdlib")
if(VE_SANITIZE)
  set(SAN_FLAGS "-fsanitize=address,undefined -fno-sanitize-recover=undefined -fno-omit-frame-pointer")
  set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} ${SAN_FLAGS}")
  set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} ${SAN_FLAGS}")
  set(CMAKE_EXE_LINKER_FLAGS "${CMAKE_EXE_LINKER_FLAGS} ${SAN_FLAGS}")
endif()

# Vendored, offline dependencies (see ../third_party/*/VENDOR.md).
set(FETCHCONTENT_FULLY_DISCONNECTED ON CACHE BOOL "" FORCE)
set(FETCHCONTENT_SOURCE_DIR_CLIPPER2 "${CMAKE_CURRENT_SOURCE_DIR}/../third_party/clipper2" CACHE PATH "" FORCE)
set(MANIFOLD_TEST OFF CACHE BOOL "" FORCE)
set(MANIFOLD_PAR OFF CACHE BOOL "" FORCE)
set(MANIFOLD_CBIND OFF CACHE BOOL "" FORCE)
set(MANIFOLD_PYBIND OFF CACHE BOOL "" FORCE)
set(MANIFOLD_JSBIND OFF CACHE BOOL "" FORCE)
set(MANIFOLD_CROSS_SECTION ON CACHE BOOL "" FORCE)
set(MANIFOLD_USE_BUILTIN_CLIPPER2 ON CACHE BOOL "" FORCE)
set(MANIFOLD_NO_IOSTREAM ON CACHE BOOL "" FORCE)
set(MANIFOLD_EXPORT OFF CACHE BOOL "" FORCE)
add_subdirectory("${CMAKE_CURRENT_SOURCE_DIR}/../third_party/manifold" manifold EXCLUDE_FROM_ALL)

file(GLOB MESHOPT_SOURCES "${CMAKE_CURRENT_SOURCE_DIR}/../third_party/meshoptimizer/src/*.cpp")
add_library(ve_meshopt STATIC ${MESHOPT_SOURCES})
target_include_directories(ve_meshopt PUBLIC "${CMAKE_CURRENT_SOURCE_DIR}/../third_party/meshoptimizer/src")

# OpenSubdiv Far CPU Catmull-Clark only. Keep Sdc/Vtr/Far behind ve_kernel's C ABI;
# no Hbr, Bfr, Osd, Ptex, OpenMP, TBB, or GPU backend is compiled.
set(OSD_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/../third_party/opensubdiv")
file(GLOB OSD_SDC_SOURCES "${OSD_ROOT}/opensubdiv/sdc/*.cpp")
file(GLOB OSD_VTR_SOURCES "${OSD_ROOT}/opensubdiv/vtr/*.cpp")
file(GLOB OSD_FAR_SOURCES "${OSD_ROOT}/opensubdiv/far/*.cpp")
add_library(ve_opensubdiv STATIC ${OSD_SDC_SOURCES} ${OSD_VTR_SOURCES} ${OSD_FAR_SOURCES})
target_include_directories(ve_opensubdiv PUBLIC "${OSD_ROOT}")
target_compile_options(ve_opensubdiv PRIVATE
  -Wall -Wextra -Wpedantic -Werror -fno-rtti
  -Wno-gnu-zero-variadic-macro-arguments -Wno-format-pedantic
  -Wno-unused-but-set-variable -Wno-unused-template)

# Selected Blender code (bevel, UV unwrapping, ...) under its own flags; see blender/CMakeLists.txt.
add_subdirectory(blender)

add_library(ve_kernel STATIC src/kernel.cpp src/profiles.cpp src/subdivision.cpp
  src/field_mesh.cpp src/field_tape.cpp src/subdivision_channels.cpp src/tangents.cpp)
target_include_directories(ve_kernel PUBLIC include)
# MikkTSpace (vendored from Blender, Apache-2.0): headers only; SYSTEM so its warnings stay upstream's.
target_include_directories(ve_kernel SYSTEM PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}/../third_party/mikktspace")
target_link_libraries(ve_kernel PUBLIC manifold ve_meshopt ve_opensubdiv ve_blender_ops)
target_compile_options(ve_kernel PRIVATE
  -Wall -Wextra -Wpedantic -Werror -fno-rtti -ffp-contract=off -fexcess-precision=standard)
set_source_files_properties(src/field_mesh.cpp src/field_tape.cpp PROPERTIES COMPILE_OPTIONS "-frtti")
# MikkTSpace's hash helpers type-pun floats, so this file must not assume strict aliasing.
set_source_files_properties(src/tangents.cpp PROPERTIES COMPILE_OPTIONS "-fno-strict-aliasing")

if(VE_BUILD_TESTS)
  add_executable(kernel_test tests/kernel_test.cpp)
  target_link_libraries(kernel_test PRIVATE ve_kernel)
  target_compile_options(kernel_test PRIVATE -Wall -Wextra -Wpedantic -Werror -fno-rtti)
  add_executable(subdivision_test tests/subdivision_test.cpp)
  target_link_libraries(subdivision_test PRIVATE ve_kernel)
  target_compile_options(subdivision_test PRIVATE -Wall -Wextra -Wpedantic -Werror -fno-rtti)
  add_executable(tangents_test tests/tangents_test.cpp)
  target_link_libraries(tangents_test PRIVATE ve_kernel)
  target_compile_options(tangents_test PRIVATE -Wall -Wextra -Wpedantic -Werror -fno-rtti)
  add_executable(bevel_test tests/bevel_test.cpp)
  target_link_libraries(bevel_test PRIVATE ve_kernel)
  target_compile_options(bevel_test PRIVATE -Wall -Wextra -Wpedantic -Werror -fno-rtti)
  add_executable(uv_test tests/uv_test.cpp)
  target_link_libraries(uv_test PRIVATE ve_kernel)
  target_compile_options(uv_test PRIVATE -Wall -Wextra -Wpedantic -Werror -fno-rtti)
  add_executable(field_test tests/field_test.cpp)
  target_link_libraries(field_test PRIVATE ve_kernel)
  target_compile_options(field_test PRIVATE -Wall -Wextra -Wpedantic -Werror -fno-rtti)
endif()
