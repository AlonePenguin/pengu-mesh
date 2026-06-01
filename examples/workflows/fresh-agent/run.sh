#!/bin/zsh
set -euo pipefail

script_dir="$(cd "$(dirname "$0")" && pwd)"
repo_root="$(cd "${script_dir}/../../.." && pwd)"
cd "${repo_root}"

source "${script_dir}/../common.sh"

output_dir="${1:-$(mktemp -d "${TMPDIR:-/tmp}/pengu-mesh-fresh-agent.XXXXXX")}"
runtime_root="${PENGU_MESH_RUNTIME_ROOT:-${output_dir}/runtime-root}"
summary_path="${output_dir}/summary.md"
mkdir -p "${output_dir}" "${runtime_root}"

run_id=""
current_step_id=""
run_finished=0
instance_id=""

write_summary() {
  local summary_status="$1"
  cat > "${summary_path}" <<EOF
# Fresh Agent Scenario

- status: ${summary_status}
- output_dir: ${output_dir}
- runtime_root: ${runtime_root}
- run_id: ${run_id}
- instance_id: ${instance_id}
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
    print -u2 "fresh-agent scenario failed"
  fi
  exit "${exit_code}"
}

trap cleanup EXIT INT TERM

if [[ "$(uname -s)" != "Darwin" ]]; then
  cat > "${summary_path}" <<EOF
# Fresh Agent Scenario

- status: skipped
- output_dir: ${output_dir}
- runtime_root: ${runtime_root}
- note: fresh-agent currently runs only on Darwin because it launches managed Chrome Dev
EOF
  print "skipped"
  exit 0
fi

run_id="$(
  scenario_record_run_id \
    "${runtime_root}" \
    "${output_dir}/scenario-record-run.json" \
    "fresh-agent" \
    "fresh-agent" \
    "v1" \
    "cli"
)"

# Step 1: health — verify runtime envelope
current_step_id="$(
  scenario_record_step_id \
    "${runtime_root}" \
    "${output_dir}/step01-record.json" \
    "${run_id}" \
    "health" \
    "command" \
    "pengu-mesh health"
)"
health_ms="$(run_pengu_json "${runtime_root}" "${output_dir}/health.json" health)"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step01-latency.json" "${run_id}" "${current_step_id}" "health" "${health_ms}"
health_ok="$(json_path_value "${output_dir}/health.json" "ok")"
if ! record_equals_assertion "${runtime_root}" "${output_dir}/step01-assert-ok.json" "${run_id}" "${current_step_id}" "health ok" "true" "${health_ok}" "health should return an ok envelope from a fresh runtime root"; then
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step01-finish.json" "${current_step_id}" "failed" "health_not_ok"
  current_step_id=""
  exit 1
fi
scenario_finish_step_event "${runtime_root}" "${output_dir}/step01-finish.json" "${current_step_id}" "passed"
current_step_id=""

# Step 2: diagnose — verify readiness report schema contract
current_step_id="$(
  scenario_record_step_id \
    "${runtime_root}" \
    "${output_dir}/step02-record.json" \
    "${run_id}" \
    "diagnose" \
    "command" \
    "pengu-mesh diagnose"
)"
diagnose_ms="$(run_pengu_json "${runtime_root}" "${output_dir}/diagnose.json" diagnose)"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step02-latency.json" "${run_id}" "${current_step_id}" "diagnose" "${diagnose_ms}"
diagnose_schema_version="$(json_path_value "${output_dir}/diagnose.json" "data.schema_version")"
if ! record_equals_assertion "${runtime_root}" "${output_dir}/step02-assert-schema.json" "${run_id}" "${current_step_id}" "diagnose schema_version" "diagnose.v1" "${diagnose_schema_version}" "diagnose should preserve the current schema contract in a fresh state"; then
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step02-finish.json" "${current_step_id}" "failed" "diagnose_schema_version"
  current_step_id=""
  exit 1
fi
scenario_finish_step_event "${runtime_root}" "${output_dir}/step02-finish.json" "${current_step_id}" "passed"
current_step_id=""

# Step 3: doctor --json — verify operator truth
current_step_id="$(
  scenario_record_step_id \
    "${runtime_root}" \
    "${output_dir}/step03-record.json" \
    "${run_id}" \
    "doctor" \
    "command" \
    "pengu-mesh-doctor -- --json"
)"
doctor_ms="$(run_doctor_json "${runtime_root}" "${output_dir}/doctor.json")"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step03-latency.json" "${run_id}" "${current_step_id}" "doctor" "${doctor_ms}"
doctor_ok="$(json_path_value "${output_dir}/doctor.json" "ok")"
if ! record_equals_assertion "${runtime_root}" "${output_dir}/step03-assert-ok.json" "${run_id}" "${current_step_id}" "doctor ok" "true" "${doctor_ok}" "doctor should return ok from a fresh runtime root"; then
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step03-finish.json" "${current_step_id}" "failed" "doctor_not_ok"
  current_step_id=""
  exit 1
