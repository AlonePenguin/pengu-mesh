#!/bin/zsh
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$repo_root"

if [[ -x "${HOME}/.cargo/bin/cargo" ]]; then
  cargo_bin="${HOME}/.cargo/bin/cargo"
else
  cargo_bin="$(command -v cargo)"
fi

output_dir="${1:-$(mktemp -d "${TMPDIR:-/tmp}/pengu-mesh-tab-lifecycle.XXXXXX")}"
runtime_root="${output_dir}/runtime-root"
mkdir -p "$output_dir" "$runtime_root"

instance_id=""
holder_id="tab-lifecycle-writer"
open_url='data:text/html,<html><head><title>Before</title></head><body>BeforeState</body></html>'
navigate_url='data:text/html,<html><head><title>After</title></head><body>AfterState</body></html>'

cleanup() {
  if [[ -n "${instance_id}" ]]; then
    PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
      "${cargo_bin}" run --quiet -p pengu-mesh -- instance-stop --instance-id "${instance_id}" --holder-id "${holder_id}" > "${output_dir}/instance-stop.json" 2> "${output_dir}/instance-stop.stderr.log" || true
  fi
}

trap cleanup EXIT INT TERM

if [[ "$(uname -s)" != "Darwin" ]]; then
  cat > "${output_dir}/summary.md" <<EOF
# Tab Lifecycle Integration

- output_dir: ${output_dir}
- skipped: tab lifecycle integration currently runs only on Darwin in the local gate baseline
EOF
  printf '%s\n' "${output_dir}"
  exit 0
fi

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- instance-start --name tab-lifecycle-headless --channel chrome-dev --headless --holder-id "${holder_id}" > "${output_dir}/instance-start.json"

instance_id="$(
  /usr/bin/python3 - "${output_dir}/instance-start.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
assert payload["ok"] is True, payload
print(payload["data"]["id"])
PY
)"

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- tab-open --instance-id "${instance_id}" --url "${open_url}" --holder-id "${holder_id}" > "${output_dir}/tab-open.json"

tab_id="$(
  /usr/bin/python3 - "${output_dir}/tab-open.json" "${instance_id}" <<'PY'
import json
import sys

path, instance_id = sys.argv[1:3]
with open(path, "r", encoding="utf-8") as handle:
    payload = json.load(handle)
if payload["ok"] is not True:
    data = payload["data"]
    assert data["operation"] == "tab opened", payload
    assert data["attempted"]["instance_id"] == instance_id, payload
    assert isinstance(data["recovery"], list) and data["recovery"], payload
    assert isinstance(data["retry_likely"], bool), payload
    raise SystemExit("tab_open returned structured failure")
print(payload["data"]["id"])
PY
)"

sleep 1

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- tab-list --instance-id "${instance_id}" --holder-id "${holder_id}" > "${output_dir}/tab-list-before.json"

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- tab-list-actions --instance-id "${instance_id}" --tab-id "${tab_id}" --holder-id "${holder_id}" > "${output_dir}/tab-list-actions.json"

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- tab-action --tab-id "${tab_id}" --kind navigate --url "${navigate_url}" --holder-id "${holder_id}" > "${output_dir}/tab-action-navigate.json"

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- tab-action --tab-id "${tab_id}" --kind evaluate --expression "document.title" --holder-id "${holder_id}" > "${output_dir}/tab-action-evaluate.json"

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- tab-snapshot --tab-id "${tab_id}" --holder-id "${holder_id}" > "${output_dir}/tab-snapshot.json"

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- tab-screenshot --tab-id "${tab_id}" --holder-id "${holder_id}" > "${output_dir}/tab-screenshot.json"

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- tab-text --tab-id "${tab_id}" --holder-id "${holder_id}" > "${output_dir}/tab-text.json"

snapshot_artifact_id="$(
  /usr/bin/python3 - "${output_dir}/tab-snapshot.json" "${instance_id}" "${tab_id}" <<'PY'
import json
import sys

path, instance_id, tab_id = sys.argv[1:4]
with open(path, "r", encoding="utf-8") as handle:
    payload = json.load(handle)
if payload["ok"] is not True:
    data = payload["data"]
    assert data["operation"] == "tab snapshot", payload
    assert data["attempted"]["instance_id"] == instance_id, payload
    assert data["attempted"]["tab_id"] == tab_id, payload
    assert isinstance(data["reason"], str) and data["reason"], payload
    assert isinstance(data["recovery"], list) and data["recovery"], payload
    assert isinstance(data["retry_likely"], bool), payload
    raise SystemExit("tab_snapshot returned structured failure")
print(payload["data"]["artifact"]["id"])
PY
)"

