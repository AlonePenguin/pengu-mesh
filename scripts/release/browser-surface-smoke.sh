#!/bin/zsh
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$repo_root"

if [[ -x "${HOME}/.cargo/bin/cargo" ]]; then
  cargo_bin="${HOME}/.cargo/bin/cargo"
else
  cargo_bin="$(command -v cargo)"
fi

output_dir="${1:-$(mktemp -d "${TMPDIR:-/tmp}/pengu-mesh-browser-surface-smoke.XXXXXX")}"
runtime_root="${output_dir}/runtime-root"
attach_runtime_root="${output_dir}/attach-runtime-root"
mkdir -p "$output_dir" "$runtime_root" "$attach_runtime_root"

instance_id=""
holder_id="browser-surface-smoke-writer"
attach_instance_id=""
attach_holder_id="browser-surface-attach-writer"
daemon_pid=""
daemon_log="${output_dir}/daemon-start.json"
daemon_stderr="${output_dir}/daemon-start.stderr.log"
daemon_bind=""

cleanup() {
  if [[ -n "${daemon_pid}" ]]; then
    kill "${daemon_pid}" >/dev/null 2>&1 || true
    wait "${daemon_pid}" >/dev/null 2>&1 || true
  fi
  if [[ -n "${instance_id}" ]]; then
    PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
      "${cargo_bin}" run --quiet -p pengu-mesh -- instance-stop --instance-id "${instance_id}" --holder-id "${holder_id}" > "${output_dir}/instance-stop.json" 2> "${output_dir}/instance-stop.stderr.log" || true
  fi
}

trap cleanup EXIT INT TERM

if [[ "$(uname -s)" != "Darwin" ]]; then
  cat > "${output_dir}/summary.md" <<EOF
# Browser Surface Smoke

- output_dir: ${output_dir}
- skipped: browser surface smoke requires Darwin because it exercises the macOS Accessibility substrate
EOF
  printf '%s\n' "${output_dir}"
  exit 0
fi

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- instance-start --name browser-surface-smoke --channel chrome-dev --holder-id "${holder_id}" > "${output_dir}/instance-start.json"

instance_id="$(
  /usr/bin/python3 - "${output_dir}/instance-start.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
print(payload["data"]["id"])
PY
)"

browser_ws_url="$(
  /usr/bin/python3 - "${output_dir}/instance-start.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
print(payload["data"]["browser_ws_url"])
PY
)"

focus_url='data:text/html,<html><body><label>smoke<input id="focus-target" autofocus value="smoke"></label></body></html>'

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- tab-open --instance-id "${instance_id}" --url "${focus_url}" --holder-id "${holder_id}" > "${output_dir}/tab-open.json"

sleep 1

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- browser-surface-list --instance-id "${instance_id}" --holder-id "${holder_id}" > "${output_dir}/surface-list.json"

PENGU_MESH_ALLOW_EXTERNAL_ATTACH=1 \
  PENGU_MESH_RUNTIME_ROOT="${attach_runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- instance-attach --name browser-surface-attached --cdp-url "${browser_ws_url}" --holder-id "${attach_holder_id}" > "${output_dir}/attach-instance.json"

attach_instance_id="$(
  /usr/bin/python3 - "${output_dir}/attach-instance.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
print(payload["data"]["id"])
PY
)"

PENGU_MESH_ALLOW_EXTERNAL_ATTACH=1 \
  PENGU_MESH_RUNTIME_ROOT="${attach_runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- browser-surface-list --instance-id "${attach_instance_id}" --holder-id "${attach_holder_id}" > "${output_dir}/attach-surface-list.json"

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

# Refresh the live AX tree after attach and daemon startup so later snapshot
# and action checks do not rely on a stale surface identifier.
PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- browser-surface-list --instance-id "${instance_id}" --holder-id "${holder_id}" > "${output_dir}/surface-list.json"

window_surface_id="$(
  /usr/bin/python3 - "${output_dir}/surface-list.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)

surfaces = payload["data"]["surfaces"]
window = next((item["id"] for item in surfaces if item["role"] == "AXWindow"), surfaces[0]["id"])
print(window)
PY
)"

