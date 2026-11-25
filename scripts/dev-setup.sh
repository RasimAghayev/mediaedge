#!/usr/bin/env bash
set -euo pipefail

echo "==> MediaEdge dev setup"
echo "Installing Rust (if missing)..."
if ! command -v cargo &> /dev/null; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
  source "$HOME/.cargo/env"
fi

echo "Installing cargo-chef and cargo-nextest..."
cargo install cargo-chef cargo-nextest

echo "Starting dev stack..."
make up

echo "==> MediaEdge dev setup complete. Run 'make test' to verify."