snapshot_run_id="$(
  /usr/bin/python3 - "${output_dir}/tab-snapshot.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
assert payload["ok"] is True, payload
print(payload["data"]["artifact"]["run_id"])
PY
)"

screenshot_artifact_id="$(
  /usr/bin/python3 - "${output_dir}/tab-screenshot.json" "${instance_id}" "${tab_id}" <<'PY'
import json
import sys

path, instance_id, tab_id = sys.argv[1:4]
with open(path, "r", encoding="utf-8") as handle:
    payload = json.load(handle)
if payload["ok"] is not True:
    data = payload["data"]
    assert data["operation"] == "tab screenshot", payload
    assert data["attempted"]["instance_id"] == instance_id, payload
    assert data["attempted"]["tab_id"] == tab_id, payload
    assert isinstance(data["reason"], str) and data["reason"], payload
    assert isinstance(data["recovery"], list) and data["recovery"], payload
    assert isinstance(data["retry_likely"], bool), payload
    raise SystemExit("tab_screenshot returned structured failure")
print(payload["data"]["artifact"]["id"])
PY
)"

screenshot_run_id="$(
  /usr/bin/python3 - "${output_dir}/tab-screenshot.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
assert payload["ok"] is True, payload
print(payload["data"]["artifact"]["run_id"])
PY
)"

/usr/bin/python3 - "${output_dir}/tab-screenshot.json" > "${output_dir}/tab-screenshot-sanity.json" <<'PY'
import json
import os
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)

assert payload["ok"] is True, payload
artifact = payload["data"]["artifact"]
path = artifact["path"]
mime_type = artifact["mime_type"]
assert os.path.exists(path), artifact
size_bytes = os.path.getsize(path)
assert size_bytes > 1024, size_bytes
png_header_valid = None
if mime_type == "image/png":
    with open(path, "rb") as handle:
        png_header_valid = handle.read(8) == b"\x89PNG\r\n\x1a\n"
    assert png_header_valid is True, path

print(
    json.dumps(
        {
            "path": path,
            "mime_type": mime_type,
            "size_bytes": size_bytes,
            "png_header_valid": png_header_valid,
        },
        indent=2,
    )
)
PY

text_artifact_id="$(
  /usr/bin/python3 - "${output_dir}/tab-text.json" "${instance_id}" "${tab_id}" <<'PY'
import json
import sys

path, instance_id, tab_id = sys.argv[1:4]
with open(path, "r", encoding="utf-8") as handle:
    payload = json.load(handle)
if payload["ok"] is not True:
    data = payload["data"]
    assert data["operation"] == "tab text", payload
    assert data["attempted"]["instance_id"] == instance_id, payload
    assert data["attempted"]["tab_id"] == tab_id, payload
    assert isinstance(data["reason"], str) and data["reason"], payload
    assert isinstance(data["recovery"], list) and data["recovery"], payload
    assert isinstance(data["retry_likely"], bool), payload
    raise SystemExit("tab_text returned structured failure")
print(payload["data"]["artifact"]["id"])
PY
)"

text_run_id="$(
  /usr/bin/python3 - "${output_dir}/tab-text.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
assert payload["ok"] is True, payload
print(payload["data"]["artifact"]["run_id"])
PY
)"

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- artifact-list --instance-id "${instance_id}" > "${output_dir}/artifact-list.json"

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- artifact-list --run-id "${snapshot_run_id}" > "${output_dir}/artifact-list-run-snapshot.json"

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- artifact-list --run-id "${screenshot_run_id}" > "${output_dir}/artifact-list-run-screenshot.json"

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- artifact-list --run-id "${text_run_id}" > "${output_dir}/artifact-list-run-text.json"

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- artifact-verify --artifact-id "${snapshot_artifact_id}" > "${output_dir}/artifact-verify-snapshot.json"

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- artifact-verify --artifact-id "${screenshot_artifact_id}" > "${output_dir}/artifact-verify-screenshot.json"

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- artifact-verify --artifact-id "${text_artifact_id}" > "${output_dir}/artifact-verify-text.json"

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- tab-close --tab-id "${tab_id}" --holder-id "${holder_id}" > "${output_dir}/tab-close.json"

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- tab-list --instance-id "${instance_id}" --holder-id "${holder_id}" > "${output_dir}/tab-list-after.json"

/usr/bin/python3 - "${output_dir}" "${instance_id}" "${tab_id}" "${navigate_url}" > "${output_dir}/summary.md" <<'PY'
import json
import os
import sys

output_dir, instance_id, tab_id, navigate_url = sys.argv[1:5]


