#!/bin/zsh
set -euo pipefail

script_dir="$(cd "$(dirname "$0")" && pwd)"
repo_root="$(cd "${script_dir}/../../.." && pwd)"
cd "${repo_root}"

source "${script_dir}/../common.sh"

output_dir="${1:-$(mktemp -d "${TMPDIR:-/tmp}/pengu-mesh-startup-readiness.XXXXXX")}"
runtime_root="${PENGU_MESH_RUNTIME_ROOT:-${output_dir}/runtime-root}"
summary_path="${output_dir}/summary.md"
mkdir -p "${output_dir}" "${runtime_root}"

run_id=""
current_step_id=""
run_finished=0
instance_id=""
tab_id=""
screenshot_artifact_id=""
screenshot_path=""

page_url="$(
  cat <<'EOF' | html_data_url_from_stdin
<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <title>Startup Readiness</title>
    <style>
      body {
        margin: 0;
        font-family: "Georgia", "Times New Roman", serif;
        background:
          radial-gradient(circle at top left, #f8f2c8 0%, transparent 34%),
          linear-gradient(135deg, #dff5e1 0%, #f5fbef 100%);
        color: #173b2f;
        min-height: 100vh;
        display: grid;
        place-items: center;
      }
      main {
        width: min(720px, 86vw);
        padding: 32px 40px;
        border: 2px solid rgba(23, 59, 47, 0.18);
        border-radius: 24px;
        background: rgba(255, 255, 255, 0.76);
        box-shadow: 0 20px 60px rgba(23, 59, 47, 0.12);
      }
      h1 {
        font-size: 42px;
        margin: 0 0 10px;
        letter-spacing: 0.02em;
      }
      p {
        font-size: 20px;
        line-height: 1.5;
        margin: 0 0 18px;
      }
      ul {
        margin: 0;
        padding-left: 24px;
        font-size: 18px;
        line-height: 1.6;
      }
      footer {
        margin-top: 22px;
        font-size: 14px;
        text-transform: uppercase;
        letter-spacing: 0.14em;
      }
    </style>
  </head>
  <body>
    <main>
      <h1>Startup readiness</h1>
      <p>Scenario startup check for health, diagnostics, and first browser proof.</p>
      <ul>
        <li>Health and diagnose should be readable under an isolated runtime root.</li>
        <li>Managed Chrome Dev should launch headless and open this page.</li>
        <li>Screenshot evidence should preserve the headline and readiness copy.</li>
      </ul>
      <footer>Scenario startup check</footer>
    </main>
  </body>
</html>
EOF
)"

write_summary() {
  local summary_status="$1"
  cat > "${summary_path}" <<EOF
# Startup Readiness Scenario

- status: ${summary_status}
- output_dir: ${output_dir}
- runtime_root: ${runtime_root}
- run_id: ${run_id}
- instance_id: ${instance_id}
- tab_id: ${tab_id}
- screenshot_artifact_id: ${screenshot_artifact_id}
- screenshot_path: ${screenshot_path}
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
    print -u2 "startup-readiness scenario failed"
  fi
  exit "${exit_code}"
}

trap cleanup EXIT INT TERM

if [[ "$(uname -s)" != "Darwin" ]]; then
  cat > "${summary_path}" <<EOF
# Startup Readiness Scenario

- status: skipped
- output_dir: ${output_dir}
- runtime_root: ${runtime_root}
- note: startup-readiness currently runs only on Darwin because it launches managed Chrome Dev in the local gate baseline
EOF
  print "skipped"
  exit 0
fi

run_id="$(
  scenario_record_run_id \
    "${runtime_root}" \
    "${output_dir}/scenario-record-run.json" \
    "startup-readiness" \
    "startup-readiness" \
    "v1" \
    "cli"
)"

current_step_id="$(
  scenario_record_step_id \
    "${runtime_root}" \
    "${output_dir}/step1-record.json" \
    "${run_id}" \
    "health" \
    "command" \
    "pengu-mesh health"
)"
health_ms="$(run_pengu_json "${runtime_root}" "${output_dir}/health.json" health)"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step1-latency.json" "${run_id}" "${current_step_id}" "health" "${health_ms}"
health_ok="$(
  /usr/bin/python3 - "${output_dir}/health.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
print("true" if payload["ok"] else "false")
PY
)"
if ! record_equals_assertion "${runtime_root}" "${output_dir}/step1-assert-ok.json" "${run_id}" "${current_step_id}" "health ok" "true" "${health_ok}" "health should return an ok envelope"; then
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step1-finish.json" "${current_step_id}" "failed" "health_not_ok"
  current_step_id=""
  exit 1
fi
scenario_finish_step_event "${runtime_root}" "${output_dir}/step1-finish.json" "${current_step_id}" "passed"
current_step_id=""

