#!/bin/zsh
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$repo_root"

if [[ -x "${HOME}/.cargo/bin/cargo" ]]; then
  cargo_bin="${HOME}/.cargo/bin/cargo"
else
  cargo_bin="$(command -v cargo)"
fi

timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
output_dir="${1:-reports/audit/${timestamp}_local_gate}"
gate_runtime_root="${output_dir}/gate-runtime-root"
mkdir -p "$output_dir" "$gate_runtime_root"

run_step() {
  local name="$1"
  shift
  echo "running ${name}"
  "$@" > "${output_dir}/${name}.txt" 2>&1
}

run_step cargo-fmt "${cargo_bin}" fmt --all --check
run_step cargo-check "${cargo_bin}" check --workspace
run_step cargo-test "${cargo_bin}" test --workspace
run_step bench-discover ./scripts/bench/discover.sh
run_step bench-compile "${cargo_bin}" bench --workspace --no-run
run_step bench-threshold-check ./scripts/bench/threshold-check.sh
run_step lease-smoke /bin/zsh ./scripts/release/lease-smoke.sh "${output_dir}/lease-smoke"
run_step continuity-smoke /bin/zsh ./scripts/release/continuity-smoke.sh "${output_dir}/continuity-smoke"
run_step attach-continuity-smoke /bin/zsh ./scripts/release/attach-continuity-smoke.sh "${output_dir}/attach-continuity-smoke"
run_step diagnose-smoke /bin/zsh ./scripts/release/diagnose-smoke.sh "${output_dir}/diagnose-smoke"
run_step host-access-smoke /bin/zsh ./scripts/release/host-access-smoke.sh "${output_dir}/host-access-smoke"
run_step browser-lifecycle-integration /bin/zsh ./scripts/release/browser-lifecycle-integration.sh "${output_dir}/browser-lifecycle-integration"
run_step tab-lifecycle-integration /bin/zsh ./scripts/release/tab-lifecycle-integration.sh "${output_dir}/tab-lifecycle-integration"
run_step evidence-chain-smoke /bin/zsh ./scripts/release/evidence-chain-smoke.sh "${output_dir}/evidence-chain-smoke"
run_step browser-surface-smoke /bin/zsh ./scripts/release/browser-surface-smoke.sh "${output_dir}/browser-surface-smoke"
echo "running startup-readiness-scenario"
PENGU_MESH_RUNTIME_ROOT="${gate_runtime_root}" \
  /bin/zsh ./examples/workflows/startup-readiness/run.sh "${output_dir}/startup-readiness-scenario" \
  > "${output_dir}/startup-readiness-scenario.txt" \
  2> "${output_dir}/startup-readiness-scenario.stderr.log"

startup_run_id="$(tr -d '\r\n' < "${output_dir}/startup-readiness-scenario.txt")"
PENGU_MESH_RUNTIME_ROOT="${gate_runtime_root}" \
  "${cargo_bin}" run -p pengu-mesh -- scenario-run-detail --run-id "${startup_run_id}" \
  > "${output_dir}/startup-readiness-scenario-detail.json" \
  2> "${output_dir}/startup-readiness-scenario-detail.stderr.log"

/usr/bin/python3 - "${output_dir}/startup-readiness-scenario-detail.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
run = payload["data"]["run"]
if run["scenario_family"] != "startup-readiness":
    raise SystemExit(f"expected startup-readiness family, got {run['scenario_family']}")
if run["status"] != "passed":
    raise SystemExit(f"expected startup-readiness status passed, got {run['status']}")
PY

PENGU_MESH_RUNTIME_ROOT="${gate_runtime_root}" \
  "${cargo_bin}" run -p pengu-mesh -- health > "${output_dir}/pengu-mesh-health.json" 2> "${output_dir}/pengu-mesh-health.stderr.log"

PENGU_MESH_RUNTIME_ROOT="${gate_runtime_root}" \
  "${cargo_bin}" run -p pengu-mesh-doctor -- --json > "${output_dir}/pengu-mesh-doctor.json" 2> "${output_dir}/pengu-mesh-doctor.stderr.log"

PENGU_MESH_RUNTIME_ROOT="${gate_runtime_root}" \
  "${cargo_bin}" run -p pengu-mesh -- scenario-list --limit 10 > "${output_dir}/scenario-list.json" 2> "${output_dir}/scenario-list.stderr.log"

/usr/bin/python3 - "${timestamp}" "${repo_root}" > "${output_dir}/gate-metadata.json" <<'PY'
import json
import sys

timestamp, repo_root = sys.argv[1:3]
print(json.dumps({"created_at": timestamp, "repo_root": repo_root}, indent=2))
PY

cat > "${output_dir}/summary.md" <<EOF
# Local Production Gate

- created_at: ${timestamp}
- output_dir: ${output_dir}
- checks:
  - cargo fmt --all --check
  - cargo check --workspace
  - cargo test --workspace
  - bench discovery
  - bench compile
  - bench threshold manifest check
  - lease coexistence and conflict smoke
  - daemon restart continuity smoke
  - attach continuity restart and stale-reclaim smoke
  - diagnose CLI/MCP/HTTP smoke
  - host access capability and setup smoke
  - headless browser lifecycle integration smoke
  - headless tab lifecycle integration smoke
  - evidence chain integrity and corruption smoke
  - browser surface native-control smoke
  - startup-readiness scenario smoke with persisted scenario detail
  - pengu-mesh health
  - pengu-mesh doctor
  - pengu-mesh scenario-list
EOF

printf '%s\n' "${output_dir}"
