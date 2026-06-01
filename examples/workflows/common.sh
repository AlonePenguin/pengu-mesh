#!/bin/zsh

if [[ -x "${HOME}/.cargo/bin/cargo" ]]; then
  cargo_bin="${HOME}/.cargo/bin/cargo"
else
  cargo_bin="$(command -v cargo)"
fi

now_ns() {
  /usr/bin/python3 - <<'PY'
import time
print(time.time_ns())
PY
}

elapsed_ms() {
  /usr/bin/python3 - "$1" "$2" <<'PY'
import sys

start_ns = int(sys.argv[1])
end_ns = int(sys.argv[2])
print(f"{(end_ns - start_ns) / 1_000_000:.3f}")
PY
}

run_pengu_json() {
  local runtime_root="$1"
  local output_path="$2"
  shift 2

  local started_ns ended_ns
  started_ns="$(now_ns)"
  PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
    "${cargo_bin}" run --quiet -p pengu-mesh -- "$@" \
    > "${output_path}" \
    2> "${output_path%.json}.stderr.log"
  ended_ns="$(now_ns)"
  elapsed_ms "${started_ns}" "${ended_ns}"
}

run_doctor_json() {
  local runtime_root="$1"
  local output_path="$2"
  local started_ns ended_ns
  started_ns="$(now_ns)"
  PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
    "${cargo_bin}" run --quiet -p pengu-mesh-doctor -- --json \
    > "${output_path}" \
    2> "${output_path%.json}.stderr.log"
  ended_ns="$(now_ns)"
  elapsed_ms "${started_ns}" "${ended_ns}"
}

json_data_field() {
  /usr/bin/python3 - "$1" "$2" <<'PY'
import json
import sys

path, field = sys.argv[1:3]
with open(path, "r", encoding="utf-8") as handle:
    payload = json.load(handle)
value = payload["data"][field]
if value is None:
    print("")
elif isinstance(value, bool):
    print("true" if value else "false")
else:
    print(value)
PY
}

json_path_value() {
  /usr/bin/python3 - "$1" "$2" <<'PY'
import json
import sys

path, dotted_path = sys.argv[1:3]
with open(path, "r", encoding="utf-8") as handle:
    payload = json.load(handle)

value = payload
for part in dotted_path.split("."):
    if not part:
        continue
    if isinstance(value, list):
        value = value[int(part)]
    else:
        value = value[part]

if value is None:
    print("")
elif isinstance(value, bool):
    print("true" if value else "false")
elif isinstance(value, (dict, list)):
    print(json.dumps(value, sort_keys=True, separators=(",", ":")))
else:
    print(value)
PY
}

json_path_length() {
  /usr/bin/python3 - "$1" "$2" <<'PY'
import json
import sys

path, dotted_path = sys.argv[1:3]
with open(path, "r", encoding="utf-8") as handle:
    payload = json.load(handle)

value = payload
for part in dotted_path.split("."):
    if not part:
        continue
    if isinstance(value, list):
        value = value[int(part)]
    else:
        value = value[part]

print(len(value))
PY
}

scenario_record_run_id() {
  local runtime_root="$1"
  local output_path="$2"
  local name="$3"
  local family="$4"
  local version="$5"
  local surface="$6"

  run_pengu_json "${runtime_root}" "${output_path}" \
    scenario-record-run \
    --name "${name}" \
    --family "${family}" \
    --version "${version}" \
    --surface "${surface}" >/dev/null
  json_data_field "${output_path}" "run_id"
}

scenario_record_step_id() {
  local runtime_root="$1"
  local output_path="$2"
  local run_id="$3"
  local name="$4"
  local kind="$5"
  local command_line="${6:-}"

  if [[ -n "${command_line}" ]]; then
    run_pengu_json "${runtime_root}" "${output_path}" \
      scenario-record-step \
      --run-id "${run_id}" \
      --name "${name}" \
      --kind "${kind}" \
      --command-line "${command_line}" >/dev/null
  else
    run_pengu_json "${runtime_root}" "${output_path}" \
      scenario-record-step \
      --run-id "${run_id}" \
      --name "${name}" \
      --kind "${kind}" >/dev/null
  fi
  json_data_field "${output_path}" "step_id"
}

