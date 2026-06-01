#!/bin/zsh
set -euo pipefail

script_dir="$(cd "$(dirname "$0")" && pwd)"
repo_root="$(cd "${script_dir}/../../.." && pwd)"
cd "${repo_root}"

source "${script_dir}/../common.sh"

output_dir="${1:-$(mktemp -d "${TMPDIR:-/tmp}/pengu-mesh-weak-prompt.XXXXXX")}"
runtime_root="${output_dir}/runtime-root"
summary_path="${output_dir}/summary.md"
mkdir -p "${output_dir}" "${runtime_root}"

run_id=""
current_step_id=""
current_step_prefix=""
run_finished=0

probe_tab_list_actions_path="${output_dir}/probe-tab-list-actions.json"
probe_artifact_verify_path="${output_dir}/probe-artifact-verify.json"
probe_instance_attach_path="${output_dir}/probe-instance-attach.json"
probe_tab_action_path="${output_dir}/probe-tab-action.json"
probe_replay_export_path="${output_dir}/probe-replay-export.json"
probe_surface_action_path="${output_dir}/probe-surface-action.json"

write_summary() {
  local summary_status="$1"
  cat > "${summary_path}" <<EOF
# Weak Prompt Scenario

- status: ${summary_status}
- output_dir: ${output_dir}
- runtime_root: ${runtime_root}
- run_id: ${run_id}
- probe_tab_list_actions_path: ${probe_tab_list_actions_path}
- probe_artifact_verify_path: ${probe_artifact_verify_path}
- probe_instance_attach_path: ${probe_instance_attach_path}
- instance_attach_external_attach_override: true
- probe_tab_action_path: ${probe_tab_action_path}
- probe_replay_export_path: ${probe_replay_export_path}
- probe_surface_action_path: ${probe_surface_action_path}
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
    print -u2 "weak-prompt scenario failed"
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

run_pengu_json_allow_external_attach() {
  local runtime_root="$1"
  local output_path="$2"
  shift 2

  local started_ns ended_ns
  started_ns="$(now_ns)"
  PENGU_MESH_ALLOW_EXTERNAL_ATTACH=1 \
    PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
    "${cargo_bin}" run --quiet -p pengu-mesh -- "$@" \
    > "${output_path}" \
    2> "${output_path%.json}.stderr.log"
  ended_ns="$(now_ns)"
  elapsed_ms "${started_ns}" "${ended_ns}"
}

assert_weak_prompt_contract() {
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

  if ! record_equals_assertion "${runtime_root}" "${output_dir}/${prefix}-assert-ok.json" "${run_id}" "${current_step_id}" "${prefix} ok false" "false" "${ok_value}" "weak-prompt probes must return ok=false"; then
    return 1
  fi
  if ! record_equals_assertion "${runtime_root}" "${output_dir}/${prefix}-assert-code.json" "${run_id}" "${current_step_id}" "${prefix} code" "${expected_code}" "${code_value}" "failure code must match the expected classification"; then
    return 1
  fi
  if ! record_equals_assertion "${runtime_root}" "${output_dir}/${prefix}-assert-operation.json" "${run_id}" "${current_step_id}" "${prefix} operation" "${expected_operation}" "${operation_value}" "operation should preserve the failing surface"; then
    return 1
  fi
  if ! record_contains_assertion "${runtime_root}" "${output_dir}/${prefix}-assert-reason.json" "${run_id}" "${current_step_id}" "${prefix} reason" "${expected_reason}" "${reason_value}" "reason should explain the missing or malformed prompt context"; then
    return 1
  fi
  if ! record_positive_integer_assertion "${runtime_root}" "${output_dir}/${prefix}-assert-recovery-count.json" "${run_id}" "${current_step_id}" "${prefix} recovery non-empty" "${recovery_count}" "recovery guidance must not be empty for weak-prompt failures"; then
    return 1
  fi
  if ! record_contains_assertion "${runtime_root}" "${output_dir}/${prefix}-assert-recovery.json" "${run_id}" "${current_step_id}" "${prefix} recovery guidance" "${expected_recovery}" "${recovery_json}" "recovery guidance should point the next agent step in the right direction"; then
    return 1
  fi
  if ! record_equals_assertion "${runtime_root}" "${output_dir}/${prefix}-assert-retry.json" "${run_id}" "${current_step_id}" "${prefix} retry likely false" "false" "${retry_value}" "weak-prompt failures should not be marked retriable without fixing the prompt or context"; then
    return 1
  fi

  local marker_index=1
  local marker
  for marker in "${attempted_markers[@]}"; do
    if ! record_contains_assertion "${runtime_root}" "${output_dir}/${prefix}-assert-attempted-${marker_index}.json" "${run_id}" "${current_step_id}" "${prefix} attempted marker ${marker_index}" "${marker}" "${attempted_json}" "attempted should preserve the weak prompt context"; then
      return 1
    fi
    marker_index=$((marker_index + 1))
  done

  return 0
}

# ── register scenario run ────────────────────────────────────────────

run_id="$(
  scenario_record_run_id \
    "${runtime_root}" \
    "${output_dir}/scenario-record-run.json" \
    "weak-prompt" \
    "weak-prompt" \
    "v1" \
    "cli"
)"