current_step_id="$(
  scenario_record_step_id \
    "${runtime_root}" \
    "${output_dir}/step2-record.json" \
    "${run_id}" \
    "diagnose" \
    "command" \
    "pengu-mesh diagnose"
)"
diagnose_ms="$(run_pengu_json "${runtime_root}" "${output_dir}/diagnose.json" diagnose)"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step2-latency.json" "${run_id}" "${current_step_id}" "diagnose" "${diagnose_ms}"
diagnose_state="$(
  /usr/bin/python3 - "${output_dir}/diagnose.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
print(payload["data"]["state"])
PY
)"
permissions_count="$(
  /usr/bin/python3 - "${output_dir}/diagnose.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
print(len(payload["data"]["permissions"]))
PY
)"
diagnose_state_status="failed"
diagnose_state_category="unexpected_state"
if [[ "${diagnose_state}" == "ready" || "${diagnose_state}" == "degraded" ]]; then
  diagnose_state_status="passed"
  diagnose_state_category=""
fi
scenario_record_assertion_event \
  "${runtime_root}" \
  "${output_dir}/step2-assert-state.json" \
  "${run_id}" \
  "${current_step_id}" \
  "diagnose state ready or degraded" \
  "ready|degraded" \
  "${diagnose_state}" \
  "${diagnose_state_status}" \
  "${diagnose_state_category}" \
  "diagnose should be actionable without blocking startup proof"
if [[ "${diagnose_state_status}" != "passed" ]]; then
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step2-finish.json" "${current_step_id}" "failed" "diagnose_state"
  current_step_id=""
  exit 1
fi
if ! record_positive_integer_assertion "${runtime_root}" "${output_dir}/step2-assert-permissions.json" "${run_id}" "${current_step_id}" "diagnose permissions non-empty" "${permissions_count}" "diagnose should enumerate at least one permission entry"; then
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step2-finish.json" "${current_step_id}" "failed" "diagnose_permissions"
  current_step_id=""
  exit 1
fi
scenario_finish_step_event "${runtime_root}" "${output_dir}/step2-finish.json" "${current_step_id}" "passed"
current_step_id=""

current_step_id="$(
  scenario_record_step_id \
    "${runtime_root}" \
    "${output_dir}/step3-record.json" \
    "${run_id}" \
    "doctor" \
    "command" \
    "pengu-mesh-doctor -- --json"
)"
doctor_ms="$(run_doctor_json "${runtime_root}" "${output_dir}/doctor.json")"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step3-latency.json" "${run_id}" "${current_step_id}" "doctor" "${doctor_ms}"
doctor_ok="$(
  /usr/bin/python3 - "${output_dir}/doctor.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
print("true" if payload["ok"] else "false")
PY
)"
if ! record_equals_assertion "${runtime_root}" "${output_dir}/step3-assert-ok.json" "${run_id}" "${current_step_id}" "doctor ok" "true" "${doctor_ok}" "doctor should return an ok envelope"; then
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step3-finish.json" "${current_step_id}" "failed" "doctor_not_ok"
  current_step_id=""
  exit 1
fi
scenario_finish_step_event "${runtime_root}" "${output_dir}/step3-finish.json" "${current_step_id}" "passed"
current_step_id=""

current_step_id="$(
  scenario_record_step_id \
    "${runtime_root}" \
    "${output_dir}/step4-record.json" \
    "${run_id}" \
    "instance-start" \
    "command" \
    "pengu-mesh instance-start --name scenario-startup --channel chrome-dev --headless --holder-id scenario-agent"
)"
instance_start_ms="$(run_pengu_json "${runtime_root}" "${output_dir}/instance-start.json" instance-start --name scenario-startup --channel chrome-dev --headless --holder-id scenario-agent)"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step4-latency.json" "${run_id}" "${current_step_id}" "instance-start" "${instance_start_ms}"
instance_start_ok="$(
  /usr/bin/python3 - "${output_dir}/instance-start.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
print("true" if payload["ok"] else "false")
PY
)"
if ! record_equals_assertion "${runtime_root}" "${output_dir}/step4-assert-ok.json" "${run_id}" "${current_step_id}" "instance start ok" "true" "${instance_start_ok}" "instance-start should launch Chrome Dev headless"; then
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step4-finish.json" "${current_step_id}" "failed" "instance_start"
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
scenario_finish_step_event "${runtime_root}" "${output_dir}/step4-finish.json" "${current_step_id}" "passed"
current_step_id=""

current_step_id="$(
  scenario_record_step_id \
    "${runtime_root}" \
    "${output_dir}/step5-record.json" \
    "${run_id}" \
    "tab-open" \
    "command" \
    "pengu-mesh tab-open --instance-id ${instance_id} --url <base64-html> --holder-id scenario-agent"
)"
tab_open_ms="$(run_pengu_json "${runtime_root}" "${output_dir}/tab-open.json" tab-open --instance-id "${instance_id}" --url "${page_url}" --holder-id scenario-agent)"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step5-latency.json" "${run_id}" "${current_step_id}" "tab-open" "${tab_open_ms}"
tab_open_ok="$(
  /usr/bin/python3 - "${output_dir}/tab-open.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