scenario_record_assertion_event() {
  local runtime_root="$1"
  local output_path="$2"
  local run_id="$3"
  local step_id="$4"
  local name="$5"
  local expected="$6"
  local actual="$7"
  local assertion_status="$8"
  local failure_category="${9:-}"
  local notes="${10:-}"

  local args=(
    scenario-record-assertion
    --run-id "${run_id}"
    --name "${name}"
    --expected "${expected}"
    --actual "${actual}"
    --status "${assertion_status}"
  )
  if [[ -n "${step_id}" ]]; then
    args+=(--step-id "${step_id}")
  fi
  if [[ -n "${failure_category}" ]]; then
    args+=(--failure-category "${failure_category}")
  fi
  if [[ -n "${notes}" ]]; then
    args+=(--notes "${notes}")
  fi
  run_pengu_json "${runtime_root}" "${output_path}" "${args[@]}" >/dev/null
}

scenario_record_latency_event() {
  local runtime_root="$1"
  local output_path="$2"
  local run_id="$3"
  local step_id="$4"
  local metric="$5"
  local sample_ms="$6"

  local args=(
    scenario-record-latency
    --run-id "${run_id}"
    --metric "${metric}"
    --sample-ms "${sample_ms}"
  )
  if [[ -n "${step_id}" ]]; then
    args+=(--step-id "${step_id}")
  fi
  run_pengu_json "${runtime_root}" "${output_path}" "${args[@]}" >/dev/null
}

scenario_finish_step_event() {
  local runtime_root="$1"
  local output_path="$2"
  local step_id="$3"
  local step_status="$4"
  local error_code="${5:-}"

  local args=(
    scenario-finish-step
    --step-id "${step_id}"
    --status "${step_status}"
  )
  if [[ -n "${error_code}" ]]; then
    args+=(--error-code "${error_code}")
  fi
  run_pengu_json "${runtime_root}" "${output_path}" "${args[@]}" >/dev/null
}

scenario_finish_run_event() {
  local runtime_root="$1"
  local output_path="$2"
  local run_id="$3"
  local run_status="$4"
  local summary_path="${5:-}"

  local args=(
    scenario-finish-run
    --run-id "${run_id}"
    --status "${run_status}"
  )
  if [[ -n "${summary_path}" ]]; then
    args+=(--summary-path "${summary_path}")
  fi
  run_pengu_json "${runtime_root}" "${output_path}" "${args[@]}" >/dev/null
}

record_equals_assertion() {
  local runtime_root="$1"
  local output_path="$2"
  local run_id="$3"
  local step_id="$4"
  local name="$5"
  local expected="$6"
  local actual="$7"
  local notes="${8:-}"
  local failure_category="${9:-mismatch}"

  local assertion_status="failed"
  local category="${failure_category}"
  if [[ "${expected}" == "${actual}" ]]; then
    assertion_status="passed"
    category=""
  fi

  scenario_record_assertion_event \
    "${runtime_root}" \
    "${output_path}" \
    "${run_id}" \
    "${step_id}" \
    "${name}" \
    "${expected}" \
    "${actual}" \
    "${assertion_status}" \
    "${category}" \
    "${notes}"

  [[ "${assertion_status}" == "passed" ]]
}

record_contains_assertion() {
  local runtime_root="$1"
  local output_path="$2"
  local run_id="$3"
  local step_id="$4"
  local name="$5"
  local needle="$6"
  local haystack="$7"
  local notes="${8:-}"

  local assertion_status="failed"
  local category="missing_substring"
  if [[ "${haystack}" == *"${needle}"* ]]; then
    assertion_status="passed"
    category=""
  fi

  scenario_record_assertion_event \
    "${runtime_root}" \
    "${output_path}" \
    "${run_id}" \
    "${step_id}" \
    "${name}" \
    "${needle}" \
    "${haystack}" \
    "${assertion_status}" \
    "${category}" \
    "${notes}"

  [[ "${assertion_status}" == "passed" ]]
}

record_positive_integer_assertion() {
  local runtime_root="$1"
  local output_path="$2"
  local run_id="$3"
  local step_id="$4"
  local name="$5"
  local actual="$6"
  local notes="${7:-}"

  local assertion_status="failed"
  local category="not_positive"
  if [[ "${actual}" =~ ^[0-9]+$ ]] && (( actual > 0 )); then
    assertion_status="passed"
    category=""
  fi

  scenario_record_assertion_event \
    "${runtime_root}" \
    "${output_path}" \
    "${run_id}" \
    "${step_id}" \
    "${name}" \
    ">0" \
    "${actual}" \
    "${assertion_status}" \
    "${category}" \
    "${notes}"

  [[ "${assertion_status}" == "passed" ]]
}

html_data_url_from_stdin() {
  /usr/bin/python3 -c 'import base64, sys; html = sys.stdin.read(); print("data:text/html;base64," + base64.b64encode(html.encode("utf-8")).decode("ascii"))'
}
