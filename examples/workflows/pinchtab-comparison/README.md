# PinchTab Comparison

Family name: `pinchtab-comparison`
Scenario version: `v1`

This family measures pengu mesh performance on standard browser-automation
operations and generates a structured comparison report against PinchTab
baseline values.

## What it proves

Quantified performance comparison against PinchTab baseline on six core
operations: instance startup, page navigation, tab snapshot, tab screenshot,
artifact verification, and instance shutdown. The report ties each result back
to the current commit, branch, platform, runtime artifact, and comparison
target metadata.

## How to run

```
./run.sh
```

Or from the repo root:

```
examples/workflows/pinchtab-comparison/run.sh
```

## Notes

This measures pengu mesh only; PinchTab baseline values are declared as static
constants from prior measurement. The output directory contains:

- `comparison-report.json` with leaderboard-friendly structured comparison data
- `summary.md` with the run id, environment, winner, and artifact paths
- `scenario-run-detail.json` when the local runtime can query the stored run
  detail after completion

See `docs/baseline/parity-matrix.md` for broader parity context.
