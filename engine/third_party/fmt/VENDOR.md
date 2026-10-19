# Vendored fmt (headers only)
- Upstream: https://github.com/fmtlib/fmt
- Tag: 12.2.0, commit 1be298e1bd68957e4cd352e1f676f00e07dcfb57
- Licence: MIT (`LICENSE`).
- What is here: `include/fmt` (the header-only library), `LICENSE`, `README.md`, fetched with git sparse checkout, then the nested `.git` was removed. Nothing is modified.
- Used by: `../blender/source/blender/blenkernel/intern/attribute.cc` (`fmt::format_to_n`), compiled with `-DFMT_HEADER_ONLY` so no library is built.
