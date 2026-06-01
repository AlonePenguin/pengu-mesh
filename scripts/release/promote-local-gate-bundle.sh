#!/bin/zsh
set -euo pipefail

if [[ $# -lt 1 || $# -gt 2 ]]; then
  print -u2 "usage: $0 <source_dir> [dest_dir]"
  exit 64
fi

SOURCE_DIR=${1:A}
DEST_DIR=${2:-"reports/local-gate/${SOURCE_DIR:t}"}
DEST_DIR=${DEST_DIR:A}

if [[ ! -d "$SOURCE_DIR" ]]; then
  print -u2 "source directory not found: $SOURCE_DIR"
  exit 1
fi

mkdir -p "$DEST_DIR"

# Commit the durable proof subset, not the disposable browser profile cache,
# sqlite state, or daemon/browser log noise from the raw gate run.
rsync -a --delete --prune-empty-dirs \
  --include '*/' \
  --include '*.json' \
  --include '*.md' \
  --include '*.txt' \
  --include '*.status' \
  --include '*.png' \
  --include '*.jpg' \
  --include '*.jpeg' \
  --exclude '*.log' \
  --exclude '*.sqlite3' \
  --exclude '**/profiles/**' \
  --exclude '*' \
  "$SOURCE_DIR/" "$DEST_DIR/"

# rsync pattern matching can still preserve nested profile folders when a
# parent directory matched earlier. Remove those generated caches explicitly.
find "$DEST_DIR" -type d -name profiles -prune -exec rm -rf {} +
find "$DEST_DIR" -type d -empty -delete

print "promoted local-gate bundle to $DEST_DIR"