direct_action_payload="$(
  /usr/bin/python3 - "${output_dir}/surface-list.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)

surfaces = payload["data"]["surfaces"]
for item in surfaces:
    assert item["channel"] == payload["data"]["instance"]["channel"], item
address_bar = next(
    (
        item
        for item in surfaces
        if item["role"] == "AXTextField"
        and "set_value" in item["actions"]
    ),
    None,
)
if address_bar is not None:
    print(json.dumps({
        "surface_id": address_bar["id"],
        "action": "set_value",
        "value": address_bar.get("value") or "about:blank",
    }))
    raise SystemExit(0)

pressable = next(
    (item for item in surfaces if "press" in item["actions"]),
    None,
)
if pressable is None:
    raise SystemExit("no direct-action browser surface found")
print(json.dumps({
    "surface_id": pressable["id"],
    "action": "press",
    "value": None,
}))
PY
)"

direct_surface_id="$(
  /usr/bin/python3 - <<'PY' "${direct_action_payload}"
import json
import sys
print(json.loads(sys.argv[1])["surface_id"])
PY
)"

direct_action_kind="$(
  /usr/bin/python3 - <<'PY' "${direct_action_payload}"
import json
import sys
print(json.loads(sys.argv[1])["action"])
PY
)"

direct_action_value="$(
  /usr/bin/python3 - <<'PY' "${direct_action_payload}"
import json
import sys
value = json.loads(sys.argv[1])["value"]
print("" if value is None else value)
PY
)"

direct_action_args=(
  --action "${direct_action_kind}"
  --surface-id "${direct_surface_id}"
)
if [[ -n "${direct_action_value}" ]]; then
  direct_action_args+=(--value "${direct_action_value}")
fi

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- browser-surface-list-actions --instance-id "${instance_id}" --surface-id "${direct_surface_id}" --holder-id "${holder_id}" > "${output_dir}/action-catalog-direct.json"

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh-mcp -- --once-tool browser_surface_list_actions --once-input "{\"instance_id\":\"${instance_id}\",\"surface_id\":\"${direct_surface_id}\",\"holder_id\":\"${holder_id}\"}" > "${output_dir}/action-catalog-direct-mcp.json"

/usr/bin/python3 - "${daemon_bind}" "${instance_id}" "${direct_surface_id}" "${output_dir}/action-catalog-direct-http.json" <<'PY'
import json
import sys
import urllib.parse
import urllib.request

bind_addr, instance_id, surface_id, output_path = sys.argv[1:5]
query = urllib.parse.urlencode({"instance_id": instance_id, "surface_id": surface_id})
url = f"http://{bind_addr}/browser/surfaces/actions?{query}"
with urllib.request.urlopen(url, timeout=15) as response:
    body = response.read().decode("utf-8")
with open(output_path, "w", encoding="utf-8") as handle:
    handle.write(body)
PY

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- browser-surface-list-actions --instance-id "${instance_id}" --surface-id "ax:0" --holder-id "${holder_id}" > "${output_dir}/action-catalog-root.json"

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- browser-surface-list-actions --instance-id "${instance_id}" --surface-id "ax:missing" --holder-id "${holder_id}" > "${output_dir}/action-catalog-missing.json"

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- browser-surface-snapshot --instance-id "${instance_id}" --root-surface-id "${window_surface_id}" --holder-id "${holder_id}" > "${output_dir}/surface-snapshot.json"

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- browser-surface-action --instance-id "${instance_id}" "${direct_action_args[@]}" --holder-id "${holder_id}" > "${output_dir}/action-direct.json"

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- browser-surface-action --instance-id "${instance_id}" --action confirm --surface-id "ax:0" --holder-id "${holder_id}" > "${output_dir}/action-fallback.json"

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- browser-surface-action --instance-id "${instance_id}" --action key_sequence --surface-id "ax:0" --execution-channel global_takeover --allow-takeover --holder-id "${holder_id}" > "${output_dir}/action-global-takeover.json"

/usr/bin/python3 - "${output_dir}" "${window_surface_id}" "${direct_surface_id}" "${direct_action_kind}" <<'PY'
import json
import os
import sys

