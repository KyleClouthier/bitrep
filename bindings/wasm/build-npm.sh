#!/usr/bin/env bash
# Build the npm package for bitrep from the wasm-bindgen crate, with the
# published name/metadata patched in. Used both locally and in CI.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"

wasm-pack build "$HERE" --target web --release
PKG="$HERE/pkg"

python3 - "$PKG/package.json" << 'PY'
import json, sys
p = sys.argv[1]
d = json.load(open(p))
d["name"] = "bitrep"
d["description"] = "Exact, order-invariant, bit-identical floating-point reductions and CRDTs (Rust core, via WebAssembly)."
d["license"] = "MIT OR Apache-2.0"
d["repository"] = {"type": "git", "url": "git+https://github.com/KyleClouthier/bitrep.git"}
d["homepage"] = "https://simgen.dev/bitrep"
d["keywords"] = ["deterministic", "reproducible", "summation", "crdt", "exact-arithmetic", "wasm"]
json.dump(d, open(p, "w"), indent=2)
PY

cp "$HERE/README.md" "$PKG/README.md"
echo "npm package ready at $PKG (name=bitrep)"
