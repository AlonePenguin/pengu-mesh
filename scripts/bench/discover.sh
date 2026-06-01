#!/bin/zsh
set -euo pipefail

printf "bench-packages\n"
printf "  - pengu-mesh-bench-json (response envelope serialization)\n"
printf "  - pengu-mesh-bench-cdp (target catalog parse path)\n"
printf "  - pengu-mesh-bench-persistence (runtime state, event tail, manifest-only replay, portable replay serialization)\n"
printf "  - pengu-mesh-bench-artifacts (artifact write-to-disk, crop-grid derivation, recording archive materialization, checksum, materialization)\n"
