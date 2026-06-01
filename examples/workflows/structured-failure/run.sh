#!/bin/zsh
set -euo pipefail

script_dir="$(cd "$(dirname "$0")" && pwd)"
repo_root="$(cd "${script_dir}/../../.." && pwd)"
cd "${repo_root}"

source "${script_dir}/../common.sh"

output_dir="${1:-$(mktemp -d "${TMPDIR:-/tmp}/pengu-mesh-structured-failure.XXXXXX")}"
runtime_root="${PENGU_MESH_RUNTIME_ROOT:-${output_dir}/runtime-root}"
summary_path="${output_dir}/summary.md"
mkdir -p "${output_dir}" "${runtime_root}"

run_id=""
current_step_id=""
current_step_prefix=""
run_finished=0
missing_instance_path="${output_dir}/missing-instance.json"
missing_tab_path="${output_dir}/missing-tab.json"
missing_artifact_path="${output_dir}/missing-artifact.json"
missing_run_path="${output_dir}/missing-run.json"
external_attach_path="${output_dir}/external-attach-disabled.json"
duplicate_profile_path="${output_dir}/duplicate-profile.json"

write_summary() {
  local summary_status="$1"
  cat > "${summary_path}" <<EOF
# Structured Failure Scenario

- status: ${summary_status}
- output_dir: ${output_dir}
- runtime_root: ${runtime_root}
- run_id: ${run_id}
- missing_instance_path: ${missing_instance_path}
- missing_tab_path: ${missing_tab_path}
- missing_artifact_path: ${missing_artifact_path}
- missing_run_path: ${missing_run_path}
- external_attach_path: ${external_attach_path}
- duplicate_profile_path: ${duplicate_profile_path}
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
    current_step_prefix=""
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
    print -u2 "structured-failure scenario failed"
  fi
  exit "${exit_code}"
}

trap cleanup EXIT INT TERM

start_step() {
  local prefix="$1"
  local name="$2"
  local command_line="$3"

  current_step_prefix="${prefix}"
  current_step_id="$(
    scenario_record_step_id \
      "${runtime_root}" \
      "${output_dir}/${prefix}-record.json" \
      "${run_id}" \
      "${name}" \
      "command" \
      "${command_line}"
  )"
}

finish_step_passed() {
  scenario_finish_step_event \
    "${runtime_root}" \
    "${output_dir}/${current_step_prefix}-finish.json" \
    "${current_step_id}" \
    "passed"
  current_step_id=""
  current_step_prefix=""
}

finish_step_failed() {
  local error_code="$1"
  scenario_finish_step_event \
    "${runtime_root}" \
    "${output_dir}/${current_step_prefix}-finish.json" \
    "${current_step_id}" \
    "failed" \
    "${error_code}"
  current_step_id=""
  current_step_prefix=""
  exit 1
}

assert_failure_contract() {
  local prefix="$1"
  local payload_path="$2"
  local expected_code="$3"
  local expected_operation="$4"
  local expected_reason="$5"
  local expected_recovery="$6"
  shift 6
  local attempted_markers=("$@")

  local ok_value code_value operation_value reason_value recovery_json recovery_count retry_value attempted_json
  ok_value="$(json_path_value "${payload_path}" "ok")"
  code_value="$(json_path_value "${payload_path}" "code")"
  operation_value="$(json_path_value "${payload_path}" "data.operation")"
  reason_value="$(json_path_value "${payload_path}" "data.reason")"
  recovery_json="$(json_path_value "${payload_path}" "data.recovery")"
  recovery_count="$(json_path_length "${payload_path}" "data.recovery")"
  retry_value="$(json_path_value "${payload_path}" "data.retry_likely")"
  attempted_json="$(json_path_value "${payload_path}" "data.attempted")"

  if ! record_equals_assertion "${runtime_root}" "${output_dir}/${prefix}-assert-ok.json" "${run_id}" "${current_step_id}" "${prefix} ok false" "false" "${ok_value}" "structured failure probes must return ok=false"; then
    return 1
  fi
  if ! record_equals_assertion "${runtime_root}" "${output_dir}/${prefix}-assert-code.json" "${run_id}" "${current_step_id}" "${prefix} code" "${expected_code}" "${code_value}" "failure code should match the classified envelope"; then
    return 1
  fi
  if ! record_equals_assertion "${runtime_root}" "${output_dir}/${prefix}-assert-operation.json" "${run_id}" "${current_step_id}" "${prefix} operation" "${expected_operation}" "${operation_value}" "operation should identify the failing surface"; then
    return 1
  fi
  if ! record_contains_assertion "${runtime_root}" "${output_dir}/${prefix}-assert-reason.json" "${run_id}" "${current_step_id}" "${prefix} reason" "${expected_reason}" "${reason_value}" "reason should stay actionable and specific"; then
    return 1
  fi
  if ! record_positive_integer_assertion "${runtime_root}" "${output_dir}/${prefix}-assert-recovery-count.json" "${run_id}" "${current_step_id}" "${prefix} recovery non-empty" "${recovery_count}" "recovery guidance must not be empty"; then
    return 1
  fi
  if ! record_contains_assertion "${runtime_root}" "${output_dir}/${prefix}-assert-recovery.json" "${run_id}" "${current_step_id}" "${prefix} recovery guidance" "${expected_recovery}" "${recovery_json}" "recovery guidance should include the expected remediation"; then
    return 1
  fi
  if ! record_equals_assertion "${runtime_root}" "${output_dir}/${prefix}-assert-retry.json" "${run_id}" "${current_step_id}" "${prefix} retry likely false" "false" "${retry_value}" "the named failure probes should all be non-retriable without operator action"; then
    return 1
  fi

  local marker_index=1
  local marker
  for marker in "${attempted_markers[@]}"; do
    if ! record_contains_assertion "${runtime_root}" "${output_dir}/${prefix}-assert-attempted-${marker_index}.json" "${run_id}" "${current_step_id}" "${prefix} attempted marker ${marker_index}" "${marker}" "${attempted_json}" "attempted should preserve the identifying request context"; then
      return 1
    fi
    marker_index=$((marker_index + 1))
  done

  return 0
}

