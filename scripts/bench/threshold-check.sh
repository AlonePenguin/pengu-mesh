#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$repo_root"

thresholds_file="benches/thresholds.json"

if [[ ! -f "$thresholds_file" ]]; then
  echo "error: thresholds file not found at ${thresholds_file}" >&2
  exit 1
fi

if [[ -x "${HOME}/.cargo/bin/cargo" ]]; then
  cargo_bin="${HOME}/.cargo/bin/cargo"
else
  cargo_bin="$(command -v cargo)"
fi

/usr/bin/python3 - "$cargo_bin" "$thresholds_file" <<'PY'
import json
import re
import subprocess
import sys


def duration_to_ns(raw_value: str, unit: str) -> int:
    scale = {
        "ns": 1,
        "us": 1_000,
        "µs": 1_000,
        "ms": 1_000_000,
        "s": 1_000_000_000,
    }
    return int(round(float(raw_value.replace(",", "")) * scale[unit]))


def parse_bench_line(output: str, benchmark: str) -> dict[str, int]:
    for line in output.splitlines():
        if benchmark not in line:
            continue
        match = re.search(
            r"([0-9.,]+)\s+(ns|us|µs|ms|s)\s+│\s+"
            r"([0-9.,]+)\s+(ns|us|µs|ms|s)\s+│\s+"
            r"([0-9.,]+)\s+(ns|us|µs|ms|s)\s+│\s+"
            r"([0-9.,]+)\s+(ns|us|µs|ms|s)",
            line,
        )
        if not match:
            continue
        return {
            "fastest": duration_to_ns(match.group(1), match.group(2)),
            "slowest": duration_to_ns(match.group(3), match.group(4)),
            "median": duration_to_ns(match.group(5), match.group(6)),
            "mean": duration_to_ns(match.group(7), match.group(8)),
        }
    raise ValueError(f"could not parse benchmark row for {benchmark!r}")


def main() -> int:
    cargo_bin, thresholds_path = sys.argv[1:3]
    with open(thresholds_path, "r", encoding="utf-8") as handle:
        manifest = json.load(handle)

    default_sample_count = str(manifest.get("sample_count", 10))
    default_min_time = str(manifest.get("min_time", 0.01))

    results = []
    overall_passed = True

    for entry in manifest["thresholds"]:
        package = entry["package"]
        bench = entry["bench"]
        benchmark = entry["benchmark"]
        statistic = entry.get("statistic", "median")
        max_ns = int(entry["max_ns"])
        description = entry["description"]
        sample_count = str(entry.get("sample_count", default_sample_count))
        min_time = str(entry.get("min_time", default_min_time))

        command = [
            cargo_bin,
            "bench",
            "-p",
            package,
            "--bench",
            bench,
            benchmark,
            "--",
            "--color",
            "never",
            "--sample-count",
            sample_count,
            "--min-time",
            min_time,
        ]
        completed = subprocess.run(command, capture_output=True, text=True)
        output = (completed.stdout or "") + (completed.stderr or "")

        result = {
            "package": package,
            "bench": bench,
            "benchmark": benchmark,
            "description": description,
            "statistic": statistic,
            "max_ns": max_ns,
            "command": command,
        }

        if completed.returncode != 0:
            overall_passed = False
            result["status"] = "error"
            result["reason"] = f"cargo bench exited with status {completed.returncode}"
            result["output_tail"] = output.splitlines()[-12:]
            results.append(result)
            continue

        try:
            statistics = parse_bench_line(output, benchmark)
        except ValueError as exc:
            overall_passed = False
            result["status"] = "error"
            result["reason"] = str(exc)
            result["output_tail"] = output.splitlines()[-12:]
            results.append(result)
            continue

        observed_ns = statistics[statistic]
        result["statistics_ns"] = statistics
        result["observed_ns"] = observed_ns
        result["status"] = "pass" if observed_ns <= max_ns else "fail"
        if result["status"] == "fail":
            overall_passed = False
        results.append(result)

    print(json.dumps({"passed": overall_passed, "results": results}, indent=2))
    return 0 if overall_passed else 1


raise SystemExit(main())
PY
