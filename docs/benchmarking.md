# Benchmarking

## Benchmark lanes

- `benches/json`: response envelope serialization
- `benches/cdp`: target catalog parse path
- `benches/persistence`: runtime-state, event-tail, manifest-only replay, and
  portable replay serialization
- `benches/artifacts`: artifact write-to-disk, single-crop and grid-crop
  generation, recording-archive packaging, checksum, and portable
  materialization paths

## Rules

- benchmark on `darwin/arm64` before locking core dependency decisions
- record the environment fingerprint beside benchmark outputs
- treat new run/event/replay payloads as first-class persistence hot paths, not
  doc-only contracts
- treat crop generation and checksum/materialization as first-class artifact hot
  paths because they back `artifact_crop`, `artifact_crop_grid`, bounded
  recording capture, and portable replay bundles
- use `make bench-run` or `./scripts/bench/run.sh [output_dir]` to write a
  timestamped ignored bundle under `reports/audit/` with per-lane outputs and
  the host fingerprint
- `./scripts/bench/threshold-check.sh` and `./scripts/release/local-gate.sh`
  now enforce the first narrow benchmark manifest in `benches/thresholds.json`;
  broader scenario-backed thresholds remain deferred until stored evidence is
  stable
- store repeated benchmark results in the planned metrics database once it lands
- pair microbench lanes with bounded real-browser scenario timing where operator
  experience matters
- performance regressions become local-gate failures once baselines stabilize
- threshold changes must cite a reviewed summary, audit bundle, or stored metric
  series that justified the change
