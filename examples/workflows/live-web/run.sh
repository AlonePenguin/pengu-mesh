#!/bin/zsh
set -euo pipefail

script_dir="$(cd "$(dirname "$0")" && pwd)"
repo_root="$(cd "${script_dir}/../../.." && pwd)"
cd "${repo_root}"

source "${script_dir}/../common.sh"

output_dir="${1:-$(mktemp -d "${TMPDIR:-/tmp}/pengu-mesh-live-web.XXXXXX")}"
runtime_root="${PENGU_MESH_RUNTIME_ROOT:-${output_dir}/runtime-root}"
summary_path="${output_dir}/summary.md"
mkdir -p "${output_dir}" "${runtime_root}"

run_id=""
current_step_id=""
run_finished=0
instance_id=""
tab_id=""
snapshot_artifact_id=""
screenshot_artifact_id=""
text_artifact_id=""
snapshot_path=""
screenshot_path=""
text_path=""

page_url="https://example.com"

write_summary() {
  local summary_status="$1"
  cat > "${summary_path}" <<EOF
# Live Web Scenario

- status: ${summary_status}
- output_dir: ${output_dir}
- runtime_root: ${runtime_root}
- page_url: ${page_url}
- run_id: ${run_id}
- instance_id: ${instance_id}
- tab_id: ${tab_id}
- snapshot_artifact_id: ${snapshot_artifact_id}
- screenshot_artifact_id: ${screenshot_artifact_id}
- text_artifact_id: ${text_artifact_id}
- snapshot_path: ${snapshot_path}
- screenshot_path: ${screenshot_path}
- text_path: ${text_path}
EOF
}

cleanup() {
  local exit_code=$?

  if [[ -n "${current_step_id}" ]]; then
    scenario_finish_step_event \
      "${runtime_root}" \
      "${output_dir}/cleanup-finish-step.json" \
      "${current_step_id}" \
      "failed" \
      "script_exit_${exit_code}" || true
    current_step_id=""
  fi

  if [[ -n "${instance_id}" ]]; then
    PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
      "${cargo_bin}" run --quiet -p pengu-mesh -- \
      instance-stop --instance-id "${instance_id}" --holder-id scenario-agent \
      > "${output_dir}/cleanup-instance-stop.json" \
      2> "${output_dir}/cleanup-instance-stop.stderr.log" || true
    instance_id=""
  fi

  if [[ -n "${run_id}" && "${run_finished}" -ne 1 ]]; then
    write_summary "failed"
    scenario_finish_run_event \
      "${runtime_root}" \
      "${output_dir}/cleanup-finish-run.json" \
      "${run_id}" \
      "failed" \
      "${summary_path}" || true
  fi

  if (( exit_code != 0 )); then
    print -u2 "live-web scenario failed"
  fi
  exit "${exit_code}"
}

trap cleanup EXIT INT TERM

if [[ "$(uname -s)" != "Darwin" ]]; then
  cat > "${summary_path}" <<EOF
# Live Web Scenario

- status: skipped
- output_dir: ${output_dir}
- runtime_root: ${runtime_root}
- note: live-web currently runs only on Darwin because it launches managed Chrome Dev in the local gate baseline
EOF
  print "skipped"
  exit 0
fi

run_id="$(
  scenario_record_run_id \
    "${runtime_root}" \
    "${output_dir}/scenario-record-run.json" \
    "live-web" \
    "live-web" \
    "v1" \
    "cli"
)"

# Step 1: instance-start
current_step_id="$(
  scenario_record_step_id \
    "${runtime_root}" \
    "${output_dir}/step1-record.json" \
    "${run_id}" \
    "instance-start" \
    "command" \
    "pengu-mesh instance-start --name live-web --channel chrome-dev --headless --holder-id scenario-agent"
)"
instance_start_ms="$(run_pengu_json "${runtime_root}" "${output_dir}/instance-start.json" instance-start --name live-web --channel chrome-dev --headless --holder-id scenario-agent)"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step1-latency.json" "${run_id}" "${current_step_id}" "instance-start" "${instance_start_ms}"
instance_start_ok="$(
  /usr/bin/python3 - "${output_dir}/instance-start.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
