#!/bin/zsh
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$repo_root"

if [[ -x "${HOME}/.cargo/bin/cargo" ]]; then
  cargo_bin="${HOME}/.cargo/bin/cargo"
else
  cargo_bin="$(command -v cargo)"
fi

output_dir="${1:-$(mktemp -d "${TMPDIR:-/tmp}/pengu-mesh-evidence-chain.XXXXXX")}"
runtime_root="${output_dir}/runtime-root"
mkdir -p "$output_dir" "$runtime_root"

instance_id=""
tab_id=""
holder_id="evidence-chain-writer"
open_url='data:text/html;base64,PGh0bWw+PGJvZHkgc3R5bGU9ImJhY2tncm91bmQ6IzBiM2Q5MTtjb2xvcjp3aGl0ZTtmb250LXNpemU6MzZweDtwYWRkaW5nOjQwcHgiPjxoMT5FVklERU5DRSBDSEFJTiBURVNUPC9oMT48cD5BcnRpZmFjdHMgc2hvdWxkIHZlcmlmeSBiZWZvcmUgY29ycnVwdGlvbi48L3A+PC9ib2R5PjwvaHRtbD4='

cleanup() {
  if [[ -n "${instance_id}" ]]; then
    PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
      "${cargo_bin}" run --quiet -p pengu-mesh -- instance-stop --instance-id "${instance_id}" --holder-id "${holder_id}" > "${output_dir}/instance-stop.json" 2> "${output_dir}/instance-stop.stderr.log" || true
  fi
}

trap cleanup EXIT INT TERM

if [[ "$(uname -s)" != "Darwin" ]]; then
  cat > "${output_dir}/summary.md" <<EOF
# Evidence Chain Smoke

- output_dir: ${output_dir}
- skipped: evidence-chain smoke currently runs only on Darwin in the local gate baseline
EOF
  printf '%s\n' "${output_dir}"
  exit 0
fi

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- instance-start --name evidence-chain --channel chrome-dev --headless --holder-id "${holder_id}" > "${output_dir}/instance-start.json"

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
  /usr/bin/python3 - "${output_dir}/tab-open.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
assert payload["ok"] is True, payload
print(payload["data"]["id"])
PY
)"

sleep 1

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- tab-snapshot --tab-id "${tab_id}" --holder-id "${holder_id}" > "${output_dir}/tab-snapshot.json"

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- tab-screenshot --tab-id "${tab_id}" --holder-id "${holder_id}" > "${output_dir}/tab-screenshot.json"

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- tab-text --tab-id "${tab_id}" --holder-id "${holder_id}" > "${output_dir}/tab-text.json"

/usr/bin/python3 - "${output_dir}" > "${output_dir}/artifact-metadata.json" <<'PY'
import json
import os
import sys

output_dir = sys.argv[1]


def load(name):
    with open(os.path.join(output_dir, f"{name}.json"), "r", encoding="utf-8") as handle:
        return json.load(handle)


snapshot = load("tab-snapshot")
screenshot = load("tab-screenshot")
text = load("tab-text")

for payload in [snapshot, screenshot, text]:
    assert payload["ok"] is True, payload

print(
    json.dumps(
        {
            "snapshot_run_id": snapshot["data"]["artifact"]["run_id"],
            "screenshot_run_id": screenshot["data"]["artifact"]["run_id"],
            "text_run_id": text["data"]["artifact"]["run_id"],
            "snapshot_artifact_id": snapshot["data"]["artifact"]["id"],
            "screenshot_artifact_id": screenshot["data"]["artifact"]["id"],
            "text_artifact_id": text["data"]["artifact"]["id"],
            "snapshot_path": snapshot["data"]["artifact"]["path"],
            "screenshot_path": screenshot["data"]["artifact"]["path"],
            "text_path": text["data"]["artifact"]["path"],
        },
        indent=2,
    )
)
PY

snapshot_artifact_id="$(
  /usr/bin/python3 - "${output_dir}/artifact-metadata.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
print(payload["snapshot_artifact_id"])
PY
)"

screenshot_artifact_id="$(
  /usr/bin/python3 - "${output_dir}/artifact-metadata.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
print(payload["screenshot_artifact_id"])
PY
)"

text_artifact_id="$(
  /usr/bin/python3 - "${output_dir}/artifact-metadata.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
print(payload["text_artifact_id"])
PY
)"

text_path="$(
  /usr/bin/python3 - "${output_dir}/artifact-metadata.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
print(payload["text_path"])
PY
)"

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- artifact-list --instance-id "${instance_id}" > "${output_dir}/artifact-list-instance-before.json"

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- artifact-verify --artifact-id "${snapshot_artifact_id}" > "${output_dir}/artifact-verify-snapshot.json"

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- artifact-verify --artifact-id "${screenshot_artifact_id}" > "${output_dir}/artifact-verify-screenshot.json"

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- artifact-verify --artifact-id "${text_artifact_id}" > "${output_dir}/artifact-verify-text.json"

/usr/bin/python3 - "${text_path}" > "${output_dir}/corruption.json" <<'PY'
import json
import os
import sys

path = sys.argv[1]
size_before = os.path.getsize(path)
with open(path, "ab") as handle:
    handle.write(b"\nCORRUPTED-EVIDENCE-CHAIN\n")
size_after = os.path.getsize(path)
assert size_after > size_before, (size_before, size_after)

print(
    json.dumps(
        {
            "path": path,
            "size_before": size_before,
            "size_after": size_after,
        },
        indent=2,
    )
)
PY

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- artifact-verify --artifact-id "${text_artifact_id}" > "${output_dir}/artifact-verify-text-corrupted.json"

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- artifact-list --instance-id "${instance_id}" > "${output_dir}/artifact-list-instance-after-corruption.json"

