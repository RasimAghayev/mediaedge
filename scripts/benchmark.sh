#!/usr/bin/env bash
set -euo pipefail

echo "==> MediaEdge benchmark"
cargo criterion --workspace --output-format html
