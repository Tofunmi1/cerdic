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

add_library(ve_kernel STATIC src/kernel.cpp src/profiles.cpp)
target_include_directories(ve_kernel PUBLIC include)
target_link_libraries(ve_kernel PUBLIC manifold ve_meshopt)
target_compile_options(ve_kernel PRIVATE -Wall -Wextra -Wpedantic -Werror)

if(VE_BUILD_TESTS)
  add_executable(kernel_test tests/kernel_test.cpp)
  target_link_libraries(kernel_test PRIVATE ve_kernel)
  target_compile_options(kernel_test PRIVATE -Wall -Wextra -Wpedantic -Werror)
endif()
