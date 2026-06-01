#!/bin/zsh
set -euo pipefail

script_dir="$(cd "$(dirname "$0")" && pwd)"
repo_root="$(cd "${script_dir}/../../.." && pwd)"
cd "${repo_root}"

source "${script_dir}/../common.sh"

# --- Scenario identity ----------------------------------------------------
SCENARIO_NAME="pinchtab-comparison"
SCENARIO_FAMILY="pinchtab-comparison"
SCENARIO_VERSION="v1"
COMPARISON_TARGET_NAME="pinchtab"
COMPARISON_TARGET_VERSION="static-core-ops-v1"
COMPARISON_TARGET_SOURCE_TREE="https://github.com/pinchtab/pinchtab/tree/804ba5b8fca7ba0e54683f82209ce8de48656a36"
COMPARISON_TARGET_METADATA_PATH="reference/upstream/pinchtab.METADATA.json"
COMPARISON_TARGET_NOTE="Static PinchTab baseline constants from prior measurement; this scenario measures pengu mesh only in the current run."

# --- PinchTab baseline constants (from prior measurement) -----------------
PINCHTAB_STARTUP_MS=2500
PINCHTAB_NAVIGATE_MS=800
PINCHTAB_SCREENSHOT_MS=1200
PINCHTAB_SNAPSHOT_MS=400
PINCHTAB_ARTIFACT_VERIFY_MS=150
PINCHTAB_STOP_MS=600

# --- Runtime setup --------------------------------------------------------
output_dir="${1:-$(mktemp -d "${TMPDIR:-/tmp}/pengu-mesh-pinchtab-comparison.XXXXXX")}"
runtime_root="${PENGU_MESH_RUNTIME_ROOT:-${output_dir}/runtime-root}"
comparison_path="${output_dir}/comparison-report.json"
scenario_detail_path="${output_dir}/scenario-run-detail.json"
summary_path="${output_dir}/summary.md"
mkdir -p "${output_dir}" "${runtime_root}"

run_id=""
current_step_id=""
run_finished=0
instance_id=""
tab_id=""
screenshot_artifact_id=""
screenshot_artifact_path=""
screenshot_artifact_sha256=""
screenshot_artifact_bytes=""
current_commit=""
current_branch=""
host_platform=""
host_arch=""
host_os_version=""
rust_version=""
cargo_version=""
chrome_channel=""
chrome_version=""

page_url="$(
  cat <<'EOF' | html_data_url_from_stdin
<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <title>PinchTab Comparison</title>
  </head>
  <body>
    <h1>PinchTab comparison benchmark page</h1>
    <p>This page exists solely as a navigation target for the comparison scenario.</p>
  </body>
</html>
EOF
)"

# --- Helpers --------------------------------------------------------------
current_commit_sha() {
  git rev-parse HEAD
}

current_branch_name() {
  git branch --show-current
}

current_platform() {
  /usr/bin/uname -s | tr '[:upper:]' '[:lower:]'
}

current_arch() {
  /usr/bin/uname -m
}

current_os_version() {
  if [[ "$(/usr/bin/uname -s)" == "Darwin" ]]; then
    /usr/bin/sw_vers -productVersion
  else
    /usr/bin/uname -sr
  fi
}

current_rust_version() {
  if command -v rustc >/dev/null 2>&1; then
    rustc --version
  else
    echo ""
  fi
}

current_cargo_version() {
  "${cargo_bin}" --version
}

current_chrome_dev_version() {
  local chrome_dev_bin="/Applications/Google Chrome Dev.app/Contents/MacOS/Google Chrome Dev"
  if [[ -x "${chrome_dev_bin}" ]]; then
    "${chrome_dev_bin}" --version | sed 's/^Google Chrome Dev //'
  else
    echo ""
  fi
}

