#!/bin/zsh
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$repo_root"

if [[ -x "${HOME}/.cargo/bin/cargo" ]]; then
  cargo_bin="${HOME}/.cargo/bin/cargo"
else
  cargo_bin="$(command -v cargo)"
fi

output_dir="${1:-$(mktemp -d "${TMPDIR:-/tmp}/pengu-mesh-lease-smoke.XXXXXX")}"
runtime_root="${output_dir}/runtime-root"
rm -rf "${runtime_root}"
mkdir -p "$output_dir" "$runtime_root"

daemon_pid=""

cleanup() {
  if [[ -n "${daemon_pid}" ]] && kill -0 "${daemon_pid}" 2>/dev/null; then
    kill "${daemon_pid}" 2>/dev/null || true
    wait "${daemon_pid}" 2>/dev/null || true
  fi
}

trap cleanup EXIT INT TERM

start_daemon() {
  local log_file="$1"
  rm -f "${runtime_root}/daemon.json"
  PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
    "${cargo_bin}" run -p pengu-mesh -- serve --bind 127.0.0.1:0 >"${log_file}" 2>&1 &
  daemon_pid="$!"
}

wait_for_daemon_metadata() {
  local metadata_path="${runtime_root}/daemon.json"
  local attempt=0
  while (( attempt < 200 )); do
    if [[ -f "${metadata_path}" ]]; then
      local bind_addr
      bind_addr="$(
        /usr/bin/python3 - "${metadata_path}" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    data = json.load(handle)
print(data.get("bind_addr", ""))
PY
      )"
      if [[ -n "${bind_addr}" ]]; then
        printf '%s\n' "${bind_addr}"
        return 0
      fi
    fi
    /bin/sleep 0.1
    attempt=$((attempt + 1))
  done
  echo "timed out waiting for daemon metadata" >&2
  return 1
}

post_json() {
  local bind_addr="$1"
  local path="$2"
  local body="$3"
  local output_prefix="$4"
  local http_status
  http_status="$(
    /usr/bin/curl \
      --silent \
      --show-error \
      --output "${output_prefix}.json" \
      --write-out "%{http_code}" \
      --header "Content-Type: application/json" \
      --request POST \
      --data "${body}" \
      "http://${bind_addr}${path}"
  )"
  printf '%s\n' "${http_status}" > "${output_prefix}.status"
}

get_json() {
  local bind_addr="$1"
  local path="$2"
  local output_prefix="$3"
  local http_status
  http_status="$(
    /usr/bin/curl \
      --silent \
      --show-error \
      --output "${output_prefix}.json" \
      --write-out "%{http_code}" \
      "http://${bind_addr}${path}"
  )"
  printf '%s\n' "${http_status}" > "${output_prefix}.status"
}

seed_instance() {
  local db_path="${runtime_root}/runtime.sqlite3"
  /usr/bin/python3 - "${db_path}" <<'PY'
import json
import sqlite3
import sys
from datetime import datetime, timezone

db_path = sys.argv[1]
now = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")

conn = sqlite3.connect(db_path)
cur = conn.cursor()
cur.execute(
    """
    INSERT OR REPLACE INTO instances (
        id, name, channel, mode, status, debug_http_url, browser_ws_url,
        profile_id, profile_path, pid, last_error, created_at, updated_at
    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    """,
    (
        "inst_lease_smoke",
        "lease smoke",
        "chrome_dev",
        json.dumps("managed"),
        json.dumps("running"),
        "http://127.0.0.1:65533",
        "ws://127.0.0.1:65533/devtools/browser/lease-smoke",
        None,
        None,
        None,
        None,
        now,
        now,
    ),
)
conn.commit()
conn.close()
PY
}

assert_payloads() {
  /usr/bin/python3 - "${output_dir}" <<'PY'
import json
import pathlib
import sys

output_dir = pathlib.Path(sys.argv[1])

def load(name):
    with open(output_dir / f"{name}.json", "r", encoding="utf-8") as handle:
        return json.load(handle)

def status(name):
    return int((output_dir / f"{name}.status").read_text(encoding="utf-8").strip())

writer = load("lease-acquire-writer")
observer = load("lease-acquire-observer")
leases = load("lease-status")
writer_conflict = load("lease-acquire-writer-conflict")
mutation_conflict = load("tab-open-conflict")

assert status("lease-acquire-writer") == 200, status("lease-acquire-writer")
assert writer["ok"] is True, writer
assert writer["data"]["lease"]["holder_id"] == "agent_alpha", writer
assert writer["data"]["lease"]["mode"] == "writer", writer

assert status("lease-acquire-observer") == 200, status("lease-acquire-observer")
assert observer["ok"] is True, observer
assert observer["data"]["lease"]["holder_id"] == "agent_beta", observer
assert observer["data"]["lease"]["mode"] == "observer", observer

assert status("lease-status") == 200, status("lease-status")
holder_modes = {(lease["holder_id"], lease["mode"]) for lease in leases["data"]["leases"]}
assert ("agent_alpha", "writer") in holder_modes, holder_modes
assert ("agent_beta", "observer") in holder_modes, holder_modes

assert status("lease-acquire-writer-conflict") == 409, status("lease-acquire-writer-conflict")
assert writer_conflict["ok"] is False, writer_conflict
assert writer_conflict["code"] == "conflict", writer_conflict
assert "held by agent_alpha" in writer_conflict["message"], writer_conflict

assert status("tab-open-conflict") == 409, status("tab-open-conflict")
assert mutation_conflict["ok"] is False, mutation_conflict
assert mutation_conflict["code"] == "conflict", mutation_conflict
assert "held by agent_alpha" in mutation_conflict["message"], mutation_conflict
PY
}

start_daemon "${output_dir}/daemon.log"
bind_addr="$(wait_for_daemon_metadata)"
seed_instance

post_json "${bind_addr}" "/leases/acquire" '{"instance_id":"inst_lease_smoke","holder_id":"agent_alpha","holder_label":"Alpha","mode":"writer","ttl_seconds":120}' "${output_dir}/lease-acquire-writer"
post_json "${bind_addr}" "/leases/acquire" '{"instance_id":"inst_lease_smoke","holder_id":"agent_beta","holder_label":"Beta","mode":"observer","ttl_seconds":120}' "${output_dir}/lease-acquire-observer"
get_json "${bind_addr}" "/leases?instance_id=inst_lease_smoke" "${output_dir}/lease-status"
post_json "${bind_addr}" "/leases/acquire" '{"instance_id":"inst_lease_smoke","holder_id":"agent_gamma","holder_label":"Gamma","mode":"writer","ttl_seconds":120}' "${output_dir}/lease-acquire-writer-conflict"
post_json "${bind_addr}" "/tabs/open" '{"instance_id":"inst_lease_smoke","url":"https://example.com","holder_id":"agent_gamma"}' "${output_dir}/tab-open-conflict"

assert_payloads

cat > "${output_dir}/summary.md" <<EOF
# Lease Smoke

- runtime_root: ${runtime_root}
- bind_addr: ${bind_addr}
- verified:
  - writer lease acquisition succeeds through HTTP
  - observer lease can coexist with an active writer
  - second writer acquisition returns typed \`conflict\`
  - mutation by a non-holder returns typed \`conflict\`
EOF

printf '%s\n' "${output_dir}"