run_id="$(
  scenario_record_run_id \
    "${runtime_root}" \
    "${output_dir}/scenario-record-run.json" \
    "structured-failure" \
    "structured-failure" \
    "v1" \
    "cli"
)"

start_step \
  "step1-missing-instance" \
  "missing-instance" \
  "pengu-mesh tab-list --instance-id inst_missing --holder-id scenario-agent"
missing_instance_ms="$(run_pengu_json "${runtime_root}" "${missing_instance_path}" tab-list --instance-id inst_missing --holder-id scenario-agent)"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step1-missing-instance-latency.json" "${run_id}" "${current_step_id}" "missing-instance" "${missing_instance_ms}"
if ! assert_failure_contract \
  "step1-missing-instance" \
  "${missing_instance_path}" \
  "not_found" \
  "tab list" \
  "unknown instance inst_missing" \
  "run pengu-mesh instance-list" \
  '"action_kind":"list"' \
  '"instance_id":"inst_missing"'; then
  finish_step_failed "missing_instance_contract"
fi
finish_step_passed

start_step \
  "step2-missing-tab" \
  "missing-tab" \
  "pengu-mesh tab-snapshot --tab-id tab_missing --holder-id scenario-agent"
missing_tab_ms="$(run_pengu_json "${runtime_root}" "${missing_tab_path}" tab-snapshot --tab-id tab_missing --holder-id scenario-agent)"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step2-missing-tab-latency.json" "${run_id}" "${current_step_id}" "missing-tab" "${missing_tab_ms}"
if ! assert_failure_contract \
  "step2-missing-tab" \
  "${missing_tab_path}" \
  "not_found" \
  "tab snapshot" \
  "unknown tab tab_missing" \
  "run pengu-mesh tab-list --instance-id <instance-id>" \
  '"action_kind":"snapshot"' \
  '"tab_id":"tab_missing"'; then
  finish_step_failed "missing_tab_contract"
fi
finish_step_passed

start_step \
  "step3-missing-artifact" \
  "missing-artifact" \
  "pengu-mesh artifact-verify --artifact-id artifact_missing"
missing_artifact_ms="$(run_pengu_json "${runtime_root}" "${missing_artifact_path}" artifact-verify --artifact-id artifact_missing)"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step3-missing-artifact-latency.json" "${run_id}" "${current_step_id}" "missing-artifact" "${missing_artifact_ms}"
if ! assert_failure_contract \
  "step3-missing-artifact" \
  "${missing_artifact_path}" \
  "not_found" \
  "artifact verify" \
  "unknown artifact artifact_missing" \
  "run pengu-mesh run-list --limit 25" \
  '"action_kind":"verify"' \
  '"artifact_id":"artifact_missing"'; then
  finish_step_failed "missing_artifact_contract"
fi
finish_step_passed

start_step \
  "step4-missing-run" \
  "missing-run" \
  "pengu-mesh scenario-run-detail --run-id scenario_run_missing"