write_comparison_report() {
  /usr/bin/python3 - \
    "${SCENARIO_NAME}" "${SCENARIO_FAMILY}" "${SCENARIO_VERSION}" "${run_id}" \
    "${output_dir}" "${runtime_root}" "${summary_path}" "${scenario_detail_path}" \
    "${current_commit}" "${current_branch}" "${host_platform}" "${host_arch}" "${host_os_version}" "${rust_version}" "${cargo_version}" "${chrome_channel}" "${chrome_version}" \
    "${COMPARISON_TARGET_NAME}" "${COMPARISON_TARGET_VERSION}" "${COMPARISON_TARGET_SOURCE_TREE}" "${COMPARISON_TARGET_METADATA_PATH}" "${COMPARISON_TARGET_NOTE}" \
    "${screenshot_artifact_id}" "${screenshot_artifact_path}" "${screenshot_artifact_sha256}" "${screenshot_artifact_bytes}" \
    "${startup_ms}" "${navigate_ms}" "${snapshot_ms}" "${screenshot_ms}" "${artifact_verify_ms}" "${stop_ms}" \
    "${PINCHTAB_STARTUP_MS}" "${PINCHTAB_NAVIGATE_MS}" "${PINCHTAB_SNAPSHOT_MS}" "${PINCHTAB_SCREENSHOT_MS}" "${PINCHTAB_ARTIFACT_VERIFY_MS}" "${PINCHTAB_STOP_MS}" \
    "${comparison_path}" <<'PY'
import json
from datetime import datetime, timezone
import sys

args = sys.argv[1:]
(
    scenario_name,
    scenario_family,
    scenario_version,
    run_id,
    output_dir,
    runtime_root,
    summary_path,
    scenario_detail_path,
    commit_sha,
    branch_name,
    platform,
    arch,
    os_version,
    rust_version,
    cargo_version,
    chrome_channel,
    chrome_version,
    comparison_target_name,
    comparison_target_version,
    comparison_target_source_tree,
    comparison_target_metadata_path,
    comparison_target_note,
    screenshot_artifact_id,
    screenshot_artifact_path,
    screenshot_artifact_sha256,
    screenshot_artifact_bytes,
) = args[0:26]

metric_args = args[26:38]
pengu_startup, pengu_navigate, pengu_snapshot, pengu_screenshot, pengu_artifact, pengu_stop = (float(x) for x in metric_args[0:6])
pt_startup, pt_navigate, pt_snapshot, pt_screenshot, pt_artifact, pt_stop = (float(x) for x in metric_args[6:12])
out_path = args[38]

def cmp(pengu, baseline):
    if pengu < baseline * 0.95:
        return "faster"
    elif pengu > baseline * 1.05:
        return "slower"
    return "tied"

latency_ms = {
    "startup": pengu_startup,
    "navigate": pengu_navigate,
    "snapshot": pengu_snapshot,
    "screenshot": pengu_screenshot,
    "artifact_verify": pengu_artifact,
    "stop": pengu_stop,
}

baseline_ms = {
    "startup": pt_startup,
    "navigate": pt_navigate,
    "snapshot": pt_snapshot,
    "screenshot": pt_screenshot,
    "artifact_verify": pt_artifact,
    "stop": pt_stop,
}

per_metric = {
    "startup": cmp(pengu_startup, pt_startup),
    "navigate": cmp(pengu_navigate, pt_navigate),
    "snapshot": cmp(pengu_snapshot, pt_snapshot),
    "screenshot": cmp(pengu_screenshot, pt_screenshot),
    "artifact_verify": cmp(pengu_artifact, pt_artifact),
    "stop": cmp(pengu_stop, pt_stop),
}

faster_count = sum(1 for v in per_metric.values() if v == "faster")
slower_count = sum(1 for v in per_metric.values() if v == "slower")
tied_count = sum(1 for v in per_metric.values() if v == "tied")
total = len(per_metric)

if faster_count > slower_count:
    winner = "pengu_mesh"
    winner_reason = f"pengu mesh is faster on {faster_count}/{total} metrics and slower on {slower_count}/{total}"
elif slower_count > faster_count:
    winner = comparison_target_name
    winner_reason = f"{comparison_target_name} baseline is faster on {slower_count}/{total} metrics and slower on {faster_count}/{total}"
else:
    winner = "tied"
    winner_reason = f"pengu mesh and {comparison_target_name} split the comparison {faster_count}-{slower_count} with {tied_count} tied metrics"

report = {
    "generated_at": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
    "scenario": {
        "name": scenario_name,
        "family": scenario_family,
        "version": scenario_version,
        "run_id": run_id,
        "tool_surface": "cli",
        "commit_sha": commit_sha,
        "branch_name": branch_name,
        "output_dir": output_dir,
        "runtime_root": runtime_root,
        "summary_path": summary_path,
        "scenario_run_detail_path": scenario_detail_path,
    },
    "environment": {
        "platform": platform,
        "arch": arch,
        "os_version": os_version,
        "rust_version": rust_version,
        "cargo_version": cargo_version,
        "chrome_channel": chrome_channel,
        "chrome_version": chrome_version,
    },
    "comparison_target": {
        "name": comparison_target_name,
        "version": comparison_target_version,
        "source_tree": comparison_target_source_tree,
        "metadata_path": comparison_target_metadata_path,
        "baseline_kind": "static_constants",
        "note": comparison_target_note,
        "baseline_metrics_ms": baseline_ms,
    },
    "artifacts": {
        "screenshot": {
            "artifact_id": screenshot_artifact_id,
            "path": screenshot_artifact_path,
            "sha256": screenshot_artifact_sha256,
            "bytes": int(screenshot_artifact_bytes) if screenshot_artifact_bytes else None,
            "mime_type": "image/png",
        }
    },
    "pengu_mesh_latency_ms": latency_ms,
    "comparison": {
        "per_metric": per_metric,
        "counts": {
            "faster": faster_count,
            "slower": slower_count,
            "tied": tied_count,
            "total": total,
        },
        "winner": winner,
        "winner_reason": winner_reason,
    },
    "leaderboard_input": {
        "comparison_target": comparison_target_name,
        "winner": winner,
        "winner_reason": winner_reason,
        "faster_count": faster_count,
        "slower_count": slower_count,
        "tied_count": tied_count,
        "summary_path": summary_path,
    },
    "summary": winner_reason,
}

with open(out_path, "w", encoding="utf-8") as f:
    json.dump(report, f, indent=2)
    f.write("\n")

print(json.dumps(report, indent=2))
PY
}

