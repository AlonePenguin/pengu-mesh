#!/bin/zsh
set -euo pipefail

printf "timestamp=%s\n" "$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
printf "kernel=%s\n" "$(uname -a)"
printf "arch=%s\n" "$(uname -m)"
printf "os=%s\n" "$(sw_vers -productVersion)"
printf "rustup=%s\n" "$("$HOME/.cargo/bin/rustup" show active-toolchain 2>/dev/null || echo missing)"
printf "go=%s\n" "$(go version 2>/dev/null || echo missing)"

