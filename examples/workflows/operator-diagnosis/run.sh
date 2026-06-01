#!/bin/zsh
set -euo pipefail

script_dir="$(cd "$(dirname "$0")" && pwd)"
repo_root="$(cd "${script_dir}/../../.." && pwd)"
cd "${repo_root}"

source "${script_dir}/../common.sh"

output_dir="${1:-$(mktemp -d "${TMPDIR:-/tmp}/pengu-mesh-operator-diagnosis.XXXXXX")}"
runtime_root="${PENGU_MESH_RUNTIME_ROOT:-${output_dir}/runtime-root}"
summary_path="${output_dir}/summary.md"
mkdir -p "${output_dir}" "${runtime_root}"

run_id=""
current_step_id=""
run_finished=0
passed=true
probes=()
diagnose_ms=""
health_ms=""
doctor_ms=""
host_access_ms=""
lease_ms=""
diagnose_ok=""
diagnose_state=""
health_ok=""
doctor_ok=""
host_access_ok=""
host_platform=""
lease_ok=""

write_summary() {
  local summary_status="$1"
  cat > "${summary_path}" <<EOF
# Operator Diagnosis Scenario

- status: ${summary_status}
- output_dir: ${output_dir}
- runtime_root: ${runtime_root}
- run_id: ${run_id}
- diagnose_ok: ${diagnose_ok}
- diagnose_state: ${diagnose_state}
- health_ok: ${health_ok}
- doctor_ok: ${doctor_ok}
- host_access_ok: ${host_access_ok}
- host_platform: ${host_platform}
- lease_ok: ${lease_ok}
- diagnose_ms: ${diagnose_ms}
- health_ms: ${health_ms}
- doctor_ms: ${doctor_ms}
- host_access_ms: ${host_access_ms}
- lease_ms: ${lease_ms}
- diagnose_path: ${output_dir}/diagnose.json
- health_path: ${output_dir}/health.json
- doctor_path: ${output_dir}/doctor.json
- host_access_status_path: ${output_dir}/host-access-status.json
- lease_status_path: ${output_dir}/lease-status.json
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
    print -u2 "operator-diagnosis scenario failed"
  fi
  exit "${exit_code}"
}

trap cleanup EXIT INT TERM

# ---------------------------------------------------------------------------
# Record the scenario run
# ---------------------------------------------------------------------------

run_id="$(
  scenario_record_run_id \
    "${runtime_root}" \
    "${output_dir}/scenario-record-run.json" \
    "operator-diagnosis" \
    "operator-diagnosis" \
    "v1" \
    "cli"
)"

# ---------------------------------------------------------------------------
# Step 1: diagnose — full readiness report
# ---------------------------------------------------------------------------

current_step_id="$(
  scenario_record_step_id \
    "${runtime_root}" \
    "${output_dir}/step1-record.json" \
    "${run_id}" \
    "diagnose" \
    "command" \
    "pengu-mesh diagnose"
)"
diagnose_ms="$(run_pengu_json "${runtime_root}" "${output_dir}/diagnose.json" diagnose)"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step1-latency.json" "${run_id}" "${current_step_id}" "diagnose" "${diagnose_ms}"

diagnose_ok="$(json_path_value "${output_dir}/diagnose.json" "ok")"
diagnose_schema_version="$(json_path_value "${output_dir}/diagnose.json" "data.schema_version")"
diagnose_state="$(json_path_value "${output_dir}/diagnose.json" "data.state")"
permissions_len="$(json_path_length "${output_dir}/diagnose.json" "data.permissions")"
services_len="$(json_path_length "${output_dir}/diagnose.json" "data.services")"
capabilities_len="$(json_path_length "${output_dir}/diagnose.json" "data.capabilities")"
remediations_len="$(json_path_length "${output_dir}/diagnose.json" "data.remediations")"

step1_ok=true

if ! record_equals_assertion "${runtime_root}" "${output_dir}/step1-assert-ok.json" "${run_id}" "${current_step_id}" "diagnose ok" "true" "${diagnose_ok}" "diagnose should return ok true"; then
  step1_ok=false
fi

if ! record_positive_integer_assertion "${runtime_root}" "${output_dir}/step1-assert-schema.json" "${run_id}" "${current_step_id}" "diagnose schema_version present" "${#diagnose_schema_version}" "diagnose should include a schema_version field"; then
  step1_ok=false
fi

diagnose_state_status="failed"
if [[ "${diagnose_state}" == "ready" || "${diagnose_state}" == "degraded" ]]; then
  diagnose_state_status="passed"
