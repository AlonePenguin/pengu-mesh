# Startup Readiness

This family is the smallest end-to-end readiness proof that still exercises the
metrics recorder, durable scenario tables, and the core browser lifecycle.

`run.sh` records a named scenario run, captures latency for each command, and
stores assertions for:

- isolated-runtime `health`
- isolated-runtime `diagnose`
- isolated-runtime `pengu-mesh-doctor -- --json`
- managed Chrome Dev launch
- base64-backed tab open
- screenshot capture and artifact integrity
- clean instance shutdown

Artifacts and the scenario summary stay inside the provided output directory.
The scenario run itself is persisted in the runtime SQLite database under the
isolated runtime root.