# ── step 1: tab-list-actions with missing instance + tab ─────────────

start_step \
  "step1-tab-list-actions" \
  "tab-list-actions-missing" \
  "pengu-mesh tab-list-actions --instance-id inst_missing --tab-id tab_missing --holder-id scenario-agent"
probe_ms="$(run_pengu_json "${runtime_root}" "${probe_tab_list_actions_path}" tab-list-actions --instance-id inst_missing --tab-id tab_missing --holder-id scenario-agent)"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step1-tab-list-actions-latency.json" "${run_id}" "${current_step_id}" "tab-list-actions-missing" "${probe_ms}"
if ! assert_weak_prompt_contract \
  "step1-tab-list-actions" \
  "${probe_tab_list_actions_path}" \
  "not_found" \
  "tab action catalog" \
  "unknown instance inst_missing" \
  "run pengu-mesh instance-list" \
  '"action_kind":"list_actions"' \
  '"instance_id":"inst_missing"' \
  '"tab_id":"tab_missing"'; then
  finish_step_failed "tab_list_actions_contract"
fi
finish_step_passed

# ── step 2: artifact-verify with missing artifact ────────────────────

start_step \
  "step2-artifact-verify" \
  "artifact-verify-missing" \
  "pengu-mesh artifact-verify --artifact-id artifact_missing"
probe_ms="$(run_pengu_json "${runtime_root}" "${probe_artifact_verify_path}" artifact-verify --artifact-id artifact_missing)"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step2-artifact-verify-latency.json" "${run_id}" "${current_step_id}" "artifact-verify-missing" "${probe_ms}"
if ! assert_weak_prompt_contract \
  "step2-artifact-verify" \
  "${probe_artifact_verify_path}" \
  "not_found" \
  "artifact verify" \
  "unknown artifact artifact_missing" \
  "run pengu-mesh run-list --limit 25" \
  '"action_kind":"verify"' \
  '"artifact_id":"artifact_missing"'; then
  finish_step_failed "artifact_verify_contract"
fi
finish_step_passed

# ── step 3: instance-attach with invalid CDP URL ─────────────────────

start_step \
  "step3-instance-attach" \
  "instance-attach-bad-url" \
  "PENGU_MESH_ALLOW_EXTERNAL_ATTACH=1 pengu-mesh instance-attach --name bad --cdp-url not-a-url --holder-id scenario-agent"