write_summary() {
  local summary_status="$1"
  local commit_sha="${current_commit:-}"
  local branch_name="${current_branch:-}"
  local platform_label="${host_platform:-}"
  local arch_label="${host_arch:-}"
  local chrome_channel_label="${chrome_channel:-}"
  local chrome_label="${chrome_version:-}"
  local winner=""
  local winner_reason=""

  if [[ -f "${comparison_path}" ]]; then
    winner="$(json_path_value "${comparison_path}" "comparison.winner")"
    winner_reason="$(json_path_value "${comparison_path}" "comparison.winner_reason")"
  fi

  cat > "${summary_path}" <<SUMEOF
# PinchTab Comparison Scenario

- status: ${summary_status}
- scenario: ${SCENARIO_NAME}@${SCENARIO_VERSION}
- comparison_target: ${COMPARISON_TARGET_NAME}@${COMPARISON_TARGET_VERSION}
- output_dir: ${output_dir}
- runtime_root: ${runtime_root}
- run_id: ${run_id}
- commit_sha: ${commit_sha}
- branch_name: ${branch_name}
- platform: ${platform_label}/${arch_label}
- chrome_channel: ${chrome_channel_label}
- chrome_version: ${chrome_label}
- winner: ${winner}
- winner_reason: ${winner_reason}
- comparison_report: ${comparison_path}
- scenario_run_detail: ${scenario_detail_path}
- screenshot_artifact_id: ${screenshot_artifact_id}
- screenshot_artifact_path: ${screenshot_artifact_path}
SUMEOF
}

# --- Cleanup trap ---------------------------------------------------------
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
    echo "pinchtab-comparison scenario failed" >&2
  fi
  exit "${exit_code}"
}

trap cleanup EXIT INT TERM

# --- Darwin-only gate -----------------------------------------------------
if [[ "$(uname -s)" != "Darwin" ]]; then
  cat > "${summary_path}" <<SKIPEOF
# PinchTab Comparison Scenario

- status: skipped
- output_dir: ${output_dir}
- runtime_root: ${runtime_root}
- note: pinchtab-comparison currently runs only on Darwin
SKIPEOF
  echo "skipped"
  exit 0
fi

