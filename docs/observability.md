# Observability

The baseline observability contract includes:

- structured logs
- stable `run_*` IDs created at runtime boot
- append-only event tailing through the shared SQLite runtime
- artifact-to-run correlation for capture outputs
- artifact provenance for derived inspection crops
- bounded trace and recording artifacts that travel through the same run/event
  and replay-export model as screenshots and PDFs
- replay-manifest export in manifest-only and portable bundle modes for agent
  handoff and debugging
- scenario summary aggregation over stored runs, status counts, assertion
  failures, latency min/median/max, latest run, and latest commit
- diagnose output for side-effect-free readiness and remediation truth
- doctor validation for missing files, checksum mismatches, and replay
  provenance errors
- doctor output for environment truth
- health and doctor output for lease coverage posture
- health and doctor output for attach continuity outcome and freshness

Heavy external observability stacks remain intentionally deferred while the
portable replay path proves out.

## Required next observability layer

- broader durable metrics usage for scenario thresholds, failures, and usability signals
- named real-web scenario runs with stored outcomes and artifact links
- repeated attach, lease, and replay validation results that can be compared over time
- fresh-agent and weak-prompt metrics instead of prose-only confidence
- operator-diagnosis metrics such as time-to-root-cause and manual intervention count
- explicit PinchTab comparison records for named scenarios

Observability is not complete if the repo can explain a single run but cannot
measure whether the product is actually improving over time.
