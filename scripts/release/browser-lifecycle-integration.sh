#!/bin/zsh
set -euo pipefail

# Headless Chrome on macOS often exposes zero Accessibility surfaces because
# there is no window-server-backed UI. In the strictest headless case, the
# attach-side browser-surface list can also fail honestly before any AX tree is
# available. This smoke test still proves the managed launch, external attach,
# browser-surface list, and browser-surface follow-up contracts. When no GUI
# browser surface exists, it requires the commands to fail honestly with
# structured recovery payloads instead of opaque strings.

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$repo_root"

if [[ -x "${HOME}/.cargo/bin/cargo" ]]; then
  cargo_bin="${HOME}/.cargo/bin/cargo"
else
  cargo_bin="$(command -v cargo)"
fi

output_dir="${1:-$(mktemp -d "${TMPDIR:-/tmp}/pengu-mesh-browser-lifecycle.XXXXXX")}"
runtime_root="${output_dir}/runtime-root"
attach_runtime_root="${output_dir}/attach-runtime-root"
mkdir -p "$output_dir" "$runtime_root" "$attach_runtime_root"

instance_id=""
attach_instance_id=""
holder_id="browser-lifecycle-writer"
attach_holder_id="browser-lifecycle-attach-writer"

cleanup() {
  if [[ -n "${instance_id}" ]]; then
    PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
      "${cargo_bin}" run --quiet -p pengu-mesh -- instance-stop --instance-id "${instance_id}" --holder-id "${holder_id}" > "${output_dir}/instance-stop.json" 2> "${output_dir}/instance-stop.stderr.log" || true
  fi
}

trap cleanup EXIT INT TERM

if [[ "$(uname -s)" != "Darwin" ]]; then
  cat > "${output_dir}/summary.md" <<EOF
# Browser Lifecycle Integration

- output_dir: ${output_dir}
- skipped: browser lifecycle integration requires Darwin because browser-surface APIs are macOS Accessibility-backed
EOF
  printf '%s\n' "${output_dir}"
  exit 0
fi

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- instance-start --name browser-lifecycle-headless --channel chrome-dev --headless --holder-id "${holder_id}" > "${output_dir}/instance-start.json"

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

focus_url='data:text/html,<html><body><label>focus<input id="focus-target" autofocus value="ready"></label></body></html>'

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- tab-open --instance-id "${instance_id}" --url "${focus_url}" --holder-id "${holder_id}" > "${output_dir}/tab-open.json"

sleep 1

PENGU_MESH_ALLOW_EXTERNAL_ATTACH=1 \
  PENGU_MESH_RUNTIME_ROOT="${attach_runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- instance-attach --name browser-lifecycle-attached --cdp-url "${browser_ws_url}" --holder-id "${attach_holder_id}" > "${output_dir}/attach-instance.json"

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
  "${cargo_bin}" run --quiet -p pengu-mesh -- browser-surface-list --instance-id "${attach_instance_id}" --holder-id "${attach_holder_id}" > "${output_dir}/surface-list.json"

/usr/bin/python3 - "${output_dir}" "${attach_instance_id}" > "${output_dir}/selection.json" <<'PY'
import json
import os
import sys

output_dir, attach_instance_id = sys.argv[1:3]

with open(os.path.join(output_dir, "instance-start.json"), "r", encoding="utf-8") as handle:
    started = json.load(handle)
with open(os.path.join(output_dir, "attach-instance.json"), "r", encoding="utf-8") as handle:
    attached = json.load(handle)
with open(os.path.join(output_dir, "surface-list.json"), "r", encoding="utf-8") as handle:
    surface_list = json.load(handle)

assert started["ok"] is True, started
assert attached["ok"] is True, attached

if surface_list["ok"] is False:
    data = surface_list["data"]
    assert data["attempted"]["instance_id"] == attach_instance_id, surface_list
    assert isinstance(data.get("operation"), str) and data["operation"], surface_list
    assert isinstance(data.get("reason"), str) and data["reason"], surface_list
    assert isinstance(data.get("recovery"), list) and data["recovery"], surface_list
    assert isinstance(data.get("retry_likely"), bool), surface_list
    json.dump(
        {
            "surface_list_ok": False,
            "surface_count": 0,
            "focus_surface_id": None,
            "root_surface_id": None,
            "app_name": None,
            "reason": data["reason"],
        },
        sys.stdout,
    )
    raise SystemExit(0)