def load(name):
    with open(os.path.join(output_dir, f"{name}.json"), "r", encoding="utf-8") as handle:
        return json.load(handle)


def assert_structured_tab_failure(payload, operation, action_kind=None):
    assert payload["ok"] is False, payload
    data = payload["data"]
    assert data["operation"] == operation, payload
    assert data["attempted"]["instance_id"] == instance_id, payload
    assert data["attempted"]["tab_id"] == tab_id, payload
    if action_kind is not None:
        assert data["attempted"]["action_kind"] == action_kind, payload
    assert isinstance(data["reason"], str) and data["reason"], payload
    assert isinstance(data["recovery"], list) and data["recovery"], payload
    assert isinstance(data["retry_likely"], bool), payload


def assert_structured_artifact_failure(payload, operation, action_kind):
    assert payload["ok"] is False, payload
    data = payload["data"]
    assert data["operation"] == operation, payload
    assert data["attempted"]["action_kind"] == action_kind, payload
    assert isinstance(data["reason"], str) and data["reason"], payload
    assert isinstance(data["recovery"], list) and data["recovery"], payload
    assert isinstance(data["retry_likely"], bool), payload


tab_open = load("tab-open")
tab_list_before = load("tab-list-before")
tab_list_actions = load("tab-list-actions")
tab_action_navigate = load("tab-action-navigate")
tab_action_evaluate = load("tab-action-evaluate")
tab_snapshot = load("tab-snapshot")
tab_screenshot = load("tab-screenshot")
tab_text = load("tab-text")
artifact_list = load("artifact-list")
artifact_list_run_snapshot = load("artifact-list-run-snapshot")
artifact_list_run_screenshot = load("artifact-list-run-screenshot")
artifact_list_run_text = load("artifact-list-run-text")
artifact_verify_snapshot = load("artifact-verify-snapshot")
artifact_verify_screenshot = load("artifact-verify-screenshot")
artifact_verify_text = load("artifact-verify-text")
tab_screenshot_sanity = load("tab-screenshot-sanity")
tab_close = load("tab-close")
tab_list_after = load("tab-list-after")

assert tab_open["ok"] is True, tab_open

for payload, operation, action_kind in [
    (tab_list_before, "tab list", "list"),
    (tab_list_actions, "tab action catalog", "list_actions"),
    (tab_action_navigate, "tab action completed", "navigate"),
    (tab_action_evaluate, "tab action completed", "evaluate"),
    (tab_snapshot, "tab snapshot", None),
    (tab_screenshot, "tab screenshot", None),
    (tab_text, "tab text", None),
    (tab_close, "tab closed", "close"),
    (tab_list_after, "tab list", "list"),
]:
    if payload["ok"] is not True:
        assert_structured_tab_failure(payload, operation, action_kind)
        raise SystemExit(f"{operation} returned structured failure")

for payload, operation, action_kind in [(artifact_list, "artifact list", "list")]:
    if payload["ok"] is not True:
        assert_structured_artifact_failure(payload, operation, action_kind)
        raise SystemExit(f"{operation} returned structured failure")

for payload in [
    artifact_verify_snapshot,
    artifact_verify_screenshot,
    artifact_verify_text,
]:
    if payload["ok"] is not True:
        assert_structured_artifact_failure(payload, "artifact verify", "verify")
        raise SystemExit("artifact verify returned structured failure")

before_tabs = tab_list_before["data"]
assert isinstance(before_tabs, list), tab_list_before
assert any(item["id"] == tab_id for item in before_tabs), tab_list_before

actions_payload = tab_list_actions["data"]
assert actions_payload["instance"]["id"] == instance_id, actions_payload
assert actions_payload["tab"]["id"] == tab_id, actions_payload
actions = actions_payload["actions"]
assert isinstance(actions, list) and actions, actions_payload
for required in ["navigate", "evaluate", "snapshot", "screenshot", "text", "recording"]:
    assert any(item["kind"] == required and isinstance(item["available"], bool) for item in actions), actions_payload

navigate = tab_action_navigate["data"]
assert navigate["tab"]["id"] == tab_id, navigate
assert navigate["requested"]["kind"] == "navigate", navigate
assert navigate["final_url"] == navigate_url, navigate
assert navigate["load_event_fired"] is True, navigate
assert isinstance(navigate["duration_ms"], int) and navigate["duration_ms"] >= 0, navigate
assert isinstance(navigate["detail"], str) and navigate["detail"], navigate

evaluate = tab_action_evaluate["data"]
assert evaluate["requested"]["kind"] == "evaluate", evaluate
assert evaluate["result"] == "After", evaluate
assert evaluate["final_url"] is None, evaluate
assert evaluate["load_event_fired"] is None, evaluate
assert evaluate["duration_ms"] is None, evaluate

