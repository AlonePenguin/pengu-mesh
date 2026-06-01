role: scenario_evidence_owner
task_id: scenario-gate-freshness-ceilings
requested_by_holder_id: pengu-mesh-autonomous-improvement-sprint
assigned_holder_id: codex
commit: pending
scope: Add typed freshness ceilings to scenario-gate across CLI, MCP, HTTP, shared types, release manifest, docs, and proof.
files_read:
  - docs/autonomous-operating-model.md
  - docs/observability.md
  - docs/current-status.md
  - docs/feature-file-map.md
  - scripts/release/README.md
  - scripts/release/scenario-gates.json
  - scripts/release/scenario-gate-manifest.py
  - crates/pengu-mesh/src/main.rs
  - crates/pengu-mesh-mcp/src/lib.rs
  - crates/pengu-mesh-core/src/lib.rs
  - crates/pengu-mesh-shared/src/types.rs
files_changed:
  - crates/pengu-mesh/src/main.rs
  - crates/pengu-mesh-mcp/src/lib.rs
  - crates/pengu-mesh-core/src/lib.rs
  - crates/pengu-mesh-shared/src/types.rs
  - scripts/release/scenario-gate-manifest.py
  - scripts/release/scenario-gates.json
  - docs/observability.md
  - docs/current-status.md
  - docs/feature-file-map.md
  - scripts/release/README.md
lease_or_capability_decisions:
  - scenario evidence remains outside live lease coordination because it reads stored runtime state only
  - freshness policy is explicit and typed rather than inferred by local-gate
commands_run:
  - cargo fmt --all
  - cargo fmt --all --check
  - cargo test -p pengu-mesh-core scenario_gate -- --nocapture
  - cargo test -p pengu-mesh-mcp scenario -- --nocapture
  - cargo test -p pengu-mesh http_router_exposes_scenario_routes -- --nocapture
  - cargo check --workspace
  - cargo test --workspace
  - ./scripts/release/local-gate.sh
artifacts:
  - reports/audit/20260601T105509Z_local_gate/summary.md
  - reports/audit/20260601T105509Z_local_gate/scenario-gates.json
scenario_or_run_ids:
  - release manifest families: startup-readiness, evidence-chain, operator-diagnosis, structured-failure, weak-prompt
outcome:
  - scenario-gate can now fail stale evidence using max_latest_age_minutes and the release manifest opts into freshness ceilings
open_risks:
  - live-web, fresh-agent, and pinchtab-comparison remain outside the gate manifest
  - freshness currently keys off latest started_at rather than commit equality or repeated-day coverage
next_owner: release_proof_auditor