probe_ms="$(run_pengu_json_allow_external_attach "${runtime_root}" "${probe_instance_attach_path}" instance-attach --name bad --cdp-url not-a-url --holder-id scenario-agent)"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step3-instance-attach-latency.json" "${run_id}" "${current_step_id}" "instance-attach-bad-url" "${probe_ms}"
if ! assert_weak_prompt_contract \
  "step3-instance-attach" \
  "${probe_instance_attach_path}" \
  "invalid_input" \
  "instance attached" \
  "parse cdp url" \
  "retry instance-attach with --cdp-url ws://127.0.0.1:<port>/devtools/browser/<id>" \
  '"detail":"name=bad cdp_url=not-a-url"' \
  '"holder_id":"scenario-agent"' \
  '"operation":"instance_attach"'; then
  finish_step_failed "instance_attach_contract"
fi
finish_step_passed

# ── step 4: tab-action navigate with missing tab ─────────────────────

start_step \
  "step4-tab-action" \
  "tab-action-missing" \
  "pengu-mesh tab-action --tab-id tab_missing --kind navigate --url http://example.com --holder-id scenario-agent"
probe_ms="$(run_pengu_json "${runtime_root}" "${probe_tab_action_path}" tab-action --tab-id tab_missing --kind navigate --url http://example.com --holder-id scenario-agent)"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step4-tab-action-latency.json" "${run_id}" "${current_step_id}" "tab-action-missing" "${probe_ms}"
if ! assert_weak_prompt_contract \
  "step4-tab-action" \
  "${probe_tab_action_path}" \
  "not_found" \
  "tab action completed" \
  "unknown tab tab_missing" \
  "run pengu-mesh tab-list --instance-id <instance-id>" \
  '"action_kind":"navigate"' \
  '"tab_id":"tab_missing"'; then
  finish_step_failed "tab_action_contract"
fi
finish_step_passed

# ── step 5: replay-export with missing run ───────────────────────────

start_step \
  "step5-replay-export" \
  "replay-export-missing" \
  "pengu-mesh replay-export --run-id run_missing"
probe_ms="$(run_pengu_json "${runtime_root}" "${probe_replay_export_path}" replay-export --run-id run_missing)"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step5-replay-export-latency.json" "${run_id}" "${current_step_id}" "replay-export-missing" "${probe_ms}"
if ! assert_weak_prompt_contract \
  "step5-replay-export" \
  "${probe_replay_export_path}" \
  "not_found" \
  "replay manifest exported" \
  "unknown run run_missing" \
  "run pengu-mesh run-list --limit 25" \
  '"detail":"run_id=run_missing mode=manifest_only"' \
  '"operation":"replay_export"'; then
  finish_step_failed "replay_export_contract"
fi
finish_step_passed

# ── step 6: browser-surface-action with missing instance + bad surface

start_step \
  "step6-surface-action" \
  "surface-action-missing" \
  "pengu-mesh browser-surface-action --instance-id inst_missing --surface-id ax:0/bad --action focus --holder-id scenario-agent"
probe_ms="$(run_pengu_json "${runtime_root}" "${probe_surface_action_path}" browser-surface-action --instance-id inst_missing --surface-id "ax:0/bad" --action focus --holder-id scenario-agent)"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step6-surface-action-latency.json" "${run_id}" "${current_step_id}" "surface-action-missing" "${probe_ms}"
if ! assert_weak_prompt_contract \
  "step6-surface-action" \
  "${probe_surface_action_path}" \
  "not_found" \
  "browser surface action completed" \
  "unknown instance inst_missing" \
  "run pengu-mesh instance-list" \
  '"action":"focus"' \
  '"instance_id":"inst_missing"' \
  '"surface_id":"ax:0/bad"'; then
  finish_step_failed "surface_action_contract"
fi
finish_step_passed

# ── finish ───────────────────────────────────────────────────────────

write_summary "passed"
scenario_finish_run_event "${runtime_root}" "${output_dir}/scenario-finish-run.json" "${run_id}" "passed" "${summary_path}"
run_finished=1

print "${run_id}"
