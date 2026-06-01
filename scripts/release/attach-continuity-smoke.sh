#!/bin/zsh
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$repo_root"

if [[ -x "${HOME}/.cargo/bin/cargo" ]]; then
  cargo_bin="${HOME}/.cargo/bin/cargo"
else
  cargo_bin="$(command -v cargo)"
fi

output_dir="${1:-$(mktemp -d "${TMPDIR:-/tmp}/pengu-mesh-attach-smoke.XXXXXX")}"
runtime_root="${output_dir}/runtime-root"
server_state="${output_dir}/debug-server-state.json"
server_port_file="${output_dir}/debug-server-port.txt"
rm -rf "${runtime_root}" "${server_state}" "${server_port_file}"
mkdir -p "$output_dir" "$runtime_root"

daemon_pid=""
server_pid=""

cleanup() {
  if [[ -n "${daemon_pid}" ]] && kill -0 "${daemon_pid}" 2>/dev/null; then
    kill "${daemon_pid}" 2>/dev/null || true
    wait "${daemon_pid}" 2>/dev/null || true
  fi
  if [[ -n "${server_pid}" ]] && kill -0 "${server_pid}" 2>/dev/null; then
    kill "${server_pid}" 2>/dev/null || true
    wait "${server_pid}" 2>/dev/null || true
  fi
}

trap cleanup EXIT INT TERM

start_debug_server() {
  /usr/bin/python3 - "${server_state}" "${server_port_file}" >"${output_dir}/debug-server.log" 2>&1 <<'PY' &
import http.server
import json
import socketserver
import sys
import threading
import time

state_path = sys.argv[1]
port_path = sys.argv[2]

class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        with open(state_path, "r", encoding="utf-8") as handle:
            state = json.load(handle)
        port = self.server.server_address[1]
        if self.path == "/json/version":
            payload = {
                "Browser": "Chrome Dev 136.0",
                "Protocol-Version": "1.3",
                "User-Agent": "Attach Smoke",
                "webSocketDebuggerUrl": f"ws://127.0.0.1:{port}/devtools/browser/{state['suffix']}",
            }
            body = json.dumps(payload).encode("utf-8")
            self.send_response(200)
        elif self.path == "/json/list":
            payload = [{
                "id": "ATTACH-SMOKE-PAGE",
                "title": "Attach Smoke Page",
                "url": "https://example.com/attach-smoke",
                "type": "page",
                "webSocketDebuggerUrl": f"ws://127.0.0.1:{port}/devtools/page/ATTACH-SMOKE-PAGE",
            }]
            body = json.dumps(payload).encode("utf-8")
            self.send_response(200)
        else:
            body = b"{}"
            self.send_response(404)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, fmt, *args):
        return

with open(state_path, "w", encoding="utf-8") as handle:
    json.dump({"suffix": "live-one"}, handle)

server = socketserver.TCPServer(("127.0.0.1", 0), Handler)
with open(port_path, "w", encoding="utf-8") as handle:
    handle.write(str(server.server_address[1]))
thread = threading.Thread(target=server.serve_forever, daemon=True)
thread.start()
try:
    while True:
        time.sleep(1)
except KeyboardInterrupt:
    pass
finally:
    server.shutdown()
    server.server_close()
PY
  server_pid="$!"
}

wait_for_debug_server() {
  local attempt=0
  while (( attempt < 200 )); do
    if [[ -f "${server_port_file}" ]]; then
      local port
      port="$(<"${server_port_file}")"
      if [[ -n "${port}" ]]; then
        printf '%s\n' "${port}"
        return 0
      fi
    fi
    /bin/sleep 0.05
    attempt=$((attempt + 1))
  done
  echo "timed out waiting for debug server" >&2
  return 1
}

set_debug_suffix() {
  local suffix="$1"
  /usr/bin/python3 - "${server_state}" "${suffix}" <<'PY'
import json
import sys

state_path, suffix = sys.argv[1:3]
with open(state_path, "w", encoding="utf-8") as handle:
    json.dump({"suffix": suffix}, handle)
PY
}

start_daemon() {
  local log_file="$1"
  rm -f "${runtime_root}/daemon.json"
  PENGU_MESH_ALLOW_EXTERNAL_ATTACH=1 \
  PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
    "${cargo_bin}" run -p pengu-mesh -- serve --bind 127.0.0.1:0 >"${log_file}" 2>&1 &
  daemon_pid="$!"
}

