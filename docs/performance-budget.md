# Performance Budget

These are starting targets for the design-center machine and are intentionally
strict enough to shape architecture choices.

- daemon cold start target: under 200 ms before browser work
- local control-plane health check: under 10 ms median
- tab action dispatch overhead target: under 20 ms before browser latency
- snapshot/text response path: avoid large in-memory artifact buffering
- event-tail plus manifest-only and portable replay serialization should stay on
  the benchmarked JSON/persistence path and remain bounded by explicit `limit`
  values
- artifact crop generation and checksum/materialization should stay on the
  benchmarked stream-to-disk path; do not promote replay portability by holding
  full artifacts in memory
- grid-crop expansion, trace capture, and screenshot-recording capture must
  stay explicitly bounded by input limits so operator evidence collection does
  not silently become an unbounded browser workload
- large artifacts: stream to disk, do not assemble giant in-memory payloads

The local production gate now enforces a narrow benchmark manifest through
`benches/thresholds.json` and `scripts/bench/threshold-check.sh`.

Broader budgets become local production gates once repeated baseline
measurements exist.

## Required next step

- collect repeated `darwin/arm64` measurements for readiness, health, doctor, `tab_action`, text, snapshot, replay export, and artifact derivation
- store those measurements in a durable metrics database rather than only in one-off audit folders
- expand the current narrow manifest with additional justified thresholds and
  publish each candidate with the exact sample set used to justify it
- keep gate failures limited to thresholds stable enough to be defensible

Performance claims are provisional until they are measured, stored, compared
over time, and enforced.
