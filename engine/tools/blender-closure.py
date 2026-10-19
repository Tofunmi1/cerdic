"""Prints the files a set of Blender sources needs: the union of their quoted-include closures.

usage: blender-closure.py <blender-checkout> <list-of-sources> [more lists]

Used to choose what `third_party/blender` holds and to refresh it (see its VENDOR.md): run it against a
full or sparse Blender checkout, then fetch exactly the printed paths with
`git sparse-checkout set --no-cone --stdin`. Sources whose path contains a SKIP token are not built
(OS, file I/O and editor code we replace with stubs).
"""
import os
import re
import sys
from collections import defaultdict

SKIP = (
    'filereader_zstd', 'filereader_gzip', 'fileops_c', 'serialize.cc', 'system_win32', 'BLI_mmap',
    'BLI_filelist', 'storage.cc', 'blenlib/intern/system.cc', 'expr_pylike_eval', 'uuid.cc',
    'compute_context', 'bmesh_log', 'bmesh_mesh_convert', 'geometry_set', 'timeit', 'index_mask',
    'memory_cache_file_load', 'clog/clog',
)
EXT = ('.c', '.cc', '.cpp', '.h', '.hh', '.hpp', '.inl')
INCLUDE = re.compile(r'^\s*#\s*include\s+"([^"]+)"', re.M)


def main():
    root = sys.argv[1]
    lists = sys.argv[2:]
    by_base = defaultdict(list)
    by_path = set()
    for base, _, files in os.walk(root):
        if '/.git' in base:
            continue
        for name in files:
            if name.endswith(EXT):
                rel = os.path.relpath(os.path.join(base, name), root)
                by_base[name].append(rel)
                by_path.add(rel)

    def resolve(current, name):
        here = os.path.normpath(os.path.join(os.path.dirname(current), name))
        if here in by_path:
            return here
        candidates = by_base.get(os.path.basename(name), [])
        suffix = [c for c in candidates if c.endswith('/' + name) or c == name]
        if suffix:
            return suffix[0]
        return candidates[0] if len(candidates) == 1 else None

    entries = []
    for path in lists:
        for line in open(path):
            rel = line.strip()
            if rel and rel in by_path and not any(token in rel for token in SKIP):
                entries.append(rel)
    seen = set()
    stack = list(entries)
    while stack:
        current = stack.pop()
        if current in seen:
            continue
        seen.add(current)
        text = open(os.path.join(root, current), errors='replace').read()
        for name in INCLUDE.findall(text):
            target = resolve(current, name)
            if target:
                stack.append(target)
    for path in sorted(seen):
        print(path)


if __name__ == '__main__':
    main()
