# State And Replay

- SQLite is the metadata store
- WAL is the default journal mode
- `runs` tracks capture lifecycles by stable `run_*` IDs
- `events` is append-only and ordered by SQLite sequence for deterministic tail
  reads
- `artifacts.run_id` links snapshots, text captures, screenshots, PDFs, and
  derived crops back to the run that produced them
- artifact provenance stores `source_artifact_id`, normalized crop bounds, and
  optional PDF `page_index` for derived inspection artifacts
- `artifact_crop_grid` produces deterministic bounded batches of derived crops
  without re-decoding the same source artifact for every region
- `events_tail` is the first live replay-facing surface for recent timeline
  inspection
- trace JSON and screenshot-recording archives are now stored as first-class
  replay artifacts, so portable bundles can carry both visual evidence and
  browser-timeline evidence without a separate export path
- `replay_export --mode manifest_only` writes a lightweight manifest that keeps
  source artifact paths in place for fast local inspection
- `replay_export --mode portable` stages a self-contained bundle, copies
  run-linked artifacts by streaming, computes SHA-256 checksums, and records
  only relative bundle paths in the manifest
- replay manifests are now schema version 2 and include bundle metadata,
  inspection modes, ordered events, and replay artifact records with
  materialization/checksum state across screenshots, PDFs, traces, recordings,
  and derived crops