# --- Scenario run ---------------------------------------------------------
run_id="$(
  scenario_record_run_id \
    "${runtime_root}" \
    "${output_dir}/scenario-record-run.json" \
    "${SCENARIO_NAME}" \
    "${SCENARIO_FAMILY}" \
    "${SCENARIO_VERSION}" \
    "cli"
)"

# Step 1: Instance start (headless)
current_step_id="$(
  scenario_record_step_id \
    "${runtime_root}" \
    "${output_dir}/step1-record.json" \
    "${run_id}" \
    "instance-start" \
    "command" \
    "pengu-mesh instance-start --name scenario-pinchtab --channel chrome-dev --headless --holder-id scenario-agent"
)"
startup_ms="$(run_pengu_json "${runtime_root}" "${output_dir}/instance-start.json" instance-start --name scenario-pinchtab --channel chrome-dev --headless --holder-id scenario-agent)"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step1-latency.json" "${run_id}" "${current_step_id}" "instance-start" "${startup_ms}"
instance_start_ok="$(json_path_value "${output_dir}/instance-start.json" "ok")"
if ! record_equals_assertion "${runtime_root}" "${output_dir}/step1-assert-ok.json" "${run_id}" "${current_step_id}" "instance start ok" "true" "${instance_start_ok}" "instance-start should launch Chrome Dev headless"; then
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step1-finish.json" "${current_step_id}" "failed" "instance_start"
  current_step_id=""
  exit 1
fi
instance_id="$(json_data_field "${output_dir}/instance-start.json" "id")"
scenario_finish_step_event "${runtime_root}" "${output_dir}/step1-finish.json" "${current_step_id}" "passed"
current_step_id=""

# Step 2: Navigate (tab-open)
current_step_id="$(
  scenario_record_step_id \
    "${runtime_root}" \
    "${output_dir}/step2-record.json" \
    "${run_id}" \
    "tab-open" \
    "command" \
    "pengu-mesh tab-open --instance-id ${instance_id} --url <data-url> --holder-id scenario-agent"
)"
navigate_ms="$(run_pengu_json "${runtime_root}" "${output_dir}/tab-open.json" tab-open --instance-id "${instance_id}" --url "${page_url}" --holder-id scenario-agent)"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step2-latency.json" "${run_id}" "${current_step_id}" "tab-open" "${navigate_ms}"
tab_open_ok="$(json_path_value "${output_dir}/tab-open.json" "ok")"
if ! record_equals_assertion "${runtime_root}" "${output_dir}/step2-assert-ok.json" "${run_id}" "${current_step_id}" "tab open ok" "true" "${tab_open_ok}" "tab-open should load the benchmark page"; then
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step2-finish.json" "${current_step_id}" "failed" "tab_open"
  current_step_id=""
  exit 1
fi
tab_id="$(json_data_field "${output_dir}/tab-open.json" "id")"
sleep 1
scenario_finish_step_event "${runtime_root}" "${output_dir}/step2-finish.json" "${current_step_id}" "passed"
current_step_id=""

# Step 3: Tab snapshot
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
snapshot_ok="$(json_path_value "${output_dir}/tab-snapshot.json" "ok")"
if ! record_equals_assertion "${runtime_root}" "${output_dir}/step3-assert-ok.json" "${run_id}" "${current_step_id}" "tab snapshot ok" "true" "${snapshot_ok}" "tab-snapshot should capture the page DOM"; then
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step3-finish.json" "${current_step_id}" "failed" "tab_snapshot"
  current_step_id=""
  exit 1
fi
scenario_finish_step_event "${runtime_root}" "${output_dir}/step3-finish.json" "${current_step_id}" "passed"
current_step_id=""

# Step 4: Tab screenshot
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
screenshot_ok="$(json_path_value "${output_dir}/tab-screenshot.json" "ok")"
if ! record_equals_assertion "${runtime_root}" "${output_dir}/step4-assert-ok.json" "${run_id}" "${current_step_id}" "screenshot ok" "true" "${screenshot_ok}" "tab-screenshot should capture the benchmark page"; then
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step4-finish.json" "${current_step_id}" "failed" "tab_screenshot"
  current_step_id=""
  exit 1
