# Operator Diagnosis

This family validates the consistency of all diagnostic surfaces exposed by
pengu mesh.  Every probe must return well-formed JSON whose fields match the
documented schema, and the aggregate state reported by `diagnose` must be
consistent with the boolean `ok` returned by `health`.

`run.sh` exercises five diagnostic surfaces under an isolated runtime root:

- `diagnose` — full readiness report with schema_version, state, permissions,
  services, capabilities, and remediations arrays
- `health` — lightweight health envelope with ok and data fields
- `pengu-mesh-doctor --json` — doctor output parseable as JSON with an ok field
- `host-access-status` — platform capability matrix with platform and services
  fields
- `lease-status` — lease state returned as a well-formed JSON structure

After capturing each surface, the scenario cross-validates that `diagnose`
state is consistent with `health` ok, then prints a machine-readable summary.

The provided output directory keeps the per-probe JSON payloads, stderr logs,
the isolated runtime SQLite database, and `summary.md`. The scenario run detail
also records `summary_path` so operators can jump straight from stored metrics
to the human-readable recap.