fi
scenario_finish_step_event "${runtime_root}" "${output_dir}/step03-finish.json" "${current_step_id}" "passed"
current_step_id=""

# Step 4: host-access-status — check permissions
current_step_id="$(
  scenario_record_step_id \
    "${runtime_root}" \
    "${output_dir}/step04-record.json" \
    "${run_id}" \
    "host-access-status" \
    "command" \
    "pengu-mesh host-access-status"
)"
host_access_ms="$(run_pengu_json "${runtime_root}" "${output_dir}/host-access-status.json" host-access-status)"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step04-latency.json" "${run_id}" "${current_step_id}" "host-access-status" "${host_access_ms}"
host_access_ok="$(json_path_value "${output_dir}/host-access-status.json" "ok")"
if ! record_equals_assertion "${runtime_root}" "${output_dir}/step04-assert-ok.json" "${run_id}" "${current_step_id}" "host-access-status ok" "true" "${host_access_ok}" "host-access-status should return ok from a fresh runtime root"; then
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step04-finish.json" "${current_step_id}" "failed" "host_access_not_ok"
  current_step_id=""
  exit 1
fi
scenario_finish_step_event "${runtime_root}" "${output_dir}/step04-finish.json" "${current_step_id}" "passed"
current_step_id=""

# Step 5: profile-create --name fresh-test — create a managed profile
current_step_id="$(
  scenario_record_step_id \
    "${runtime_root}" \
    "${output_dir}/step05-record.json" \
    "${run_id}" \
    "profile-create" \
    "command" \
    "pengu-mesh profile-create --name fresh-test"
)"
profile_create_ms="$(run_pengu_json "${runtime_root}" "${output_dir}/profile-create.json" profile-create --name fresh-test)"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step05-latency.json" "${run_id}" "${current_step_id}" "profile-create" "${profile_create_ms}"
profile_create_ok="$(json_path_value "${output_dir}/profile-create.json" "ok")"
if ! record_equals_assertion "${runtime_root}" "${output_dir}/step05-assert-ok.json" "${run_id}" "${current_step_id}" "profile-create ok" "true" "${profile_create_ok}" "profile-create should succeed from a fresh runtime root"; then
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step05-finish.json" "${current_step_id}" "failed" "profile_create"
  current_step_id=""
  exit 1
fi
scenario_finish_step_event "${runtime_root}" "${output_dir}/step05-finish.json" "${current_step_id}" "passed"
current_step_id=""

# Step 6: profile-list — verify profile appears
current_step_id="$(
  scenario_record_step_id \
    "${runtime_root}" \
    "${output_dir}/step06-record.json" \
    "${run_id}" \
    "profile-list" \
    "command" \
    "pengu-mesh profile-list"
)"
profile_list_ms="$(run_pengu_json "${runtime_root}" "${output_dir}/profile-list.json" profile-list)"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step06-latency.json" "${run_id}" "${current_step_id}" "profile-list" "${profile_list_ms}"
profile_list_ok="$(json_path_value "${output_dir}/profile-list.json" "ok")"
if ! record_equals_assertion "${runtime_root}" "${output_dir}/step06-assert-ok.json" "${run_id}" "${current_step_id}" "profile-list ok" "true" "${profile_list_ok}" "profile-list should return ok"; then
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step06-finish.json" "${current_step_id}" "failed" "profile_list_not_ok"
  current_step_id=""
  exit 1
fi
profile_count="$(json_path_length "${output_dir}/profile-list.json" "data")"
if ! record_positive_integer_assertion "${runtime_root}" "${output_dir}/step06-assert-count.json" "${run_id}" "${current_step_id}" "profile-list non-empty" "${profile_count}" "profile-list should contain the freshly created profile"; then
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step06-finish.json" "${current_step_id}" "failed" "profile_list_empty"
  current_step_id=""
  exit 1
fi
scenario_finish_step_event "${runtime_root}" "${output_dir}/step06-finish.json" "${current_step_id}" "passed"
current_step_id=""

# Step 7: instance-start — start browser
current_step_id="$(
  scenario_record_step_id \
    "${runtime_root}" \
    "${output_dir}/step07-record.json" \
    "${run_id}" \
    "instance-start" \
    "command" \
    "pengu-mesh instance-start --name fresh-session --channel chrome-dev --headless --holder-id scenario-agent"
)"
instance_start_ms="$(run_pengu_json "${runtime_root}" "${output_dir}/instance-start.json" instance-start --name fresh-session --channel chrome-dev --headless --holder-id scenario-agent)"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step07-latency.json" "${run_id}" "${current_step_id}" "instance-start" "${instance_start_ms}"
instance_start_ok="$(json_path_value "${output_dir}/instance-start.json" "ok")"
if ! record_equals_assertion "${runtime_root}" "${output_dir}/step07-assert-ok.json" "${run_id}" "${current_step_id}" "instance-start ok" "true" "${instance_start_ok}" "instance-start should launch Chrome Dev headless from a fresh state"; then
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step07-finish.json" "${current_step_id}" "failed" "instance_start"
  current_step_id=""
  exit 1
