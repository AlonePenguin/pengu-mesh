#!/bin/zsh
set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  chrome-dev-navigate.sh <url> [app_name]

Examples:
  ./scripts/browser/chrome-dev-navigate.sh https://example.com
  ./scripts/browser/chrome-dev-navigate.sh chrome://inspect/#remote-debugging "Google Chrome Dev"
EOF
}

target_url="${1:-}"
app_name="${2:-Google Chrome Dev}"

if [[ -z "$target_url" ]]; then
  usage >&2
  exit 2
fi

if [[ "$target_url" == "--help" || "$target_url" == "-h" ]]; then
  usage
  exit 0
fi

osascript - "$app_name" "$target_url" <<'APPLESCRIPT'
on run argv
  set appName to item 1 of argv
  set targetUrl to item 2 of argv

  tell application appName to activate
  delay 0.2

  tell application "System Events"
    keystroke "l" using {command down}
    delay 0.1
    keystroke targetUrl
    delay 0.1
    key code 36
  end tell
end run
APPLESCRIPT