fi
scenario_record_assertion_event \
  "${runtime_root}" \
  "${output_dir}/step1-assert-state.json" \
  "${run_id}" \
  "${current_step_id}" \
  "diagnose state ready or degraded" \
  "ready|degraded" \
  "${diagnose_state}" \
  "${diagnose_state_status}" \
  "$( [[ "${diagnose_state_status}" == "passed" ]] && echo "" || echo "unexpected_state" )" \
  "diagnose state should be actionable"
if [[ "${diagnose_state_status}" != "passed" ]]; then
  step1_ok=false
fi

for arr_name in permissions services capabilities remediations; do
  local_len_var="${arr_name}_len"
  # remediations may legitimately be empty, so only assert non-empty for the others
  if [[ "${arr_name}" != "remediations" ]]; then
    if ! record_positive_integer_assertion "${runtime_root}" "${output_dir}/step1-assert-${arr_name}.json" "${run_id}" "${current_step_id}" "diagnose ${arr_name} non-empty" "${(P)local_len_var}" "diagnose should include at least one ${arr_name} entry"; then
      step1_ok=false
    fi
  else
    # For remediations just assert it is an array (length >= 0 is fine)
    scenario_record_assertion_event \
      "${runtime_root}" \
      "${output_dir}/step1-assert-${arr_name}.json" \
      "${run_id}" \
      "${current_step_id}" \
      "diagnose ${arr_name} is array" \
      ">=0" \
      "${(P)local_len_var}" \
      "passed" \
      "" \
      "remediations array should be present"
  fi
done

if [[ "${step1_ok}" == "true" ]]; then
  probes+=("\"diagnose\"")
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step1-finish.json" "${current_step_id}" "passed"
else
  passed=false
  probes+=("\"diagnose:FAIL\"")
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step1-finish.json" "${current_step_id}" "failed" "diagnose_assertions"
fi
current_step_id=""

# ---------------------------------------------------------------------------
# Step 2: health — lightweight health envelope
# ---------------------------------------------------------------------------

current_step_id="$(
  scenario_record_step_id \
    "${runtime_root}" \
    "${output_dir}/step2-record.json" \
    "${run_id}" \
    "health" \
    "command" \
    "pengu-mesh health"
)"
health_ms="$(run_pengu_json "${runtime_root}" "${output_dir}/health.json" health)"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step2-latency.json" "${run_id}" "${current_step_id}" "health" "${health_ms}"

health_ok="$(json_path_value "${output_dir}/health.json" "ok")"
health_data="$(json_path_value "${output_dir}/health.json" "data")"

step2_ok=true

if ! record_equals_assertion "${runtime_root}" "${output_dir}/step2-assert-ok.json" "${run_id}" "${current_step_id}" "health ok" "true" "${health_ok}" "health should return ok true"; then
  step2_ok=false
fi

if ! record_positive_integer_assertion "${runtime_root}" "${output_dir}/step2-assert-data.json" "${run_id}" "${current_step_id}" "health data present" "${#health_data}" "health should include a data field"; then
  step2_ok=false
fi

if [[ "${step2_ok}" == "true" ]]; then
  probes+=("\"health\"")
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step2-finish.json" "${current_step_id}" "passed"
else
  passed=false
  probes+=("\"health:FAIL\"")
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step2-finish.json" "${current_step_id}" "failed" "health_assertions"
fi
current_step_id=""

# ---------------------------------------------------------------------------
# Step 3: doctor --json via pengu-mesh-doctor
# ---------------------------------------------------------------------------

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

doctor_ok="$(json_path_value "${output_dir}/doctor.json" "ok")"

step3_ok=true

if ! record_equals_assertion "${runtime_root}" "${output_dir}/step3-assert-ok.json" "${run_id}" "${current_step_id}" "doctor ok" "true" "${doctor_ok}" "doctor should return ok true"; then
  step3_ok=false
fi

if [[ "${step3_ok}" == "true" ]]; then
  probes+=("\"doctor\"")
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step3-finish.json" "${current_step_id}" "passed"
else
  passed=false
  probes+=("\"doctor:FAIL\"")
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step3-finish.json" "${current_step_id}" "failed" "doctor_assertions"
fi
current_step_id=""

# ---------------------------------------------------------------------------
# Step 4: host-access-status — platform capability matrix
# ---------------------------------------------------------------------------

current_step_id="$(
  scenario_record_step_id \
    "${runtime_root}" \
    "${output_dir}/step4-record.json" \
    "${run_id}" \
    "host-access-status" \
    "command" \
    "pengu-mesh host-access-status"
)"
host_access_ms="$(run_pengu_json "${runtime_root}" "${output_dir}/host-access-status.json" host-access-status)"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step4-latency.json" "${run_id}" "${current_step_id}" "host-access-status" "${host_access_ms}"

