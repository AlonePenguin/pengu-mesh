role: release_gate_expander
task_id: fresh-agent-release-gate
requested_by_holder_id: active-thread-goal
assigned_holder_id: codex
commit: foreground commit containing this handoff
scope: Promote the fresh-agent cold-start workflow into the local production gate and scenario-gate manifest while preserving the clean-runtime claim by running it first in the gate runtime root.
files_read:
  - examples/workflows/fresh-agent/run.sh
  - examples/workflows/fresh-agent/README.md
  - examples/workflows/common.sh
  - scripts/release/local-gate.sh
  - scripts/release/scenario-gates.json
  - scripts/release/README.md
  - examples/workflows/README.md
  - docs/current-status.md
  - docs/feature-file-map.md
  - docs/implementation-backlog.md
  - docs/milestone-plan.md
  - docs/agent-execution-charter.md
files_changed:
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
  - fresh-agent remains a scenario proof surface, outside runtime lease coordination
  - the local gate runs fresh-agent before the other named scenarios so the shared gate runtime root is still clean when cold-start usability is measured
commands_run:
  - PENGU_MESH_RUNTIME_ROOT=reports/audit/20260601T172826Z_fresh_agent_probe/runtime-root /bin/zsh ./examples/workflows/fresh-agent/run.sh reports/audit/20260601T172826Z_fresh_agent_probe
  - zsh -n scripts/release/local-gate.sh examples/workflows/fresh-agent/run.sh
  - python3 -m json.tool scripts/release/scenario-gates.json
  - git diff --check
  - ./scripts/release/local-gate.sh
  - npm --prefix web/dashboard run build
artifacts:
  - reports/audit/20260601T172826Z_fresh_agent_probe/summary.md
  - reports/audit/20260601T172826Z_fresh_agent_probe/scenario-run-detail.json
  - reports/audit/20260601T173145Z_local_gate/summary.md
  - reports/audit/20260601T173145Z_local_gate/scenario-summary.json
  - reports/audit/20260601T173145Z_local_gate/scenario-gates.json
scenario_or_run_ids:
  - scenario_run_fresh_agent_fresh_agent_v1_70569_1780334906858893000_70569_0
  - scenario_run_fresh_agent_fresh_agent_v1_90407_1780335152856635000_90407_0
outcome:
  - fresh-agent passed as an isolated probe with ten steps, thirteen assertions, zero assertion failures, and latency samples for health, diagnose, doctor, host-access, profile lifecycle, instance lifecycle, tab inventory, and shutdown
  - local-gate now records fresh-agent first before startup-readiness and the existing scenario families
  - scenario-gates.json now contains six passing release gates, including fresh-agent latency thresholds for health, diagnose, instance-start, and tab-list
  - docs and status maps now identify startup-readiness, fresh-agent, evidence-chain, operator-diagnosis, structured-failure, and weak-prompt as gate-wired families
open_risks:
  - live-web and pinchtab-comparison remain outside the release gate
  - fresh-agent has a first release gate, but broader prompt-pack and recovery-drill coverage is still future work
next_owner: live_web_or_comparison_gate_owner