output_dir, window_surface_id, direct_surface_id, direct_action_kind = sys.argv[1:5]


def load(name):
    with open(os.path.join(output_dir, f"{name}.json"), "r", encoding="utf-8") as handle:
        return json.load(handle)


start = load("instance-start")
surface_list = load("surface-list")
snapshot = load("surface-snapshot")
direct_action = load("action-direct")
fallback = load("action-fallback")
global_takeover = load("action-global-takeover")
direct_catalog = load("action-catalog-direct")
direct_catalog_mcp = load("action-catalog-direct-mcp")
direct_catalog_http = load("action-catalog-direct-http")
root_catalog = load("action-catalog-root")
missing_catalog = load("action-catalog-missing")
attached = load("attach-instance")
attached_surfaces = load("attach-surface-list")

assert start["ok"] is True, start
assert attached["ok"] is True, attached
assert surface_list["ok"] is True, surface_list
assert snapshot["ok"] is True, snapshot
assert direct_action["ok"] is True, direct_action
assert fallback["ok"] is True, fallback
assert global_takeover["ok"] is True, global_takeover
assert direct_catalog["ok"] is True, direct_catalog
assert direct_catalog_mcp["ok"] is True, direct_catalog_mcp
assert direct_catalog_http["ok"] is True, direct_catalog_http
assert root_catalog["ok"] is True, root_catalog
assert missing_catalog["ok"] is False, missing_catalog

surfaces = surface_list["data"]["surfaces"]
assert len(surfaces) > 0, surfaces

if attached_surfaces["ok"] is True:
    attached_surface_data = attached_surfaces["data"]["surfaces"]
    assert attached_surfaces["data"]["instance"]["id"] == attached["data"]["id"], attached_surfaces
    assert len(attached_surface_data) > 0, attached_surface_data
    attached_surface_mode = "listed"
else:
    attached_failure = attached_surfaces["data"]
    assert attached_failure["attempted"]["instance_id"] == attached["data"]["id"], attached_surfaces
    assert isinstance(attached_failure.get("operation"), str) and attached_failure["operation"], attached_surfaces
    assert isinstance(attached_failure.get("reason"), str) and attached_failure["reason"], attached_surfaces
    assert isinstance(attached_failure.get("recovery"), list) and attached_failure["recovery"], attached_surfaces
    assert isinstance(attached_failure.get("retry_likely"), bool), attached_surfaces
    attached_surface_data = []
    attached_surface_mode = "structured_not_ready"

snapshot_data = snapshot["data"]
assert snapshot_data["snapshot_artifact"]["path"], snapshot_data
assert len(snapshot_data["surfaces"]) > 0, snapshot_data
assert snapshot_data["capture_artifact"], snapshot_data

direct_action_data = direct_action["data"]
direct_catalog_data = direct_catalog["data"]
assert direct_catalog_mcp["data"]["surface"]["id"] == direct_surface_id, direct_catalog_mcp
assert direct_catalog_http["data"]["surface"]["id"] == direct_surface_id, direct_catalog_http
assert [item["action"] for item in direct_catalog_mcp["data"]["actions"]] == [
    item["action"] for item in direct_catalog_data["actions"]
], (direct_catalog_mcp, direct_catalog_data)
assert [item["action"] for item in direct_catalog_http["data"]["actions"]] == [
    item["action"] for item in direct_catalog_data["actions"]
], (direct_catalog_http, direct_catalog_data)
direct_contract = next(
    item for item in direct_catalog_data["actions"]
    if item["action"] == direct_action_kind
)
assert direct_action_data["resolved_channel"] in {"ax_direct", "apple_events_activation"}, direct_action_data
assert direct_action_data["requested"]["action"] == direct_action_kind, direct_action_data
assert direct_action_data["target_surface_id"] == direct_surface_id, direct_action_data
assert direct_catalog_data["surface"]["id"] == direct_surface_id, direct_catalog_data
assert any(
    path["execution_channel"] == direct_action_data["resolved_channel"]
    and path["interference_level"] == direct_action_data["interference_level"]
    for path in direct_contract["execution_paths"]
), (direct_contract, direct_action_data)

