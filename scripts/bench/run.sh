#!/bin/zsh
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$repo_root"

timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
output_dir="${1:-reports/audit/${timestamp}_benchmarks}"
mkdir -p "$output_dir"

if [[ -x "${HOME}/.cargo/bin/cargo" ]]; then
  cargo_bin="${HOME}/.cargo/bin/cargo"
else
  cargo_bin="$(command -v cargo)"
fi

run_bench() {
  local package="$1"
  local bench_name="$2"
  local output_name="$3"
  echo "running ${package}/${bench_name}"
  "${cargo_bin}" bench -p "${package}" --bench "${bench_name}" \
    > "${output_dir}/${output_name}" 2>&1
}

./scripts/bench/discover.sh > "${output_dir}/bench-discover.txt"
{
  echo "created_at=${timestamp}"
  uname -a
  "${cargo_bin}" -V
  rustc -Vv
} > "${output_dir}/benchmark_env.txt"

run_bench "pengu-mesh-bench-json" "json_envelope" "json-bench.txt"
run_bench "pengu-mesh-bench-cdp" "cdp_target_parse" "cdp-bench.txt"
run_bench "pengu-mesh-bench-persistence" "persistence_state_serialization" "persistence-bench.txt"
run_bench "pengu-mesh-bench-artifacts" "artifacts_write_artifact" "artifacts-bench.txt"

cat > "${output_dir}/summary.md" <<EOF
# Benchmark Run

- created_at: ${timestamp}
- output_dir: ${output_dir}
- discover: bench-discover.txt
- environment: benchmark_env.txt
- json: json-bench.txt
- cdp: cdp-bench.txt
- persistence: persistence-bench.txt
- artifacts: artifacts-bench.txt
EOF

printf '%s\n' "${output_dir}"