fi
instance_id="$(json_data_field "${output_dir}/instance-start.json" "id")"
scenario_finish_step_event "${runtime_root}" "${output_dir}/step07-finish.json" "${current_step_id}" "passed"
current_step_id=""

# Step 8: instance-list — verify instance appears
current_step_id="$(
  scenario_record_step_id \
    "${runtime_root}" \
    "${output_dir}/step08-record.json" \
    "${run_id}" \
    "instance-list" \
    "command" \
    "pengu-mesh instance-list"
)"
instance_list_ms="$(run_pengu_json "${runtime_root}" "${output_dir}/instance-list.json" instance-list)"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step08-latency.json" "${run_id}" "${current_step_id}" "instance-list" "${instance_list_ms}"
instance_list_ok="$(json_path_value "${output_dir}/instance-list.json" "ok")"
if ! record_equals_assertion "${runtime_root}" "${output_dir}/step08-assert-ok.json" "${run_id}" "${current_step_id}" "instance-list ok" "true" "${instance_list_ok}" "instance-list should return ok"; then
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step08-finish.json" "${current_step_id}" "failed" "instance_list_not_ok"
  current_step_id=""
  exit 1
fi
instance_count="$(json_path_length "${output_dir}/instance-list.json" "data")"
if ! record_positive_integer_assertion "${runtime_root}" "${output_dir}/step08-assert-count.json" "${run_id}" "${current_step_id}" "instance-list non-empty" "${instance_count}" "instance-list should contain the freshly started instance"; then
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step08-finish.json" "${current_step_id}" "failed" "instance_list_empty"
  current_step_id=""
  exit 1
fi
scenario_finish_step_event "${runtime_root}" "${output_dir}/step08-finish.json" "${current_step_id}" "passed"
current_step_id=""

# Step 9: tab-list — verify tab inventory
current_step_id="$(
  scenario_record_step_id \
    "${runtime_root}" \
    "${output_dir}/step09-record.json" \
    "${run_id}" \
    "tab-list" \
    "command" \
    "pengu-mesh tab-list --instance-id ${instance_id} --holder-id scenario-agent"
)"
tab_list_ms="$(run_pengu_json "${runtime_root}" "${output_dir}/tab-list.json" tab-list --instance-id "${instance_id}" --holder-id scenario-agent)"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step09-latency.json" "${run_id}" "${current_step_id}" "tab-list" "${tab_list_ms}"
tab_list_ok="$(json_path_value "${output_dir}/tab-list.json" "ok")"
if ! record_equals_assertion "${runtime_root}" "${output_dir}/step09-assert-ok.json" "${run_id}" "${current_step_id}" "tab-list ok" "true" "${tab_list_ok}" "tab-list should return ok for the running instance"; then
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step09-finish.json" "${current_step_id}" "failed" "tab_list_not_ok"
  current_step_id=""
  exit 1
fi
tab_count="$(json_path_length "${output_dir}/tab-list.json" "data")"
if ! record_positive_integer_assertion "${runtime_root}" "${output_dir}/step09-assert-count.json" "${run_id}" "${current_step_id}" "tab-list non-empty" "${tab_count}" "tab-list should contain at least the initial browser tab"; then
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step09-finish.json" "${current_step_id}" "failed" "tab_list_empty"
  current_step_id=""
  exit 1
fi
scenario_finish_step_event "${runtime_root}" "${output_dir}/step09-finish.json" "${current_step_id}" "passed"
current_step_id=""

# Step 10: instance-stop — clean shutdown
current_step_id="$(
  scenario_record_step_id \
    "${runtime_root}" \
    "${output_dir}/step10-record.json" \
    "${run_id}" \
    "instance-stop" \
    "command" \
    "pengu-mesh instance-stop --instance-id ${instance_id} --holder-id scenario-agent"
)"
instance_stop_ms="$(run_pengu_json "${runtime_root}" "${output_dir}/instance-stop.json" instance-stop --instance-id "${instance_id}" --holder-id scenario-agent)"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step10-latency.json" "${run_id}" "${current_step_id}" "instance-stop" "${instance_stop_ms}"
instance_stop_ok="$(json_path_value "${output_dir}/instance-stop.json" "ok")"
if ! record_equals_assertion "${runtime_root}" "${output_dir}/step10-assert-ok.json" "${run_id}" "${current_step_id}" "instance-stop ok" "true" "${instance_stop_ok}" "instance-stop should close the managed browser cleanly"; then
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step10-finish.json" "${current_step_id}" "failed" "instance_stop"
  current_step_id=""
  exit 1
fi
scenario_finish_step_event "${runtime_root}" "${output_dir}/step10-finish.json" "${current_step_id}" "passed"
current_step_id=""
instance_id=""

write_summary "passed"
scenario_finish_run_event "${runtime_root}" "${output_dir}/scenario-finish-run.json" "${run_id}" "passed" "${summary_path}"
run_finished=1

print "${run_id}"
