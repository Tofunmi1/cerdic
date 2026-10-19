# Vendored Eigen (headers only)
- Upstream: https://gitlab.com/libeigen/eigen
- Commit: 3b70ae5bace4346e9607fa86853aa9ed28b59b7f (2026-10-15, `Core bug fixes: MSA, AltiVec, RVV, LSX and SVE backends`)
- Licence: MPL-2.0 (`COPYING.MPL2`); a few files carry BSD, Apache-2.0 or MINPACK terms (`COPYING.BSD`, `COPYING.APACHE`, `COPYING.MINPACK`, `COPYING.README`).
- What is here: the 424 headers reachable from `Eigen/Core`, `Geometry`, `Dense`, `Eigenvalues`, `SVD`, `Sparse` and their dependencies (about 8.5 MB), fetched with
  git sparse checkout of an exact file list, then the nested `.git` was removed. Nothing is modified.
- Used by: `../blender/intern/eigen` (Blender's linear-solver wrapper) and `../blender/intern/iksolver`.
- Build notes: compile with `-DEIGEN_DONT_VECTORIZE -ffp-contract=off` for identical native and wasm results. This version asks the host for its CPU cache sizes (`sysctlbyname` on macOS) to pick matrix
  block sizes, which changes summation order between machines; call `Eigen::setCpuCacheSizes(32768, 262144, 2097152)` once at start-up everywhere.