assert surface_list["data"]["instance"]["id"] == attach_instance_id, surface_list
surfaces = surface_list["data"]["surfaces"]
assert isinstance(surfaces, list), surface_list
for item in surfaces:
    assert item["channel"] == surface_list["data"]["instance"]["channel"], item

focus_surface = next((item for item in surfaces if "focus" in item.get("actions", [])), None)
if surfaces and focus_surface is None:
    raise SystemExit("surface list was non-empty but no focusable surface was exposed")
root_surface = next((item for item in surfaces if item.get("role") == "AXWindow"), None)
if root_surface is None and focus_surface is not None:
    root_surface = focus_surface
elif root_surface is None and surfaces:
    root_surface = surfaces[0]

json.dump(
    {
        "surface_list_ok": True,
        "surface_count": len(surfaces),
        "focus_surface_id": None if focus_surface is None else focus_surface["id"],
        "root_surface_id": None if root_surface is None else root_surface["id"],
        "app_name": surface_list["data"]["app_name"],
        "reason": None,
    },
    sys.stdout,
)
PY

surface_list_ok="$(
  /usr/bin/python3 - "${output_dir}/selection.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    selection = json.load(handle)
print("true" if selection["surface_list_ok"] else "false")
PY
)"

surface_count="$(
  /usr/bin/python3 - "${output_dir}/selection.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    selection = json.load(handle)
print(selection["surface_count"])
PY
)"

focus_surface_id="$(
  /usr/bin/python3 - "${output_dir}/selection.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    selection = json.load(handle)
print("" if selection["focus_surface_id"] is None else selection["focus_surface_id"])
PY
)"

root_surface_id="$(
  /usr/bin/python3 - "${output_dir}/selection.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    selection = json.load(handle)
print("" if selection["root_surface_id"] is None else selection["root_surface_id"])
PY
)"

target_surface_id="${focus_surface_id:-ax:headless-missing}"
target_root_surface_id="${root_surface_id:-ax:headless-missing}"

PENGU_MESH_ALLOW_EXTERNAL_ATTACH=1 \
  PENGU_MESH_RUNTIME_ROOT="${attach_runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- browser-surface-list-actions --instance-id "${attach_instance_id}" --surface-id "${target_surface_id}" --holder-id "${attach_holder_id}" > "${output_dir}/action-catalog.json"

PENGU_MESH_ALLOW_EXTERNAL_ATTACH=1 \
  PENGU_MESH_RUNTIME_ROOT="${attach_runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- browser-surface-snapshot --instance-id "${attach_instance_id}" --root-surface-id "${target_root_surface_id}" --holder-id "${attach_holder_id}" > "${output_dir}/surface-snapshot.json"

PENGU_MESH_ALLOW_EXTERNAL_ATTACH=1 \
  PENGU_MESH_RUNTIME_ROOT="${attach_runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- browser-surface-action --instance-id "${attach_instance_id}" --action focus --surface-id "${target_surface_id}" --holder-id "${attach_holder_id}" > "${output_dir}/action-focus.json"

/usr/bin/python3 - "${output_dir}" "${attach_instance_id}" "${surface_list_ok}" "${surface_count}" "${target_surface_id}" "${target_root_surface_id}" > "${output_dir}/summary.md" <<'PY'
import json
import os
import sys

output_dir, attach_instance_id, surface_list_ok_raw, surface_count_raw, target_surface_id, target_root_surface_id = sys.argv[1:7]
surface_list_ok = surface_list_ok_raw == "true"
surface_count = int(surface_count_raw)


def load(name):
    with open(os.path.join(output_dir, f"{name}.json"), "r", encoding="utf-8") as handle:
        return json.load(handle)


