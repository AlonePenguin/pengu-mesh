# Release Lane

Release automation is intentionally deferred until the daemon, doctor, and MCP
surfaces have stable contracts on `darwin/arm64`.

The current production gate is intentionally local-only and repo-owned:

- `./scripts/release/local-gate.sh`
- `./scripts/release/diagnose-smoke.sh`
- `./scripts/release/lease-smoke.sh`
- `./scripts/release/continuity-smoke.sh`
- `./scripts/release/attach-continuity-smoke.sh`
- `./scripts/release/host-access-smoke.sh`
- `./scripts/release/browser-lifecycle-integration.sh`
- `./scripts/release/tab-lifecycle-integration.sh`
- `./scripts/release/evidence-chain-smoke.sh`
- `./scripts/release/browser-surface-smoke.sh`
- `./scripts/release/promote-local-gate-bundle.sh`

`continuity-smoke.sh` performs a bounded daemon restart proof against an
isolated runtime root, verifies operator/run/lease recovery over the HTTP
surface, confirms stale-instance classification, and exits cleanly without
leaving a background daemon behind.

`diagnose-smoke.sh` verifies the side-effect-free agent-readiness surface.
`host-access-smoke.sh` verifies the machine host-access matrix, setup audit
flow, and settings deeplink inventory. `browser-lifecycle-integration.sh`
proves attach plus native-surface capture on a real headed browser.
`tab-lifecycle-integration.sh` proves navigate, evaluate, snapshot,
screenshot, text, artifact inventory, and artifact verification.
The local gate uses the named `evidence-chain` workflow to prove persisted
artifact verification before corruption, invalidation after corruption without
mutating stored metadata, and manifest-gated scenario evidence. The narrower
`evidence-chain-smoke.sh` remains available as a standalone probe.
`browser-surface-smoke.sh` launches a managed Chrome Dev instance under an
isolated runtime root, proves native browser-surface discovery and artifact
capture, and records both fallback and explicit takeover telemetry.
`local-gate.sh` now also runs the named `startup-readiness` workflow under the
gate-owned runtime root, validates that the recorded scenario finished with
`status = "passed"`, writes the stored scenario inventory to
`scenario-list.json`, writes the aggregate scenario evidence summary to
`scenario-summary.json`, evaluates the multi-family `scenario-gates.json`
manifest result against `scripts/release/scenario-gates.json`, and enforces the
current narrow benchmark manifest via `scripts/bench/threshold-check.sh`.

## Output location policy

- Temporary script output may live outside the repo while a run is active.
- Raw local-gate and audit outputs should stay under ignored local report
  paths unless a maintainer has reviewed them for public release.
- Use `promote-local-gate-bundle.sh` only when a raw local-gate bundle needs a
  curated durable form; then review the output before forcing it into git.
- Before retaining a heavyweight audit bundle, prune browser profile caches and
  similar runtime byproducts called out in `docs/repo-hygiene-plan.md`; they
  are local working state, not durable proof.
- JSON artifacts must keep stdout and stderr separated so machine-readable
  outputs remain trustworthy.
- Browser-facing integration proof is not complete until the relevant capture
  artifact has been visually checked.

The next release-lane hardening step is not “more scripts” in the abstract. It
is a stored scenario and metrics program that promotes repeated live-web,
fresh-agent, and comparative validations into the release discipline.