missing_run_ms="$(run_pengu_json "${runtime_root}" "${missing_run_path}" scenario-run-detail --run-id scenario_run_missing)"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step4-missing-run-latency.json" "${run_id}" "${current_step_id}" "missing-run" "${missing_run_ms}"
if ! assert_failure_contract \
  "step4-missing-run" \
  "${missing_run_path}" \
  "not_found" \
  "scenario run detail" \
  "unknown scenario run scenario_run_missing" \
  "run pengu-mesh diagnose to inspect host and runtime readiness" \
  '"detail":"run_id=scenario_run_missing"' \
  '"operation":"scenario_run_detail"'; then
  finish_step_failed "missing_run_contract"
fi
finish_step_passed

start_step \
  "step5-external-attach-disabled" \
  "external-attach-disabled" \
  "pengu-mesh instance-attach --name attach-disabled --cdp-url ws://127.0.0.1:9222/devtools/browser/demo --holder-id scenario-agent"
external_attach_ms="$(run_pengu_json "${runtime_root}" "${external_attach_path}" instance-attach --name attach-disabled --cdp-url 'ws://127.0.0.1:9222/devtools/browser/demo' --holder-id scenario-agent)"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step5-external-attach-disabled-latency.json" "${run_id}" "${current_step_id}" "external-attach-disabled" "${external_attach_ms}"
if ! assert_failure_contract \
  "step5-external-attach-disabled" \
  "${external_attach_path}" \
  "misconfigured" \
  "instance attached" \
  "external attach is disabled by default; set PENGU_MESH_ALLOW_EXTERNAL_ATTACH=1 to enable it" \
  "set PENGU_MESH_ALLOW_EXTERNAL_ATTACH=1 before retrying instance-attach" \
  '"detail":"name=attach-disabled cdp_url=ws://127.0.0.1:9222/devtools/browser/demo"' \
  '"holder_id":"scenario-agent"' \
  '"operation":"instance_attach"'; then
  finish_step_failed "external_attach_disabled_contract"
fi
finish_step_passed

start_step \
  "step6-duplicate-profile" \
  "duplicate-profile" \
  "pengu-mesh profile-create --name duplicate-profile --channel chrome-dev; pengu-mesh profile-create --name duplicate-profile --channel chrome-dev"
profile_create_first_ms="$(run_pengu_json "${runtime_root}" "${output_dir}/profile-create-first.json" profile-create --name duplicate-profile --channel chrome-dev)"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step6-duplicate-profile-latency-first.json" "${run_id}" "${current_step_id}" "profile-create-first" "${profile_create_first_ms}"
profile_create_first_ok="$(json_path_value "${output_dir}/profile-create-first.json" "ok")"
if ! record_equals_assertion "${runtime_root}" "${output_dir}/step6-duplicate-profile-assert-seed.json" "${run_id}" "${current_step_id}" "duplicate profile seed create ok" "true" "${profile_create_first_ok}" "the first profile-create call must succeed before probing the duplicate conflict"; then
  finish_step_failed "duplicate_profile_seed"
fi
duplicate_profile_ms="$(run_pengu_json "${runtime_root}" "${duplicate_profile_path}" profile-create --name duplicate-profile --channel chrome-dev)"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step6-duplicate-profile-latency-second.json" "${run_id}" "${current_step_id}" "duplicate-profile" "${duplicate_profile_ms}"
if ! assert_failure_contract \
  "step6-duplicate-profile" \
  "${duplicate_profile_path}" \
  "conflict" \
  "profile created" \
  "managed profile prof_chrome_dev_duplicate_profile already exists" \
  "run pengu-mesh profile-list" \
  '"detail":"name=duplicate-profile channel=chrome-dev"' \
  '"operation":"profile_create"'; then
  finish_step_failed "duplicate_profile_contract"
fi
duplicate_recovery_json="$(json_path_value "${duplicate_profile_path}" "data.recovery")"
if ! record_contains_assertion "${runtime_root}" "${output_dir}/step6-duplicate-profile-assert-recovery-secondary.json" "${run_id}" "${current_step_id}" "duplicate profile secondary recovery" "retry profile-create with a different --name" "${duplicate_recovery_json}" "duplicate profile conflicts should direct the operator to pick a different name"; then
  finish_step_failed "duplicate_profile_recovery"
fi
finish_step_passed

write_summary "passed"
scenario_finish_run_event "${runtime_root}" "${output_dir}/scenario-finish-run.json" "${run_id}" "passed" "${summary_path}"
run_finished=1

print "${run_id}"
