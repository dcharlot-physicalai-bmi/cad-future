#!/bin/bash
# Build script for physical.openie.dev
# Produces a deployable static site in web/

set -euo pipefail

echo "=== Building OpenIE CAD ==="

# 1. Run tests
echo "[1/3] Running tests..."
cargo test --workspace --quiet

# 2. Build WASM
echo "[2/3] Building WASM (release)..."
wasm-pack build crates/platform/web \
  --target web \
  --out-dir ../../../web/pkg \
  --no-typescript \
  --release

# 3. Report
WASM_SIZE=$(ls -lh web/pkg/physical_web_bg.wasm | awk '{print $5}')
echo "[3/3] Done!"
echo ""
echo "  WASM size: ${WASM_SIZE}"
echo "  Deploy:    web/"
echo "  URL:       https://physical.openie.dev"
echo ""
echo "To serve locally:"
echo "  cd web && python3 -m http.server 8080"
echo "  Open http://localhost:8080"
