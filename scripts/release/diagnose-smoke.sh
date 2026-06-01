#!/bin/zsh
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$repo_root"

if [[ -x "${HOME}/.cargo/bin/cargo" ]]; then
  cargo_bin="${HOME}/.cargo/bin/cargo"
else
  cargo_bin="$(command -v cargo)"
fi

output_dir="${1:-$(mktemp -d "${TMPDIR:-/tmp}/pengu-mesh-diagnose-smoke.XXXXXX")}"
runtime_root="${output_dir}/runtime-root"
mkdir -p "$output_dir" "$runtime_root"

daemon_pid=""
daemon_log="${output_dir}/daemon-start.json"
daemon_stderr="${output_dir}/daemon-start.stderr.log"
daemon_bind=""

cleanup() {
  if [[ -n "${daemon_pid}" ]]; then
    kill "${daemon_pid}" >/dev/null 2>&1 || true
    wait "${daemon_pid}" >/dev/null 2>&1 || true
  fi
}

trap cleanup EXIT INT TERM

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- diagnose > "${output_dir}/diagnose-cli.json"

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh-mcp -- --once-tool diagnose > "${output_dir}/diagnose-mcp.json"

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- serve --bind 127.0.0.1:0 > "${daemon_log}" 2> "${daemon_stderr}" &
daemon_pid=$!

for _ in {1..60}; do
  if [[ -s "${daemon_log}" ]]; then
    daemon_bind="$(
      /usr/bin/python3 - "${daemon_log}" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
print(payload["data"]["bind_addr"])
PY
    )"
    if [[ -n "${daemon_bind}" ]]; then
      break
    fi
  fi
  sleep 0.2
done

if [[ -z "${daemon_bind}" ]]; then
  echo "failed to discover daemon bind address from ${daemon_log}" >&2
  exit 1
fi

/usr/bin/python3 - "${daemon_bind}" "${output_dir}/diagnose-http.json" "${output_dir}/tool-catalog-http.json" <<'PY'
import sys
import urllib.request

bind_addr, diagnose_path, tools_path = sys.argv[1:4]
with urllib.request.urlopen(f"http://{bind_addr}/diagnose", timeout=15) as response:
    diagnose_body = response.read().decode("utf-8")
with open(diagnose_path, "w", encoding="utf-8") as handle:
    handle.write(diagnose_body)
with urllib.request.urlopen(f"http://{bind_addr}/tools", timeout=15) as response:
    tools_body = response.read().decode("utf-8")
with open(tools_path, "w", encoding="utf-8") as handle:
    handle.write(tools_body)
PY

/usr/bin/python3 - "${output_dir}" <<'PY'
import json
import os
import sys

output_dir = sys.argv[1]


def load(name):
    with open(os.path.join(output_dir, name), "r", encoding="utf-8") as handle:
        return json.load(handle)


cli = load("diagnose-cli.json")
mcp = load("diagnose-mcp.json")
http = load("diagnose-http.json")
tools = load("tool-catalog-http.json")

for payload in (cli, mcp, http):
    assert payload["ok"] is True, payload
    assert payload["code"] == "ok", payload
    assert payload["data"]["schema_version"] == "diagnose.v1", payload
    assert isinstance(payload["data"]["permissions"], list), payload
    assert isinstance(payload["data"]["browser_channels"], list), payload
    assert isinstance(payload["data"]["services"], list), payload
    assert isinstance(payload["data"]["capabilities"], list), payload
    assert isinstance(payload["data"]["remediations"], list), payload

assert cli["data"]["schema_version"] == mcp["data"]["schema_version"] == http["data"]["schema_version"]
tool_names = [tool["name"] for tool in tools["data"]["tools"]]
assert "diagnose" in tool_names, tools
assert "browser_surface_list_actions" in tool_names, tools

summary = {
    "schema_version": cli["data"]["schema_version"],
    "cli_state": cli["data"]["state"],
    "mcp_state": mcp["data"]["state"],
    "http_state": http["data"]["state"],
    "permission_count": len(cli["data"]["permissions"]),
    "capability_count": len(cli["data"]["capabilities"]),
    "remediation_count": len(cli["data"]["remediations"]),
    "tool_count": len(tool_names),
}

with open(os.path.join(output_dir, "verification.json"), "w", encoding="utf-8") as handle:
    json.dump(summary, handle, indent=2, sort_keys=True)
PY

cat > "${output_dir}/summary.md" <<EOF
# Diagnose Smoke

- output_dir: ${output_dir}
- runtime_root: ${runtime_root}
- daemon_bind: ${daemon_bind}
- verified:
  - diagnose is reachable through CLI
  - diagnose is reachable through MCP
  - diagnose is reachable through HTTP
  - the HTTP tool catalog advertises diagnose and browser_surface_list_actions
  - all diagnose surfaces preserve the same schema_version and section layout
EOF

printf '%s\n' "${output_dir}"