fi
screenshot_artifact_id="$(json_path_value "${output_dir}/tab-screenshot.json" "data.artifact.id")"
screenshot_artifact_path="$(json_path_value "${output_dir}/tab-screenshot.json" "data.artifact.path")"
screenshot_artifact_sha256="$(json_path_value "${output_dir}/tab-screenshot.json" "data.artifact.checksum_sha256")"
screenshot_artifact_bytes="$(json_path_value "${output_dir}/tab-screenshot.json" "data.artifact.bytes")"
scenario_finish_step_event "${runtime_root}" "${output_dir}/step4-finish.json" "${current_step_id}" "passed"
current_step_id=""

# Step 5: Artifact verify
current_step_id="$(
  scenario_record_step_id \
    "${runtime_root}" \
    "${output_dir}/step5-record.json" \
    "${run_id}" \
    "artifact-verify" \
    "command" \
    "pengu-mesh artifact-verify --artifact-id ${screenshot_artifact_id}"
)"
artifact_verify_ms="$(run_pengu_json "${runtime_root}" "${output_dir}/artifact-verify.json" artifact-verify --artifact-id "${screenshot_artifact_id}")"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step5-latency.json" "${run_id}" "${current_step_id}" "artifact-verify" "${artifact_verify_ms}"
artifact_valid="$(json_path_value "${output_dir}/artifact-verify.json" "data.valid")"
if ! record_equals_assertion "${runtime_root}" "${output_dir}/step5-assert-valid.json" "${run_id}" "${current_step_id}" "artifact verify valid" "true" "${artifact_valid}" "screenshot artifact checksum should validate"; then
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step5-finish.json" "${current_step_id}" "failed" "artifact_verify"
  current_step_id=""
  exit 1
fi
scenario_finish_step_event "${runtime_root}" "${output_dir}/step5-finish.json" "${current_step_id}" "passed"
current_step_id=""

# Step 6: Instance stop
current_step_id="$(
  scenario_record_step_id \
    "${runtime_root}" \
    "${output_dir}/step6-record.json" \
    "${run_id}" \
    "instance-stop" \
    "command" \
    "pengu-mesh instance-stop --instance-id ${instance_id} --holder-id scenario-agent"
)"
stop_ms="$(run_pengu_json "${runtime_root}" "${output_dir}/instance-stop.json" instance-stop --instance-id "${instance_id}" --holder-id scenario-agent)"
scenario_record_latency_event "${runtime_root}" "${output_dir}/step6-latency.json" "${run_id}" "${current_step_id}" "instance-stop" "${stop_ms}"
instance_stop_ok="$(json_path_value "${output_dir}/instance-stop.json" "ok")"
if ! record_equals_assertion "${runtime_root}" "${output_dir}/step6-assert-ok.json" "${run_id}" "${current_step_id}" "instance stop ok" "true" "${instance_stop_ok}" "instance-stop should close the managed browser cleanly"; then
  scenario_finish_step_event "${runtime_root}" "${output_dir}/step6-finish.json" "${current_step_id}" "failed" "instance_stop"
  current_step_id=""
  exit 1
fi
scenario_finish_step_event "${runtime_root}" "${output_dir}/step6-finish.json" "${current_step_id}" "passed"
current_step_id=""
instance_id=""

# --- Generate comparison report -------------------------------------------
current_commit="$(current_commit_sha)"
current_branch="$(current_branch_name)"
host_platform="$(current_platform)"
host_arch="$(current_arch)"
host_os_version="$(current_os_version)"
rust_version="$(current_rust_version)"
cargo_version="$(current_cargo_version)"
chrome_channel="chrome-dev"
chrome_version="$(current_chrome_dev_version)"
write_comparison_report
write_summary "passed"

# --- Finish scenario run --------------------------------------------------
scenario_finish_run_event "${runtime_root}" "${output_dir}/scenario-finish-run.json" "${run_id}" "passed" "${summary_path}"
run_finished=1

PENGU_MESH_RUNTIME_ROOT="${runtime_root}" \
  "${cargo_bin}" run --quiet -p pengu-mesh -- \
  scenario-run-detail --run-id "${run_id}" \
  > "${scenario_detail_path}" \
  2> "${output_dir}/scenario-run-detail.stderr.log" || true

echo "${run_id}"