print("true" if payload["ok"] else "false")
PY
)"
if ! record_equals_assertion "${runtime_root}" "${output_dir}/step1-assert-ok.json" "${run_id}" "${current_step_id}" "instance start ok" "true" "${instance_start_ok}" "instance-start should launch Chrome Dev headless for live-web navigation"; then
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step1-finish.json" "${current_step_id}" "failed" "instance_start"
  current_step_id=""
  exit 1
fi
instance_id="$(
  /usr/bin/python3 - "${output_dir}/instance-start.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
print(payload["data"]["id"])
PY
)"
scenario_finish_step_event "${runtime_root}" "${output_dir}/step1-finish.json" "${current_step_id}" "passed"
current_step_id=""

# Step 2: tab-open with live URL
current_step_id="$(
  scenario_record_step_id \
    "${runtime_root}" \
    "${output_dir}/step2-record.json" \
    "${run_id}" \
    "tab-open" \
    "command" \
    "pengu-mesh tab-open --instance-id ${instance_id} --url ${page_url} --holder-id scenario-agent"
)"
tab_open_ms="$(run_pengu_json "${runtime_root}" "${output_dir}/tab-open.json" tab-open --instance-id "${instance_id}" --url "${page_url}" --holder-id scenario-agent)"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step2-latency.json" "${run_id}" "${current_step_id}" "tab-open" "${tab_open_ms}"
tab_open_ok="$(
  /usr/bin/python3 - "${output_dir}/tab-open.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
print("true" if payload["ok"] else "false")
PY
)"
if ! record_equals_assertion "${runtime_root}" "${output_dir}/step2-assert-ok.json" "${run_id}" "${current_step_id}" "tab open ok" "true" "${tab_open_ok}" "tab-open should navigate to the live https://example.com page"; then
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step2-finish.json" "${current_step_id}" "failed" "tab_open"
  current_step_id=""
  exit 1
fi
tab_id="$(
  /usr/bin/python3 - "${output_dir}/tab-open.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
print(payload["data"]["id"])
PY
)"
sleep 2
scenario_finish_step_event "${runtime_root}" "${output_dir}/step2-finish.json" "${current_step_id}" "passed"
current_step_id=""

# Step 3: tab-snapshot
current_step_id="$(
  scenario_record_step_id \
    "${runtime_root}" \
    "${output_dir}/step3-record.json" \
    "${run_id}" \
    "tab-snapshot" \
    "command" \
    "pengu-mesh tab-snapshot --tab-id ${tab_id} --holder-id scenario-agent"
)"
snapshot_ms="$(run_pengu_json "${runtime_root}" "${output_dir}/tab-snapshot.json" tab-snapshot --tab-id "${tab_id}" --holder-id scenario-agent)"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step3-latency.json" "${run_id}" "${current_step_id}" "tab-snapshot" "${snapshot_ms}"
snapshot_ok="$(
  /usr/bin/python3 - "${output_dir}/tab-snapshot.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
print("true" if payload["ok"] else "false")
PY
)"
if ! record_equals_assertion "${runtime_root}" "${output_dir}/step3-assert-ok.json" "${run_id}" "${current_step_id}" "snapshot ok" "true" "${snapshot_ok}" "tab-snapshot should persist DOM evidence from the live page"; then
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step3-finish.json" "${current_step_id}" "failed" "tab_snapshot"
  current_step_id=""
  exit 1
fi
snapshot_artifact_id="$(
  /usr/bin/python3 - "${output_dir}/tab-snapshot.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
print(payload["data"]["artifact"]["id"])
PY
)"
snapshot_path="$(
  /usr/bin/python3 - "${output_dir}/tab-snapshot.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
print(payload["data"]["artifact"]["path"])
PY
)"
scenario_finish_step_event "${runtime_root}" "${output_dir}/step3-finish.json" "${current_step_id}" "passed"
current_step_id=""

