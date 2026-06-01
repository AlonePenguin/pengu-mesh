# Autonomous Operating Model

This document defines how pengu mesh should use autonomous agents and
subagents without turning proof into folklore. The unit of progress is a
verified handoff: a role owns a claim, changes the smallest honest surface that
proves it, and leaves enough evidence for the next agent to continue without
guessing.

The model is intentionally compact. The repo already has specialist agents
under `.codex/agents/`; this document describes how to coordinate them.

## Operating Rules

- Start from current repo and GitHub state, not memory.
- Assign one owner per lane and avoid overlapping write scopes.
- Keep CLI, MCP, HTTP, shared types, docs, lease disposition, and release proof
  aligned for every public surface.
- Prefer proof gates over narrative claims.
- Treat `holder_id` as trusted-local coordination only, not authentication.
- Use subagents for bounded sidecar work while the orchestrator keeps the
  critical path moving.
- Every serious lane leaves a handoff artifact.

## Durable Lanes

| Lane | Coordinates | Responsibilities | Required handoff |
| --- | --- | --- | --- |
| `proof_orchestrator` | `default`, `worker` | Scope work, assign lanes, preserve the objective, decide proof before merge. | `handoff.md` |
| `runtime_contract_owner` | `runtime-owner`, `contract-keeper` | Keep CLI/MCP/HTTP/shared types/docs aligned. No public surface ships without failure semantics and lease disposition. | `contract-delta.md` |
| `access_ownership_steward` | `concurrency-steward`, `readiness-contract-owner` | Own leases, holder semantics, diagnose/preflight, capability grants, and future authenticated ownership. | `ownership-report.md` |
| `browser_reality_operator` | `browser-operator`, `cdp-attach-specialist` | Prove real Chrome Dev behavior, attach continuity, native surfaces, artifacts, and cleanup. | `browser-proof.md` |
| `scenario_evidence_owner` | `scenario-harness-owner`, `observability-owner` | Turn product claims into named workflows, stored scenario runs, assertions, and latency samples. | `scenario-run.md` |
| `metrics_comparison_lead` | `perf-bench-lead`, `pinchtab-gap-hunter` | Convert repeated evidence into gates, PinchTab comparisons, budgets, and leaderboard inputs. | `comparison-metrics.md` |
| `release_proof_auditor` | `release-auditor`, `reviewer`, `monitor` | Verify gates, stale docs, CI, lingering processes, and residual risk before any claim. | `release-audit.md` |

## Handoff Schema

Every handoff should use this shape, even when short:

```text
role:
task_id:
requested_by_holder_id:
assigned_holder_id:
commit:
scope:
files_read:
files_changed:
lease_or_capability_decisions:
commands_run:
artifacts:
scenario_or_run_ids:
outcome:
open_risks:
next_owner:
```

## Spawn Rules

Use subagents when the delegated task is concrete, self-contained, and not the
orchestrator's immediate blocker.

Good parallel tasks:

- codebase reconnaissance for a specific surface
- doc ownership map and handoff structure
- bounded implementation with disjoint write ownership
- release audit while the orchestrator prepares a final diff

Bad parallel tasks:

- vague strategy requests with no artifact
- duplicate investigation of the same surface
- edits that overlap the orchestrator's active files
- tasks that require a stronger auth model than the repo currently ships

## Evidence Priority

The preferred proof order is:

1. typed runtime payloads
2. stored scenario runs and `scenario_gate`
3. local gate outputs under ignored report directories
4. CI checks on `main`
5. docs that match the verified behavior

Narrative summaries are not proof unless they point to one of those artifacts.

