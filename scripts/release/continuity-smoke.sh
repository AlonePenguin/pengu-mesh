#!/bin/zsh
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$repo_root"

if [[ -x "${HOME}/.cargo/bin/cargo" ]]; then
  cargo_bin="${HOME}/.cargo/bin/cargo"
else
  cargo_bin="$(command -v cargo)"
fi

output_dir="${1:-$(mktemp -d "${TMPDIR:-/tmp}/pengu-mesh-continuity-smoke.XXXXXX")}"
runtime_root="${output_dir}/runtime-root"
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

path = sys.argv[1]
with open(path, "r", encoding="utf-8") as handle:
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

fetch_json() {
  local bind_addr="$1"
  local path="$2"
  local output_file="$3"
  local attempt=0
  while (( attempt < 50 )); do
    if /usr/bin/curl --fail --silent --show-error "http://${bind_addr}${path}" >"${output_file}"; then
      return 0
    fi
    /bin/sleep 0.1
    attempt=$((attempt + 1))
  done
  /usr/bin/curl --fail --silent --show-error "http://${bind_addr}${path}" >"${output_file}"
}

write_synthetic_restart_state() {
  local operator_id="$1"
  local db_path="${runtime_root}/runtime.sqlite3"
  local now
  now="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
/usr/bin/python3 - "${db_path}" "${operator_id}" "${now}" <<'PY'
import json
import sqlite3
import sys
import uuid
from datetime import datetime, timedelta, timezone

db_path, operator_id, now = sys.argv[1:4]
instance_id = "inst_restart_smoke"
debug_http_url = "http://127.0.0.1:65534"
browser_ws_url = "ws://127.0.0.1:65534/devtools/browser/restart-smoke"
expires_at = (
    datetime.now(timezone.utc) + timedelta(minutes=5)
).strftime("%Y-%m-%dT%H:%M:%SZ")

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
        instance_id,
        "restart smoke",
        "chrome_dev",
        json.dumps("managed"),
        json.dumps("running"),
        debug_http_url,
        browser_ws_url,
        None,
        None,
        None,
        None,
        now,
        now,
    ),
)
cur.execute(
    """
    INSERT OR REPLACE INTO leases (
        id, resource_kind, resource_id, holder_id, holder_label, mode,
        granted_at, expires_at, last_heartbeat_at
    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
    """,
    (
        f"lease_{uuid.uuid4().hex}",
        "instance",
        instance_id,
        operator_id,
        "pengu-mesh-daemon",
        json.dumps("writer"),
        now,
        expires_at,
        now,
    ),
)
conn.commit()
conn.close()
PY
}

assert_continuity_payload() {
  local json_path="$1"
  local expected_operator="$2"
  /usr/bin/python3 - "${json_path}" "${expected_operator}" <<'PY'
import json
import sys

json_path, expected_operator = sys.argv[1:3]
with open(json_path, "r", encoding="utf-8") as handle:
    payload = json.load(handle)

if not payload.get("ok"):
    raise SystemExit(f"health request failed: {payload}")

data = payload["data"]
continuity = data["continuity"]
assert data["operator_id"] == expected_operator, (data["operator_id"], expected_operator)
assert continuity["continuity_enabled"] is True, continuity
assert continuity["recovered_run"] is True, continuity
assert continuity["reused_operator_id"] is True, continuity
assert continuity["recovered_lease_count"] == 1, continuity
assert continuity["stale_instance_count"] == 1, continuity
assert "inst_restart_smoke" in continuity["stale_instance_ids"], continuity

matching_instances = [
    instance for instance in data["instances"]
    if instance["id"] == "inst_restart_smoke"
]
assert len(matching_instances) == 1, data["instances"]
assert matching_instances[0]["status"] == "closed", matching_instances[0]
assert matching_instances[0]["last_error"], matching_instances[0]

matching_leases = [
    lease for lease in data["leases"]
    if lease["holder_id"] == expected_operator and lease["resource_id"] == "inst_restart_smoke"
]
assert len(matching_leases) == 1, data["leases"]
PY
}

assert_recovery_event() {
  local json_path="$1"
  /usr/bin/python3 - "${json_path}" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)

if not payload.get("ok"):
    raise SystemExit(f"events request failed: {payload}")

events = payload["data"]["events"]
assert any(
    event["category"] == "runtime" and event["action"] == "bootstrap_recovered"
    for event in events
), events
PY
}

start_daemon "${output_dir}/daemon-first.log"
first_bind="$(wait_for_daemon_metadata)"
fetch_json "${first_bind}" "/health" "${output_dir}/health-first.json"

first_operator="$(
  /usr/bin/python3 - "${output_dir}/health-first.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
print(payload["data"]["operator_id"])
PY
)"

cleanup
daemon_pid=""

write_synthetic_restart_state "${first_operator}"

start_daemon "${output_dir}/daemon-second.log"
second_bind="$(wait_for_daemon_metadata)"
fetch_json "${second_bind}" "/health" "${output_dir}/health-second.json"
fetch_json "${second_bind}" "/doctor" "${output_dir}/doctor-second.json"
fetch_json "${second_bind}" "/events?limit=10" "${output_dir}/events-second.json"

assert_continuity_payload "${output_dir}/health-second.json" "${first_operator}"
assert_continuity_payload "${output_dir}/doctor-second.json" "${first_operator}"
assert_recovery_event "${output_dir}/events-second.json"

cat > "${output_dir}/summary.md" <<EOF
# Continuity Smoke

- runtime_root: ${runtime_root}
- first_bind: ${first_bind}
- second_bind: ${second_bind}
- verified:
  - daemon operator identity reused across restart
  - active run recovered on daemon restart
  - daemon-owned lease recovered after restart
  - stale instance surfaced through health and doctor continuity metadata
  - bootstrap recovery event emitted through the HTTP event tail
EOF

printf '%s\n' "${output_dir}"