/usr/bin/python3 - "${output_dir}" "${instance_id}" "${tab_id}" > "${output_dir}/summary.md" <<'PY'
import json
import os
import sys

output_dir, instance_id, tab_id = sys.argv[1:4]


def load(name):
    with open(os.path.join(output_dir, f"{name}.json"), "r", encoding="utf-8") as handle:
        return json.load(handle)


artifact_metadata = load("artifact-metadata")
artifact_list_instance_before = load("artifact-list-instance-before")
artifact_list_instance_after = load("artifact-list-instance-after-corruption")
artifact_verify_snapshot = load("artifact-verify-snapshot")
artifact_verify_screenshot = load("artifact-verify-screenshot")
artifact_verify_text = load("artifact-verify-text")
artifact_verify_text_corrupted = load("artifact-verify-text-corrupted")
corruption = load("corruption")
tab_snapshot = load("tab-snapshot")
tab_screenshot = load("tab-screenshot")
tab_text = load("tab-text")

for payload in [
    artifact_list_instance_before,
    artifact_list_instance_after,
    artifact_verify_snapshot,
    artifact_verify_screenshot,
    artifact_verify_text,
    artifact_verify_text_corrupted,
]:
    assert payload["ok"] is True, payload

before_entries = {
    item["id"]: item for item in artifact_list_instance_before["data"]["artifacts"]
}
after_entries = {
    item["id"]: item for item in artifact_list_instance_after["data"]["artifacts"]
}
expected_ids = {
    artifact_metadata["snapshot_artifact_id"],
    artifact_metadata["screenshot_artifact_id"],
    artifact_metadata["text_artifact_id"],
}
assert expected_ids.issubset(before_entries.keys()), before_entries
assert expected_ids.issubset(after_entries.keys()), after_entries
assert artifact_list_instance_before["data"]["instance_id"] == instance_id, artifact_list_instance_before
assert artifact_list_instance_after["data"]["instance_id"] == instance_id, artifact_list_instance_after
assert len(
    {
        artifact_metadata["snapshot_run_id"],
        artifact_metadata["screenshot_run_id"],
        artifact_metadata["text_run_id"],
    }
) == 3, artifact_metadata

for artifact_id in expected_ids:
    assert before_entries[artifact_id]["sha256"] == after_entries[artifact_id]["sha256"], artifact_id
    assert before_entries[artifact_id]["size_bytes"] == after_entries[artifact_id]["size_bytes"], artifact_id

snapshot_verify = artifact_verify_snapshot["data"]
screenshot_verify = artifact_verify_screenshot["data"]
text_verify = artifact_verify_text["data"]
text_verify_corrupted = artifact_verify_text_corrupted["data"]

for payload in [snapshot_verify, screenshot_verify, text_verify]:
    assert payload["valid"] is True, payload
    assert payload["expected_sha256"] == payload["actual_sha256"], payload

assert text_verify_corrupted["id"] == artifact_metadata["text_artifact_id"], text_verify_corrupted
assert text_verify_corrupted["path"] == artifact_metadata["text_path"], text_verify_corrupted
assert text_verify_corrupted["valid"] is False, text_verify_corrupted
assert text_verify_corrupted["expected_sha256"] == text_verify["expected_sha256"], text_verify_corrupted
assert text_verify_corrupted["actual_sha256"] != text_verify_corrupted["expected_sha256"], text_verify_corrupted
assert corruption["path"] == artifact_metadata["text_path"], corruption
assert corruption["size_after"] > corruption["size_before"], corruption

screenshot_artifact = tab_screenshot["data"]["artifact"]
assert screenshot_artifact["id"] == artifact_metadata["screenshot_artifact_id"], screenshot_artifact
assert os.path.exists(artifact_metadata["screenshot_path"]), artifact_metadata
with open(artifact_metadata["snapshot_path"], "r", encoding="utf-8") as handle:
    persisted_snapshot = json.load(handle)
assert persisted_snapshot == tab_snapshot["data"]["snapshot"], artifact_metadata["snapshot_path"]
assert any(
    "EVIDENCE CHAIN TEST" in (node.get("text") or "") or "EVIDENCE CHAIN TEST" in (node.get("name") or "")
    for node in tab_snapshot["data"]["snapshot"]["nodes"]
), tab_snapshot
assert "EVIDENCE CHAIN TEST" in tab_text["data"]["text"], tab_text
assert "Artifacts should verify before corruption." in tab_text["data"]["text"], tab_text

print(
    "\n".join(
        [
            "# Evidence Chain Smoke",
            "",
            f"- output_dir: {output_dir}",
            f"- instance_id: {instance_id}",
            f"- tab_id: {tab_id}",
            f"- snapshot_run_id: {artifact_metadata['snapshot_run_id']}",
            f"- screenshot_run_id: {artifact_metadata['screenshot_run_id']}",
            f"- text_run_id: {artifact_metadata['text_run_id']}",
            f"- screenshot_artifact_id: {artifact_metadata['screenshot_artifact_id']}",
            f"- screenshot_path: {artifact_metadata['screenshot_path']}",
            f"- corrupted_artifact_id: {artifact_metadata['text_artifact_id']}",
            "- result: artifact list and verify passed before corruption, persisted snapshot JSON reopened cleanly, text artifact verify failed after corruption, instance-scoped metadata remained unchanged, and each standalone capture kept a distinct run id",
        ]
    )
)
PY

printf '%s\n' "${output_dir}"