snapshot = tab_snapshot["data"]
assert os.path.exists(snapshot["artifact"]["path"]), snapshot
assert isinstance(snapshot["snapshot"]["nodes"], list), snapshot
with open(snapshot["artifact"]["path"], "r", encoding="utf-8") as handle:
    persisted_snapshot = json.load(handle)
assert persisted_snapshot == snapshot["snapshot"], snapshot["artifact"]["path"]

screenshot = tab_screenshot["data"]
assert os.path.exists(screenshot["artifact"]["path"]), screenshot
assert tab_screenshot_sanity["path"] == screenshot["artifact"]["path"], tab_screenshot_sanity
assert tab_screenshot_sanity["mime_type"] == screenshot["artifact"]["mime_type"], tab_screenshot_sanity
assert tab_screenshot_sanity["size_bytes"] > 1024, tab_screenshot_sanity
if screenshot["artifact"]["mime_type"] == "image/png":
    assert tab_screenshot_sanity["png_header_valid"] is True, tab_screenshot_sanity

text = tab_text["data"]
assert text["text"].strip() == "AfterState", text
assert os.path.exists(text["artifact"]["path"]), text

artifact_entries = artifact_list["data"]["artifacts"]
assert artifact_list["data"]["instance_id"] == instance_id, artifact_list
assert artifact_list["data"]["run_id"] is None or isinstance(artifact_list["data"]["run_id"], str), artifact_list
assert isinstance(artifact_entries, list) and artifact_entries, artifact_list
artifact_ids = {item["id"] for item in artifact_entries}
for item in artifact_entries:
    assert isinstance(item["size_bytes"], int) and item["size_bytes"] >= 0, item
    assert isinstance(item["sha256"], str) and item["sha256"], item
    assert isinstance(item["created_at"], str) and item["created_at"], item
assert snapshot["artifact"]["id"] in artifact_ids, artifact_list
assert screenshot["artifact"]["id"] in artifact_ids, artifact_list
assert text["artifact"]["id"] in artifact_ids, artifact_list

for payload, artifact in [
    (artifact_list_run_snapshot, snapshot["artifact"]),
    (artifact_list_run_screenshot, screenshot["artifact"]),
    (artifact_list_run_text, text["artifact"]),
]:
    run_artifact_entries = payload["data"]["artifacts"]
    assert payload["data"]["instance_id"] is None, payload
    assert payload["data"]["run_id"] == artifact["run_id"], payload
    assert isinstance(run_artifact_entries, list) and len(run_artifact_entries) == 1, payload
    run_artifact_ids = {item["id"] for item in run_artifact_entries}
    assert run_artifact_ids == {artifact["id"]}, payload

assert len({snapshot["artifact"]["run_id"], screenshot["artifact"]["run_id"], text["artifact"]["run_id"]}) == 3, (
    snapshot["artifact"]["run_id"],
    screenshot["artifact"]["run_id"],
    text["artifact"]["run_id"],
)

for payload, artifact in [
    (artifact_verify_snapshot["data"], snapshot["artifact"]),
    (artifact_verify_screenshot["data"], screenshot["artifact"]),
    (artifact_verify_text["data"], text["artifact"]),
]:
    assert payload["id"] == artifact["id"], payload
    assert payload["path"] == artifact["path"], payload
    assert isinstance(payload["expected_sha256"], str) and payload["expected_sha256"], payload
    assert isinstance(payload["actual_sha256"], str) and payload["actual_sha256"], payload
    assert payload["valid"] is True, payload

closed = tab_close["data"]
assert closed["detail"].startswith("closed "), closed

after_tabs = tab_list_after["data"]
assert isinstance(after_tabs, list), tab_list_after
assert all(item["id"] != tab_id for item in after_tabs), tab_list_after

print(
    "\n".join(
        [
            "# Tab Lifecycle Integration",
            "",
            f"- output_dir: {output_dir}",
            f"- instance_id: {instance_id}",
            f"- snapshot_run_id: {snapshot['artifact']['run_id']}",
            f"- screenshot_run_id: {screenshot['artifact']['run_id']}",
            f"- text_run_id: {text['artifact']['run_id']}",
            f"- tab_id: {tab_id}",
            f"- screenshot_size_bytes: {tab_screenshot_sanity['size_bytes']}",
            f"- result: headless tab lifecycle completed with navigation, evaluate, persisted snapshot JSON validation, screenshot, text, artifact list by instance and per-command run, artifact verify, screenshot sanity checks, and close evidence",
        ]
    )
)
PY

printf '%s\n' "${output_dir}"
