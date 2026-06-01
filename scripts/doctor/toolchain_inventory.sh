#!/bin/zsh
set -euo pipefail

for tool in git gh rustup cargo rustc go jq sqlite3 security DevToolsSecurity brew node npm pnpm; do
  if command -v "$tool" >/dev/null 2>&1; then
    printf "%s\t%s\n" "$tool" "$(command -v "$tool")"
  else
    printf "%s\tMISSING\n" "$tool"
  fi
done

