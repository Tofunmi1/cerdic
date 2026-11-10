#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/../../../../../"
out="${1:-target/reports/character-head/obj}"
report="$(dirname "$out")/audit.json"
mkdir -p "$(dirname "$report")"
cargo run -p ve-modeling --example dump_obj -- web/tests/scenes/characters/head_study/world.txt "$out"
bun web/tests/scenes/characters/head_study/audit.ts "$out" --height 1.72 --json-out "$report"
echo "JSON report: $report"
