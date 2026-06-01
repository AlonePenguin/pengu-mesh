#!/usr/bin/env python3
"""Run a release scenario gate manifest against the current runtime root."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path
from typing import Any


def load_manifest(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as handle:
        manifest = json.load(handle)
    if manifest.get("schema_version") != "scenario-gate-manifest.v1":
        raise ValueError(
            "scenario gate manifest must use schema_version scenario-gate-manifest.v1"
        )
    if not isinstance(manifest.get("gates"), list) or not manifest["gates"]:
        raise ValueError("scenario gate manifest must define at least one gate")
    return manifest


def add_optional(args: list[str], flag: str, value: Any) -> None:
    if value is not None:
        args.extend([flag, str(value)])


def gate_command(
    cargo_bin: str,
    default_limit: int,
    gate: dict[str, Any],
    threshold: dict[str, Any] | None,
) -> list[str]:
    family = gate.get("family")
    if not family:
        raise ValueError(f"gate {gate.get('name', '<unnamed>')} is missing family")

    args = [
        cargo_bin,
        "run",
        "--quiet",
        "-p",
        "pengu-mesh",
        "--",
        "scenario-gate",
        "--family",
        str(family),
        "--limit",
        str(gate.get("limit", default_limit)),
        "--min-runs",
        str(gate.get("min_runs", 1)),
        "--max-assertion-failures",
        str(gate.get("max_assertion_failures", 0)),
        "--min-samples-per-metric",
        str(gate.get("min_samples_per_metric", 1)),
    ]
    for status in gate.get("allowed_statuses", ["passed"]):
        args.extend(["--allowed-status", str(status)])

    if threshold is not None:
        args.extend(
            [
                "--threshold-name",
                str(threshold["name"]),
                "--threshold-metric",
                str(threshold["metric"]),
                "--max-ms",
                str(threshold["max_ms"]),
            ]
        )
        add_optional(args, "--p50-ms", threshold.get("p50_ms"))
        add_optional(args, "--p95-ms", threshold.get("p95_ms"))
        add_optional(args, "--p99-ms", threshold.get("p99_ms"))

    return args


def run_gate_invocation(
    cargo_bin: str,
    default_limit: int,
    gate: dict[str, Any],
    threshold: dict[str, Any] | None,
) -> dict[str, Any]:
    command = gate_command(cargo_bin, default_limit, gate, threshold)
    completed = subprocess.run(
        command,
        check=False,
        capture_output=True,
        encoding="utf-8",
        env=os.environ.copy(),
    )
    stdout = completed.stdout.strip()
    parsed: dict[str, Any] | None = None
    parse_error: str | None = None
    if stdout:
        try:
            parsed = json.loads(stdout)
        except json.JSONDecodeError as exc:
            parse_error = str(exc)

    data = parsed.get("data", {}) if parsed else {}
    passed = (
        completed.returncode == 0
        and parsed is not None
        and parsed.get("ok") is True
        and data.get("passed") is True
    )
    return {
        "threshold_name": threshold.get("name") if threshold else None,
        "passed": passed,
        "exit_code": completed.returncode,
        "ok": parsed.get("ok") if parsed else False,
        "code": parsed.get("code") if parsed else "invalid_output",
        "message": parsed.get("message") if parsed else "scenario-gate did not return JSON",
        "stdout_parse_error": parse_error,
        "stderr": completed.stderr.strip(),
        "checks": data.get("checks", []),
        "thresholds": data.get("thresholds", []),
        "recovery": data.get("recovery", []),
    }


def run_gate(cargo_bin: str, default_limit: int, gate: dict[str, Any]) -> dict[str, Any]:
    thresholds = gate.get("thresholds") or [None]
    invocations = [
        run_gate_invocation(cargo_bin, default_limit, gate, threshold) for threshold in thresholds
    ]
    failed = [invocation for invocation in invocations if not invocation["passed"]]
    checks = []
    threshold_results = []
    recovery = []
    for invocation in invocations:
        checks.extend(invocation["checks"])
        threshold_results.extend(invocation["thresholds"])
        recovery.extend(invocation["recovery"])

    first_result = failed[0] if failed else invocations[0]
    return {
        "name": gate.get("name", gate.get("family")),
        "family": gate.get("family"),
        "passed": not failed,
        "ok": not failed,
        "code": "ok" if not failed else first_result["code"],
        "message": "scenario gate passed" if not failed else first_result["message"],
        "checks": checks,
        "thresholds": threshold_results,
        "recovery": sorted(set(recovery)),
        "invocations": invocations,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--cargo-bin", default=os.environ.get("CARGO", "cargo"))
    args = parser.parse_args()

    manifest = load_manifest(args.manifest)
    default_limit = int(manifest.get("default_limit", 25))
    gates = [run_gate(args.cargo_bin, default_limit, gate) for gate in manifest["gates"]]
    failed = [gate for gate in gates if not gate["passed"]]
    payload = {
        "schema_version": "scenario-gate-manifest-result.v1",
        "manifest_path": str(args.manifest),
        "passed": not failed,
        "gate_count": len(gates),
        "passed_count": len(gates) - len(failed),
        "failed_count": len(failed),
        "gates": gates,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("w", encoding="utf-8") as handle:
        json.dump(payload, handle, indent=2, sort_keys=True)
        handle.write("\n")
    print(json.dumps(payload, indent=2, sort_keys=True))
    return 0 if payload["passed"] else 1


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as exc:
        print(f"scenario gate manifest failed: {exc}", file=sys.stderr)
        raise SystemExit(2)