# Step 4: tab-screenshot
current_step_id="$(
  scenario_record_step_id \
    "${runtime_root}" \
    "${output_dir}/step4-record.json" \
    "${run_id}" \
    "tab-screenshot" \
    "command" \
    "pengu-mesh tab-screenshot --tab-id ${tab_id} --holder-id scenario-agent"
)"
screenshot_ms="$(run_pengu_json "${runtime_root}" "${output_dir}/tab-screenshot.json" tab-screenshot --tab-id "${tab_id}" --holder-id scenario-agent)"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step4-latency.json" "${run_id}" "${current_step_id}" "tab-screenshot" "${screenshot_ms}"
screenshot_ok="$(
  /usr/bin/python3 - "${output_dir}/tab-screenshot.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
print("true" if payload["ok"] else "false")
PY
)"
if ! record_equals_assertion "${runtime_root}" "${output_dir}/step4-assert-ok.json" "${run_id}" "${current_step_id}" "screenshot ok" "true" "${screenshot_ok}" "Expected the example.com default page with Example Domain heading."; then
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step4-finish.json" "${current_step_id}" "failed" "tab_screenshot"
  current_step_id=""
  exit 1
fi
screenshot_artifact_id="$(
  /usr/bin/python3 - "${output_dir}/tab-screenshot.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
print(payload["data"]["artifact"]["id"])
PY
)"
screenshot_path="$(
  /usr/bin/python3 - "${output_dir}/tab-screenshot.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
print(payload["data"]["artifact"]["path"])
PY
)"
scenario_finish_step_event "${runtime_root}" "${output_dir}/step4-finish.json" "${current_step_id}" "passed"
current_step_id=""

# Step 5: tab-text
current_step_id="$(
  scenario_record_step_id \
    "${runtime_root}" \
    "${output_dir}/step5-record.json" \
    "${run_id}" \
    "tab-text" \
    "command" \
    "pengu-mesh tab-text --tab-id ${tab_id} --holder-id scenario-agent"
)"
text_ms="$(run_pengu_json "${runtime_root}" "${output_dir}/tab-text.json" tab-text --tab-id "${tab_id}" --holder-id scenario-agent)"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step5-latency.json" "${run_id}" "${current_step_id}" "tab-text" "${text_ms}"
text_ok="$(
  /usr/bin/python3 - "${output_dir}/tab-text.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
print("true" if payload["ok"] else "false")
PY
)"
if ! record_equals_assertion "${runtime_root}" "${output_dir}/step5-assert-ok.json" "${run_id}" "${current_step_id}" "text ok" "true" "${text_ok}" "tab-text should extract text from the live page"; then
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step5-finish.json" "${current_step_id}" "failed" "tab_text"
  current_step_id=""
  exit 1
fi
page_text="$(
  /usr/bin/python3 - "${output_dir}/tab-text.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
print(payload["data"]["text"])
PY
)"
if ! record_contains_assertion "${runtime_root}" "${output_dir}/step5-assert-content.json" "${run_id}" "${current_step_id}" "text contains headline" "Example Domain" "${page_text}" "tab-text should capture the Example Domain headline from the live page"; then
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step5-finish.json" "${current_step_id}" "failed" "tab_text_content"
  current_step_id=""
  exit 1
fi
text_artifact_id="$(
  /usr/bin/python3 - "${output_dir}/tab-text.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
print(payload["data"]["artifact"]["id"])
PY
)"
text_path="$(
  /usr/bin/python3 - "${output_dir}/tab-text.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
print(payload["data"]["artifact"]["path"])
PY
)"
scenario_finish_step_event "${runtime_root}" "${output_dir}/step5-finish.json" "${current_step_id}" "passed"
current_step_id=""