stop_daemon() {
  if [[ -n "${daemon_pid}" ]] && kill -0 "${daemon_pid}" 2>/dev/null; then
    kill "${daemon_pid}" 2>/dev/null || true
    wait "${daemon_pid}" 2>/dev/null || true
  fi
  daemon_pid=""
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

mark_attached_instance_stale() {
  local instance_id="$1"
  local db_path="${runtime_root}/runtime.sqlite3"
  /usr/bin/python3 - "${db_path}" "${instance_id}" <<'PY'
import sqlite3
import sys

db_path, instance_id = sys.argv[1:3]
conn = sqlite3.connect(db_path)
cur = conn.cursor()
cur.execute(
    """
    UPDATE instances
    SET status = ?, last_error = ?, browser_ws_url = ?, updated_at = updated_at
    WHERE id = ?
    """,
    ('"closed"', "synthetic stale continuity proof", "ws://127.0.0.1:9/devtools/browser/stale", instance_id),
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

attach_first = load("attach-first")
health_first = load("health-first")
attach_second = load("attach-second")
health_second = load("health-second")
doctor_second = load("doctor-second")
attach_third = load("attach-third")
health_third = load("health-third")
doctor_third = load("doctor-third")

assert status("attach-first") == 200, status("attach-first")
assert status("attach-second") == 200, status("attach-second")
assert status("attach-third") == 200, status("attach-third")

first_id = attach_first["data"]["id"]
second_id = attach_second["data"]["id"]
third_id = attach_third["data"]["id"]
assert first_id == second_id == third_id, (first_id, second_id, third_id)

first_attach = health_first["data"]["attach_continuity"]
assert first_attach["outcome"] == "new_instance", first_attach
assert first_attach["freshness"] == "live", first_attach
assert first_attach["endpoint_refreshed"] is False, first_attach

second_attach = health_second["data"]["attach_continuity"]
assert second_attach["outcome"] == "reclaimed_stale_instance", second_attach
assert second_attach["freshness"] == "live", second_attach
assert second_attach["endpoint_refreshed"] is True, second_attach
assert second_attach["last_instance_id"] == first_id, second_attach
assert second_attach["last_browser_ws_url"].endswith("/live-two"), second_attach

second_doctor_attach = doctor_second["data"]["attach_continuity"]
assert second_doctor_attach["outcome"] == "reclaimed_stale_instance", second_doctor_attach
assert second_doctor_attach["freshness"] == "live", second_doctor_attach

third_attach = health_third["data"]["attach_continuity"]
assert third_attach["outcome"] == "reclaimed_stale_instance", third_attach
assert third_attach["freshness"] == "live", third_attach
assert third_attach["endpoint_refreshed"] is True, third_attach
assert third_attach["last_instance_id"] == first_id, third_attach
assert third_attach["last_browser_ws_url"].endswith("/live-three"), third_attach

third_doctor_attach = doctor_third["data"]["attach_continuity"]
assert third_doctor_attach["outcome"] == "reclaimed_stale_instance", third_doctor_attach
assert third_doctor_attach["freshness"] == "live", third_doctor_attach
PY
}

start_debug_server
debug_port="$(wait_for_debug_server)"
ws_one="ws://127.0.0.1:${debug_port}/devtools/browser/live-one"
ws_two="ws://127.0.0.1:${debug_port}/devtools/browser/live-two"
ws_three="ws://127.0.0.1:${debug_port}/devtools/browser/live-three"

start_daemon "${output_dir}/daemon-first.log"
bind_one="$(wait_for_daemon_metadata)"
post_json "${bind_one}" "/instances/attach" "{\"name\":\"attach-smoke\",\"cdp_url\":\"${ws_one}\",\"holder_id\":\"attach-smoke-holder\"}" "${output_dir}/attach-first"
get_json "${bind_one}" "/health" "${output_dir}/health-first"
stop_daemon

start_daemon "${output_dir}/daemon-second.log"
bind_two="$(wait_for_daemon_metadata)"
set_debug_suffix "live-two"
post_json "${bind_two}" "/instances/attach" "{\"name\":\"attach-smoke\",\"cdp_url\":\"${ws_two}\",\"holder_id\":\"attach-smoke-holder\"}" "${output_dir}/attach-second"
get_json "${bind_two}" "/health" "${output_dir}/health-second"
get_json "${bind_two}" "/doctor" "${output_dir}/doctor-second"

attach_instance_id="$(
  /usr/bin/python3 - "${output_dir}/attach-second.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
print(payload["data"]["id"])
PY
)"

mark_attached_instance_stale "${attach_instance_id}"
set_debug_suffix "live-three"
post_json "${bind_two}" "/instances/attach" "{\"name\":\"attach-smoke\",\"cdp_url\":\"${ws_three}\",\"holder_id\":\"attach-smoke-holder\"}" "${output_dir}/attach-third"
get_json "${bind_two}" "/health" "${output_dir}/health-third"
get_json "${bind_two}" "/doctor" "${output_dir}/doctor-third"

assert_payloads

cat > "${output_dir}/summary.md" <<EOF
# Attach Continuity Smoke

- runtime_root: ${runtime_root}
- debug_port: ${debug_port}
- first_bind: ${bind_one}
- second_bind: ${bind_two}
- verified:
  - first external attach creates a new logical attached instance
  - attach identity survives daemon restart
  - browser websocket rotation is classified as stale reclaim with endpoint refresh
  - synthetic stale attached state is reclaimed with explicit stale-instance continuity outcome
  - health and doctor surface attach continuity outcome and freshness consistently
EOF

printf '%s\n' "${output_dir}"