def assert_structured_failure(payload, expected_instance_id, expected_surface_id=None, expected_root_id=None, expected_action=None):
    assert payload["ok"] is False, payload
    data = payload["data"]
    assert isinstance(data.get("operation"), str) and data["operation"], payload
    assert data["attempted"]["instance_id"] == expected_instance_id, payload
    if expected_surface_id is not None:
        assert data["attempted"].get("surface_id") == expected_surface_id, payload
    if expected_root_id is not None:
        assert data["attempted"].get("root_surface_id") == expected_root_id, payload
    if expected_action is not None:
        assert data["attempted"].get("action") == expected_action, payload
    assert isinstance(data.get("reason"), str) and data["reason"], payload
    assert isinstance(data.get("recovery"), list) and data["recovery"], payload
    assert isinstance(data.get("retry_likely"), bool), payload


surface_list = load("surface-list")
action_catalog = load("action-catalog")
snapshot = load("surface-snapshot")
focus_action = load("action-focus")

if not surface_list_ok:
    assert_structured_failure(surface_list, attach_instance_id)
    assert_structured_failure(
        action_catalog,
        attach_instance_id,
        expected_surface_id=target_surface_id,
    )
    assert_structured_failure(
        snapshot,
        attach_instance_id,
        expected_root_id=target_root_surface_id,
    )
    assert_structured_failure(
        focus_action,
        attach_instance_id,
        expected_surface_id=target_surface_id,
        expected_action="focus",
    )
    summary = [
        "# Browser Lifecycle Integration",
        "",
        f"- output_dir: {output_dir}",
        f"- instance_id: {attach_instance_id}",
        "- result: headless attach had no running GUI browser app; surface list and follow-up commands returned structured recovery payloads",
    ]
elif surface_count == 0:
    assert surface_list["ok"] is True, surface_list
    assert isinstance(surface_list["data"]["surfaces"], list), surface_list
    assert_structured_failure(
        action_catalog,
        attach_instance_id,
        expected_surface_id=target_surface_id,
    )
    assert_structured_failure(
        snapshot,
        attach_instance_id,
        expected_root_id=target_root_surface_id,
    )
    assert_structured_failure(
        focus_action,
        attach_instance_id,
        expected_surface_id=target_surface_id,
        expected_action="focus",
    )
    summary = [
        "# Browser Lifecycle Integration",
        "",
        f"- output_dir: {output_dir}",
        f"- instance_id: {attach_instance_id}",
        "- result: headless surface list succeeded with zero surfaces; follow-up commands returned structured recovery payloads",
    ]
else:
    assert surface_list["ok"] is True, surface_list
    assert isinstance(surface_list["data"]["surfaces"], list), surface_list
    assert action_catalog["ok"] is True, action_catalog
    assert action_catalog["data"]["surface"]["id"] == target_surface_id, action_catalog
    assert isinstance(action_catalog["data"]["actions"], list), action_catalog

    snapshot_ok = snapshot["ok"] is True
    action_ok = focus_action["ok"] is True

    if snapshot_ok:
        snapshot_data = snapshot["data"]
        snapshot_artifact = snapshot_data["snapshot_artifact"]
        assert snapshot_artifact["path"], snapshot_data
        assert os.path.exists(snapshot_artifact["path"]), snapshot_data
    else:
        assert_structured_failure(
            snapshot,
            attach_instance_id,
            expected_root_id=target_root_surface_id,
        )

    if action_ok:
        focus_data = focus_action["data"]
        assert focus_data["resolved_channel"], focus_data
        assert focus_data["interference_level"], focus_data
        assert isinstance(focus_data["fallback_count"], int), focus_data
        assert isinstance(focus_data["detail"], str) and focus_data["detail"], focus_data
    else:
        assert_structured_failure(
            focus_action,
            attach_instance_id,
            expected_surface_id=target_surface_id,
            expected_action="focus",
        )

    result = "browser-surface lifecycle completed with snapshot and focus evidence"
    if not snapshot_ok or not action_ok:
        result = "browser-surface lifecycle reached structured headless limitation after surface discovery"
    summary = [
        "# Browser Lifecycle Integration",
        "",
        f"- output_dir: {output_dir}",
        f"- instance_id: {attach_instance_id}",
        f"- surface_count: {surface_count}",
        f"- result: {result}",
    ]

print("\n".join(summary))
PY

printf '%s\n' "${output_dir}"