host_access_ok="$(json_path_value "${output_dir}/host-access-status.json" "ok")"
host_platform="$(json_path_value "${output_dir}/host-access-status.json" "data.platform")"
host_services_len="$(json_path_length "${output_dir}/host-access-status.json" "data.services")"

step4_ok=true

if ! record_equals_assertion "${runtime_root}" "${output_dir}/step4-assert-ok.json" "${run_id}" "${current_step_id}" "host-access-status ok" "true" "${host_access_ok}" "host-access-status should return ok true"; then
  step4_ok=false
fi

if ! record_positive_integer_assertion "${runtime_root}" "${output_dir}/step4-assert-platform.json" "${run_id}" "${current_step_id}" "host-access-status platform present" "${#host_platform}" "host-access-status should include a platform field"; then
  step4_ok=false
fi

if ! record_positive_integer_assertion "${runtime_root}" "${output_dir}/step4-assert-services.json" "${run_id}" "${current_step_id}" "host-access-status services non-empty" "${host_services_len}" "host-access-status should enumerate services"; then
  step4_ok=false
fi

if [[ "${step4_ok}" == "true" ]]; then
  probes+=("\"host-access-status\"")
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step4-finish.json" "${current_step_id}" "passed"
else
  passed=false
  probes+=("\"host-access-status:FAIL\"")
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step4-finish.json" "${current_step_id}" "failed" "host_access_assertions"
fi
current_step_id=""

# ---------------------------------------------------------------------------
# Step 5: lease-status — lease state
# ---------------------------------------------------------------------------

current_step_id="$(
  scenario_record_step_id \
    "${runtime_root}" \
    "${output_dir}/step5-record.json" \
    "${run_id}" \
    "lease-status" \
    "command" \
    "pengu-mesh lease-status"
)"
lease_ms="$(run_pengu_json "${runtime_root}" "${output_dir}/lease-status.json" lease-status)"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step5-latency.json" "${run_id}" "${current_step_id}" "lease-status" "${lease_ms}"

lease_ok="$(json_path_value "${output_dir}/lease-status.json" "ok")"

step5_ok=true

if ! record_equals_assertion "${runtime_root}" "${output_dir}/step5-assert-ok.json" "${run_id}" "${current_step_id}" "lease-status ok" "true" "${lease_ok}" "lease-status should return ok true"; then
  step5_ok=false
fi

if [[ "${step5_ok}" == "true" ]]; then
  probes+=("\"lease-status\"")
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step5-finish.json" "${current_step_id}" "passed"
else
  passed=false
  probes+=("\"lease-status:FAIL\"")
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step5-finish.json" "${current_step_id}" "failed" "lease_assertions"
fi
current_step_id=""

# ---------------------------------------------------------------------------
# Step 6: cross-validation — diagnose state vs health ok
# ---------------------------------------------------------------------------

current_step_id="$(
  scenario_record_step_id \
    "${runtime_root}" \
    "${output_dir}/step6-record.json" \
    "${run_id}" \
    "cross-validation" \
    "assertion" \
    "diagnose.state consistent with health.ok"
)"

# A ready/degraded diagnose state should pair with health ok=true.
cross_ok=true
expected_health_ok="false"
if [[ "${diagnose_state}" == "ready" || "${diagnose_state}" == "degraded" ]]; then
  expected_health_ok="true"
fi

if ! record_equals_assertion "${runtime_root}" "${output_dir}/step6-assert-cross.json" "${run_id}" "${current_step_id}" "diagnose state consistent with health ok" "${expected_health_ok}" "${health_ok}" "diagnose state ${diagnose_state} should be consistent with health ok ${health_ok}"; then
  cross_ok=false
fi

if [[ "${cross_ok}" == "true" ]]; then
  probes+=("\"cross-validation\"")
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step6-finish.json" "${current_step_id}" "passed"
else
  passed=false
  probes+=("\"cross-validation:FAIL\"")
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step6-finish.json" "${current_step_id}" "failed" "cross_validation"
fi
current_step_id=""

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------

write_summary "$( [[ "${passed}" == "true" ]] && echo "passed" || echo "failed" )"
probes_json="[$(IFS=,; print -r -- "${probes[*]}")]"
print -r -- "{ \"family\": \"operator-diagnosis\", \"passed\": ${passed}, \"run_id\": \"${run_id}\", \"summary_path\": \"${summary_path}\", \"output_dir\": \"${output_dir}\", \"probes\": ${probes_json} }"

scenario_finish_run_event \
  "${runtime_root}" \
  "${output_dir}/scenario-finish-run.json" \
  "${run_id}" \
  "$( [[ "${passed}" == "true" ]] && echo "passed" || echo "failed" )" \
  "${summary_path}"
run_finished=1