fallback_data = fallback["data"]
root_catalog_data = root_catalog["data"]
confirm_contract = next(
    item for item in root_catalog_data["actions"]
    if item["action"] == "confirm"
)
assert fallback_data["requested"]["action"] == "confirm", fallback_data
assert fallback_data["fallback_count"] >= 1, fallback_data
assert fallback_data["resolved_channel"] in {"app_scoped_key_post", "global_takeover"}, fallback_data
assert any(
    path["execution_channel"] == fallback_data["resolved_channel"]
    and path["interference_level"] == fallback_data["interference_level"]
    for path in confirm_contract["execution_paths"]
), (confirm_contract, fallback_data)

takeover_data = global_takeover["data"]
takeover_contract = next(
    item for item in root_catalog_data["actions"]
    if item["action"] == "key_sequence"
)
assert takeover_data["resolved_channel"] == "global_takeover", takeover_data
assert takeover_data["interference_level"] == "global_takeover", takeover_data
assert takeover_data["took_focus"] is True, takeover_data
assert any(
    path["execution_channel"] == takeover_data["resolved_channel"]
    and path["interference_level"] == takeover_data["interference_level"]
    for path in takeover_contract["execution_paths"]
), (takeover_contract, takeover_data)

missing_payload = missing_catalog["data"]
assert missing_payload["operation"] == "browser surface action catalog", missing_payload
assert missing_payload["attempted"]["instance_id"] == start["data"]["id"], missing_payload
assert missing_payload["attempted"]["surface_id"] == "ax:missing", missing_payload
assert isinstance(missing_payload["reason"], str) and missing_payload["reason"], missing_payload
assert isinstance(missing_payload["recovery"], list) and missing_payload["recovery"], missing_payload
assert missing_payload["retry_likely"] is False, missing_payload

summary = {
    "instance_id": start["data"]["id"],
    "attached_instance_id": attached["data"]["id"],
    "surface_count": len(surfaces),
    "attached_surface_count": len(attached_surface_data),
    "attached_surface_mode": attached_surface_mode,
    "window_surface_id": window_surface_id,
    "direct_surface_id": direct_surface_id,
    "snapshot_artifact_path": snapshot_data["snapshot_artifact"]["path"],
    "capture_artifact_path": snapshot_data["capture_artifact"]["path"],
    "direct_action_kind": direct_action_data["requested"]["action"],
    "direct_action_channel": direct_action_data["resolved_channel"],
    "direct_action_catalog_channels": [
        path["execution_channel"] for path in direct_contract["execution_paths"]
    ],
    "fallback_channel": fallback_data["resolved_channel"],
    "fallback_count": fallback_data["fallback_count"],
    "takeover_channel": takeover_data["resolved_channel"],
    "missing_catalog_recovery": missing_payload["recovery"],
}

with open(os.path.join(output_dir, "verification.json"), "w", encoding="utf-8") as handle:
    json.dump(summary, handle, indent=2, sort_keys=True)
PY

cat > "${output_dir}/summary.md" <<EOF
# Browser Surface Smoke

- output_dir: ${output_dir}
- runtime_root: ${runtime_root}
- attach_runtime_root: ${attach_runtime_root}
- instance_id: ${instance_id}
- attach_instance_id: ${attach_instance_id}
- daemon_bind: ${daemon_bind}
- window_surface_id: ${window_surface_id}
- direct_surface_id: ${direct_surface_id}
- direct_action_kind: ${direct_action_kind}
- verified:
  - browser surface listing discovers native Chrome Dev surfaces through AX
  - browser surface list-actions is reachable over CLI, MCP, and HTTP
  - attached-browser surface listing either works through the fallback PID lookup path or returns structured readiness guidance when no GUI browser app exists
  - browser surface snapshot emits a native snapshot artifact plus capture artifact
  - direct, fallback, and takeover actions resolve to execution channels declared by the catalog
  - a fallback path reports non-zero fallback telemetry
  - invalid browser-surface action catalog lookups return structured recovery guidance
EOF

printf '%s\n' "${output_dir}"
