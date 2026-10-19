# OpenSubdiv vendoring record

- Upstream: [Pixar OpenSubdiv](https://github.com/PixarAnimationStudios/OpenSubdiv)
- Release: 3.7.0
- Commit: `9dab8a47bfbb1388ec8388fe61f5f916e6123f38`
- Licence: Tomorrow Open Source Technology License 1.0; see `LICENSE.txt` and `NOTICE.txt`.
- Vendored components: `opensubdiv/sdc`, `opensubdiv/vtr`, `opensubdiv/far`, and the pinned
  `opensubdiv/version.h` metadata header.
- Excluded components: Hbr, Bfr, Osd/device backends, tutorials, examples, regression suite,
  documentation, Ptex, OpenMP and TBB integrations.
- Local source modifications: none; `version.h` contains the upstream 3.7.0 version constants.
- Build adaptation: the engine's `cpp/CMakeLists.txt` compiles the three CPU source sets as one
  static library with the existing no-exceptions flags and no RTTI. This avoids upstream's full
  optional-backend CMake configuration.

The upstream version is pinned by this file and the source commit. To refresh it, update both and
repeat the native/WASI compile and G7 reference, licence, size, and parity checks.