print("true" if payload["ok"] else "false")
PY
)"
if ! record_equals_assertion "${runtime_root}" "${output_dir}/step5-assert-ok.json" "${run_id}" "${current_step_id}" "tab open ok" "true" "${tab_open_ok}" "tab-open should load the base64 startup page"; then
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step5-finish.json" "${current_step_id}" "failed" "tab_open"
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
sleep 1
scenario_finish_step_event "${runtime_root}" "${output_dir}/step5-finish.json" "${current_step_id}" "passed"
current_step_id=""

current_step_id="$(
  scenario_record_step_id \
    "${runtime_root}" \
    "${output_dir}/step6-record.json" \
    "${run_id}" \
    "tab-screenshot" \
    "command" \
    "pengu-mesh tab-screenshot --tab-id ${tab_id} --holder-id scenario-agent"
)"
screenshot_ms="$(run_pengu_json "${runtime_root}" "${output_dir}/tab-screenshot.json" tab-screenshot --tab-id "${tab_id}" --holder-id scenario-agent)"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step6-latency.json" "${run_id}" "${current_step_id}" "tab-screenshot" "${screenshot_ms}"
screenshot_ok="$(
  /usr/bin/python3 - "${output_dir}/tab-screenshot.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
print("true" if payload["ok"] else "false")
PY
)"
if ! record_equals_assertion "${runtime_root}" "${output_dir}/step6-assert-ok.json" "${run_id}" "${current_step_id}" "screenshot ok" "true" "${screenshot_ok}" "Expected pale mint page with a Startup readiness headline, readiness bullets, and a Scenario startup check footer."; then
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step6-finish.json" "${current_step_id}" "failed" "tab_screenshot"
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
scenario_finish_step_event "${runtime_root}" "${output_dir}/step6-finish.json" "${current_step_id}" "passed"
current_step_id=""

current_step_id="$(
  scenario_record_step_id \
    "${runtime_root}" \
    "${output_dir}/step7-record.json" \
    "${run_id}" \
    "artifact-verify" \
    "command" \
    "pengu-mesh artifact-verify --artifact-id ${screenshot_artifact_id}"
)"
artifact_verify_ms="$(run_pengu_json "${runtime_root}" "${output_dir}/artifact-verify.json" artifact-verify --artifact-id "${screenshot_artifact_id}")"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step7-latency.json" "${run_id}" "${current_step_id}" "artifact-verify" "${artifact_verify_ms}"
artifact_valid="$(
  /usr/bin/python3 - "${output_dir}/artifact-verify.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
print("true" if payload["data"]["valid"] else "false")
PY
)"
if ! record_equals_assertion "${runtime_root}" "${output_dir}/step7-assert-valid.json" "${run_id}" "${current_step_id}" "artifact verify valid" "true" "${artifact_valid}" "screenshot artifact checksum should validate before shutdown"; then
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step7-finish.json" "${current_step_id}" "failed" "artifact_verify"
  current_step_id=""
  exit 1
fi
scenario_finish_step_event "${runtime_root}" "${output_dir}/step7-finish.json" "${current_step_id}" "passed"
current_step_id=""

current_step_id="$(
  scenario_record_step_id \
    "${runtime_root}" \
    "${output_dir}/step8-record.json" \
    "${run_id}" \
    "instance-stop" \
    "command" \
    "pengu-mesh instance-stop --instance-id ${instance_id} --holder-id scenario-agent"
)"
instance_stop_ms="$(run_pengu_json "${runtime_root}" "${output_dir}/instance-stop.json" instance-stop --instance-id "${instance_id}" --holder-id scenario-agent)"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step8-latency.json" "${run_id}" "${current_step_id}" "instance-stop" "${instance_stop_ms}"
instance_stop_ok="$(
  /usr/bin/python3 - "${output_dir}/instance-stop.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
print("true" if payload["ok"] else "false")
PY
)"
if ! record_equals_assertion "${runtime_root}" "${output_dir}/step8-assert-ok.json" "${run_id}" "${current_step_id}" "instance stop ok" "true" "${instance_stop_ok}" "instance-stop should close the managed browser cleanly"; then
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step8-finish.json" "${current_step_id}" "failed" "instance_stop"
  current_step_id=""
  exit 1
fi
scenario_finish_step_event "${runtime_root}" "${output_dir}/step8-finish.json" "${current_step_id}" "passed"
current_step_id=""
instance_id=""

write_summary "passed"
scenario_finish_run_event "${runtime_root}" "${output_dir}/scenario-finish-run.json" "${run_id}" "passed" "${summary_path}"
run_finished=1

print "${run_id}"
