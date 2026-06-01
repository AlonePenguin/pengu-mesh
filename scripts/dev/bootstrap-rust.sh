#!/bin/zsh
set -euo pipefail

if ! command -v rustup >/dev/null 2>&1; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
fi

"$HOME/.cargo/bin/rustup" toolchain install stable
"$HOME/.cargo/bin/rustup" default stable
"$HOME/.cargo/bin/rustup" component add rustfmt clippy
"$HOME/.cargo/bin/rustup" show active-toolchain

