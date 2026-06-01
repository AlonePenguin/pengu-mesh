#!/bin/zsh
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"

"${repo_root}/scripts/browser/chrome-dialog-click.sh" "Allow" "Google Chrome Dev" "${1:-60}" "${2:-0.2}"
