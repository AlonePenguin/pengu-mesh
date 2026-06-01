role: live_web_gate_owner
task_id: live-web-release-gate
requested_by_holder_id: active-thread-goal
assigned_holder_id: codex
commit: foreground commit containing this handoff
scope: Promote the live-web workflow into the local production gate and scenario-gate manifest so real DNS/TLS-backed browser proof is release-gated rather than merely documented.
files_read:
  - examples/workflows/live-web/run.sh
  - examples/workflows/live-web/README.md
  - examples/workflows/pinchtab-comparison/run.sh
  - examples/workflows/pinchtab-comparison/README.md
  - scripts/release/local-gate.sh
  - scripts/release/scenario-gates.json
  - scripts/release/README.md
  - examples/workflows/README.md
  - README.md
  - docs/current-status.md
  - docs/feature-file-map.md
  - docs/implementation-backlog.md
  - docs/milestone-plan.md
  - docs/agent-execution-charter.md
files_changed:
  - README.md
  - scripts/release/local-gate.sh
  - scripts/release/scenario-gates.json
  - scripts/release/README.md
  - examples/workflows/README.md
  - docs/current-status.md
  - docs/feature-file-map.md
  - docs/implementation-backlog.md
  - docs/milestone-plan.md
  - docs/agent-execution-charter.md
lease_or_capability_decisions:
  - live-web remains a scenario proof surface outside runtime lease coordination
  - the release gate now treats real public-page navigation and artifact verification as product truth, with freshness and latency checks over stored scenario evidence
commands_run:
  - PENGU_MESH_RUNTIME_ROOT=reports/audit/20260601T173856Z_live_web_probe/runtime-root /bin/zsh ./examples/workflows/live-web/run.sh reports/audit/20260601T173856Z_live_web_probe
  - zsh -n scripts/release/local-gate.sh examples/workflows/live-web/run.sh
  - python3 -m json.tool scripts/release/scenario-gates.json
  - git diff --check
  - ./scripts/release/local-gate.sh
  - npm --prefix web/dashboard run build
artifacts:
  - reports/audit/20260601T173856Z_live_web_probe/summary.md
  - reports/audit/20260601T173856Z_live_web_probe/scenario-run-detail.json
  - reports/audit/20260601T174157Z_local_gate/summary.md
  - reports/audit/20260601T174157Z_local_gate/scenario-summary.json
  - reports/audit/20260601T174157Z_local_gate/scenario-gates.json
scenario_or_run_ids:
  - scenario_run_live_web_live_web_v1_53095_1780335536414369000_53095_0
  - scenario_run_live_web_live_web_v1_5044_1780335792030186000_5044_0
outcome:
  - live-web passed as an isolated probe with seven steps, eleven assertions, zero assertion failures, and latency samples for instance-start, tab-open, snapshot, screenshot, text, artifact-list, artifact verification, and shutdown
  - local-gate now records live-web after startup-readiness and before evidence-chain
  - scenario-gates.json now contains seven passing release gates, including live-web latency thresholds for tab-open, tab-snapshot, tab-screenshot, tab-text, and artifact-verify-text
  - docs and status maps now identify startup-readiness, fresh-agent, live-web, evidence-chain, operator-diagnosis, structured-failure, and weak-prompt as gate-wired families
open_risks:
  - live-web still covers a single stable public page; broader multi-page drills remain future work
  - pinchtab-comparison remains outside the release gate and should be the next comparison/leaderboard lane
next_owner: comparison_gate_owner
