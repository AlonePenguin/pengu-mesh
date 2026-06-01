role: runtime_contract_owner
task_id: scenario-evidence-readiness-surfaces
requested_by_holder_id: pengu-mesh-autonomous-improvement-sprint
assigned_holder_id: codex
commit: foreground commit containing this handoff
scope: Surface stored scenario-evidence posture through diagnose and doctor without violating read-only diagnostic trust, and align docs plus transport proof.
files_read:
  - docs/autonomous-operating-model.md
  - docs/current-status.md
  - docs/feature-file-map.md
  - docs/observability.md
  - docs/mcp-contract.md
  - docs/agent-quickstart.md
  - README.md
  - crates/pengu-mesh-core/src/lib.rs
  - crates/pengu-mesh-shared/src/types.rs
  - crates/pengu-mesh-shared/src/lib.rs
  - crates/pengu-mesh-state/src/lib.rs
  - crates/pengu-mesh-doctor/src/lib.rs
  - crates/pengu-mesh-mcp/src/lib.rs
  - crates/pengu-mesh/src/main.rs
files_changed:
  - README.md
  - crates/pengu-mesh-core/src/lib.rs
  - crates/pengu-mesh-doctor/src/lib.rs
  - crates/pengu-mesh-mcp/src/lib.rs
  - crates/pengu-mesh-shared/src/lib.rs
  - crates/pengu-mesh-shared/src/types.rs
  - crates/pengu-mesh-state/src/lib.rs
  - crates/pengu-mesh/src/main.rs
  - docs/agent-quickstart.md
  - docs/current-status.md
  - docs/feature-file-map.md
  - docs/mcp-contract.md
  - docs/observability.md
lease_or_capability_decisions:
  - diagnose and doctor remain intentionally outside lease coordination because they inspect host and stored runtime state only
  - scenario evidence inspection uses read-only state access when a runtime database exists and does not create runtime directories or sqlite files when none exist
commands_run:
  - cargo fmt --all
  - cargo test -p pengu-mesh-state inspect_existing_stays_read_only_when_state_is_absent -- --nocapture
  - cargo test -p pengu-mesh-shared scenario_payloads_round_trip -- --nocapture
  - cargo test -p pengu-mesh-core diagnose_and_doctor_surface_scenario_evidence_posture -- --nocapture
  - cargo test -p pengu-mesh-mcp diagnose_preserves_runtime_report_shape -- --nocapture
  - cargo test -p pengu-mesh http_router_exposes_diagnose_route -- --nocapture
  - cargo fmt --all --check
  - cargo check --workspace
  - cargo test --workspace
  - ./scripts/release/local-gate.sh
  - cargo fmt --all --check
  - cargo check --workspace
  - cargo test --workspace
  - ./scripts/release/local-gate.sh
artifacts:
  - reports/audit/20260601T155842Z_local_gate/cargo-test.txt
  - reports/audit/20260601T172041Z_local_gate/summary.md
  - reports/audit/20260601T172041Z_local_gate/scenario-gates.json
scenario_or_run_ids:
  - synthetic core test families: startup-readiness, weak-prompt
outcome:
  - diagnose and doctor now expose typed scenario_evidence posture with latest per-family status summaries
  - read-only inspection returns degraded when no runs exist and unknown when scenario state cannot be inspected
  - transport and docs now advertise scenario_evidence on CLI, MCP, and HTTP diagnose surfaces
  - foreground verification passed full cargo workspace tests and the local production gate
open_risks:
  - the earlier sandbox loopback bind failures did not reproduce in foreground verification
next_owner: release_gate_expander