# Step 6: artifact-list plus artifact-verify for all three artifacts
current_step_id="$(
  scenario_record_step_id \
    "${runtime_root}" \
    "${output_dir}/step6-record.json" \
    "${run_id}" \
    "artifact-list-and-verify" \
    "command" \
    "pengu-mesh artifact-list --instance-id ${instance_id}; artifact-verify on snapshot/screenshot/text"
)"
artifact_list_ms="$(run_pengu_json "${runtime_root}" "${output_dir}/artifact-list.json" artifact-list --instance-id "${instance_id}")"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step6-latency-list.json" "${run_id}" "${current_step_id}" "artifact-list" "${artifact_list_ms}"
verify_snapshot_ms="$(run_pengu_json "${runtime_root}" "${output_dir}/artifact-verify-snapshot.json" artifact-verify --artifact-id "${snapshot_artifact_id}")"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step6-latency-snapshot.json" "${run_id}" "${current_step_id}" "artifact-verify-snapshot" "${verify_snapshot_ms}"
verify_screenshot_ms="$(run_pengu_json "${runtime_root}" "${output_dir}/artifact-verify-screenshot.json" artifact-verify --artifact-id "${screenshot_artifact_id}")"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step6-latency-screenshot.json" "${run_id}" "${current_step_id}" "artifact-verify-screenshot" "${verify_screenshot_ms}"
verify_text_ms="$(run_pengu_json "${runtime_root}" "${output_dir}/artifact-verify-text.json" artifact-verify --artifact-id "${text_artifact_id}")"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step6-latency-text.json" "${run_id}" "${current_step_id}" "artifact-verify-text" "${verify_text_ms}"

artifacts_present="$(
  /usr/bin/python3 - "${output_dir}/artifact-list.json" "${snapshot_artifact_id}" "${screenshot_artifact_id}" "${text_artifact_id}" <<'PY'
import json
import sys

path, snapshot_id, screenshot_id, text_id = sys.argv[1:5]
with open(path, "r", encoding="utf-8") as handle:
    payload = json.load(handle)
ids = {item["id"] for item in payload["data"]["artifacts"]}
print("true" if {snapshot_id, screenshot_id, text_id}.issubset(ids) else "false")
PY
)"
if ! record_equals_assertion "${runtime_root}" "${output_dir}/step6-assert-present.json" "${run_id}" "${current_step_id}" "all artifacts present after live capture" "true" "${artifacts_present}" "artifact-list should report snapshot, screenshot, and text artifacts from the live page"; then
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step6-finish.json" "${current_step_id}" "failed" "artifact_presence"
  current_step_id=""
  exit 1
fi

for kind in snapshot screenshot text; do
  valid="$(
    /usr/bin/python3 - "${output_dir}/artifact-verify-${kind}.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
print("true" if payload["data"]["valid"] else "false")
PY
  )"
  if ! record_equals_assertion "${runtime_root}" "${output_dir}/step6-assert-${kind}.json" "${run_id}" "${current_step_id}" "artifact verify ${kind} valid" "true" "${valid}" "artifact-verify should pass for ${kind} captured from the live page"; then
    scenario_finish_step_event "${runtime_root}" "${output_dir}/step6-finish.json" "${current_step_id}" "failed" "artifact_verify_${kind}"
    current_step_id=""
    exit 1
  fi
done
scenario_finish_step_event "${runtime_root}" "${output_dir}/step6-finish.json" "${current_step_id}" "passed"
current_step_id=""

# Step 7: instance-stop
current_step_id="$(
  scenario_record_step_id \
    "${runtime_root}" \
    "${output_dir}/step7-record.json" \
    "${run_id}" \
    "instance-stop" \
    "command" \
    "pengu-mesh instance-stop --instance-id ${instance_id} --holder-id scenario-agent"
)"
instance_stop_ms="$(run_pengu_json "${runtime_root}" "${output_dir}/instance-stop.json" instance-stop --instance-id "${instance_id}" --holder-id scenario-agent)"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step7-latency.json" "${run_id}" "${current_step_id}" "instance-stop" "${instance_stop_ms}"
instance_stop_ok="$(
  /usr/bin/python3 - "${output_dir}/instance-stop.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
print("true" if payload["ok"] else "false")
PY
)"
if ! record_equals_assertion "${runtime_root}" "${output_dir}/step7-assert-ok.json" "${run_id}" "${current_step_id}" "instance stop ok" "true" "${instance_stop_ok}" "instance-stop should close the browser cleanly after live-web capture"; then
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step7-finish.json" "${current_step_id}" "failed" "instance_stop"
  current_step_id=""
  exit 1
fi
scenario_finish_step_event "${runtime_root}" "${output_dir}/step7-finish.json" "${current_step_id}" "passed"
current_step_id=""
instance_id=""

write_summary "passed"
scenario_finish_run_event "${runtime_root}" "${output_dir}/scenario-finish-run.json" "${run_id}" "passed" "${summary_path}"
run_finished=1

print "${run_id}"
