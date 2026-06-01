# Implementation Backlog

This is the tracked backlog for the major product gaps that are still open on
`main`. It is intentionally shorter and more operational than the milestone
plan.

Use this together with:

- `docs/current-status.md` for shipped truth
- `docs/feature-file-map.md` for where the work lives
- `docs/milestone-plan.md` for staged roadmap

## Priority legend

- `active-next`: should be the next serious build lane
- `high`: required for the repo to claim product maturity
- `medium`: important but should follow the foundational gaps

## Workstreams

| Priority | Workstream | Why it matters now | Primary owning paths | Proof to close the work |
| --- | --- | --- | --- | --- |
| active-next | Metrics database and scenario recorder (in progress) | the storage foundation is now real, but the repo still needs threshold- and comparison-grade usage of the recorded data | `crates/pengu-mesh-state/src/lib.rs`, `crates/pengu-mesh-shared/src/types.rs`, `crates/pengu-mesh-core/src/lib.rs`, `docs/observability.md`, `docs/performance-budget.md` | scenario storage is exercised by multiple named families, scenario-list/detail remain truthful, and the stored metrics start driving gate or budget decisions |
| active-next | Scenario corpus under `examples/workflows/` (in progress) | the repo now has startup-readiness, evidence-chain, structured-failure, live-web, weak-prompt, fresh-agent, operator-diagnosis, and pinchtab-comparison, but it still needs broader repeated drills and better gate coverage across those families | `examples/workflows/README.md`, `examples/workflows/startup-readiness/`, `examples/workflows/evidence-chain/`, `examples/workflows/structured-failure/`, `examples/workflows/live-web/`, `examples/workflows/weak-prompt/`, `examples/workflows/fresh-agent/`, `examples/workflows/operator-diagnosis/`, `examples/workflows/pinchtab-comparison/`, `docs/feature-file-map.md` | repeated live-web, weak-prompt, fresh-agent, and operator-diagnosis packs plus repeated comparison reruns exist in repo |
| active-next | PinchTab comparison program (in progress) | the first repo-owned comparison lane now exists, but the repo still needs repeated stored comparisons and downstream use in leaderboard or gate decisions | `examples/workflows/pinchtab-comparison/`, `reference/upstream/pinchtab.METADATA.json`, `docs/baseline/parity-matrix.md` | repeated comparison runs with stored evidence tied to commit, platform, artifacts, and comparison target |
| high | Named PinchTab parity modules on `main` | the parity matrix now names the remaining upstream-only runtime surfaces, and several now have foundation-only helpers or a partial dashboard scaffold, but none of them ship as public contracts on `main` yet | `docs/baseline/parity-matrix.md`, `docs/feature-file-map.md`, `docs/current-status.md`, `crates/pengu-mesh-core/src/lib.rs`, `crates/pengu-mesh-cdp/src/lib.rs`, `crates/pengu-mesh-http/src/lib.rs`, `web/dashboard/` | the named parity gaps are either shipped on `main` or explicitly retired from the parity story with honest docs |
| high | Performance threshold gate | performance is declared a product feature, and the gate now has a first narrow manifest, but the repo still needs broader measured coverage and defensible budgets | `scripts/release/local-gate.sh`, `scripts/bench/`, `benches/thresholds.json`, `docs/performance-budget.md`, future metrics schema | justified thresholds and threshold failures extend beyond the current four-benchmark manifest |
| high | Authenticated ownership and capability gating | the safe/elevated/dangerous posture is visible and the first dangerous gates require `PENGU_MESH_CAPABILITY_GRANTS`, but trusted-local `holder_id` is still not enough for a stronger product claim | `crates/pengu-mesh-state/src/lib.rs`, `crates/pengu-mesh-core/src/lib.rs`, `crates/pengu-mesh-shared/src/types.rs`, `docs/security-model.md`, `web/dashboard/` | real ownership model and broader typed denial paths across CLI/MCP/HTTP that use the surfaced risk tiers |
| high | Operator console | current operator value is still split across doctor, replay, release scripts, and the new read-only dashboard scaffold | `web/dashboard/`, `crates/pengu-mesh-http/src/lib.rs`, `docs/architecture.md` | an operator surface people actually use instead of command hunting |
| high | Durable task plane | leases coordinate access but do not schedule or manage work | `crates/pengu-mesh-state/src/lib.rs`, `crates/pengu-mesh-core/src/lib.rs`, `crates/pengu-mesh-shared/src/types.rs` | task lifecycle, queueing, ownership, cancellation, and replay linkage |
| medium | Repo hygiene and proof retention | `.claude/` was disposable staging state, raw proof can expose local machine details, and the repo needs an explicit keep/archive/trash policy so proof does not become operational drag | `docs/repo-hygiene-plan.md`, `reports/local-gate/`, `reports/audit/`, `scripts/release/README.md`, future prune helpers under `scripts/release/` | `.claude/` is removed, raw bundles stay ignored by default, retained proof is curated deliberately, and promotion rules stop carrying browser caches forward |
| medium | Broader desktop-control plane | the repo is still browser-native, not truly multi-surface | `crates/pengu-mesh-macos/src/lib.rs`, future platform crates, `docs/current-status.md` | clear non-browser surface model with safety and proof |
| medium | Stronger native-surface identity | app-name/PID fallback is workable but weak for attached browser truth | `crates/pengu-mesh-macos/src/lib.rs`, `docs/product-requirements.md` | stable identity better than current fallback |
| medium | Chunked or streamed heavyweight capture | large pages still risk transport-envelope and memory pain | `crates/pengu-mesh-core/src/lib.rs`, `crates/pengu-mesh-artifacts/src/lib.rs`, `crates/pengu-mesh-cdp/src/lib.rs` | bounded streaming capture on real heavy pages |
| medium | Compiled Apple framework shims | the Python bridge is honest but not the end state | `crates/pengu-mesh-macos/src/lib.rs`, `crates/pengu-mesh-cdp/src/lib.rs` | direct compiled bridge where it meaningfully improves robustness |

## Named PinchTab parity gaps still open on `main`

| Capability | Parent workstream | Current repo truth | Primary owning paths |
| --- | --- | --- | --- |
| Task scheduling and queue management | Named PinchTab parity modules on `main` | foundation-only; queue primitives exist in core, but the lease layer still does not queue or dispatch public work | `crates/pengu-mesh-core/src/task_queue.rs`, `crates/pengu-mesh-state/src/lib.rs`, `crates/pengu-mesh-shared/src/types.rs` |
| Allocation policies (FCFS, round-robin, random) | Named PinchTab parity modules on `main` | foundation-only; policy helpers exist in core, but no runtime scheduling surface ships yet | `crates/pengu-mesh-core/src/allocation.rs`, `crates/pengu-mesh-state/src/lib.rs` |
| Semantic element matching | Named PinchTab parity modules on `main` | foundation-only; lexical matcher exists in core, but browser interactions still rely on direct selectors | `crates/pengu-mesh-core/src/semantic.rs`, `crates/pengu-mesh-shared/src/types.rs` |
| Human-like input emulation | Named PinchTab parity modules on `main` | foundation-only; deterministic human timing helpers exist in CDP, but no tab-action contract uses them yet | `crates/pengu-mesh-cdp/src/human.rs`, `crates/pengu-mesh-core/src/lib.rs` |
| Ad-block and tracker filtering | Named PinchTab parity modules on `main` | foundation-only; CDP block-list and Fetch helpers exist, but no public runtime surface enables them yet | `crates/pengu-mesh-cdp/src/adblock.rs`, `crates/pengu-mesh-core/src/lib.rs` |
| Animation suppression | Named PinchTab parity modules on `main` | foundation-only; reduced-motion and CSS suppression helpers exist, but no product contract turns them on | `crates/pengu-mesh-cdp/src/animations.rs`, `crates/pengu-mesh-core/src/lib.rs` |
| Auto-restart strategies | Named PinchTab parity modules on `main` | foundation-only; continuity is shipped, but restart tracker is not yet wired into a crash-recovery policy | `crates/pengu-mesh-core/src/autorestart.rs`, `crates/pengu-mesh-state/src/lib.rs` |
| HTTP proxy forwarding | Named PinchTab parity modules on `main` | foundation-only; reverse-proxy helper exists, but no typed HTTP route is shipped | `crates/pengu-mesh-http/src/proxy.rs`, `crates/pengu-mesh-core/src/lib.rs`, `crates/pengu-mesh-shared/src/failure.rs` |
| Operator dashboard shell | Named PinchTab parity modules on `main` | partial; `web/dashboard/` now carries a read-only health console scaffold, but broader operator workflows are still missing | `web/dashboard/`, `crates/pengu-mesh-http/src/lib.rs`, `docs/architecture.md` |
| Webhook notifications | Named PinchTab parity modules on `main` | foundation-only; validated delivery helpers exist in core, but no callback contract triggers them | `crates/pengu-mesh-core/src/webhook.rs`, `crates/pengu-mesh-http/src/lib.rs`, `crates/pengu-mesh-shared/src/types.rs` |

## Repeated blockers to watch

- doc drift between README, current-status, milestone-plan, release docs, and role-pack docs
- proofs generated outside the repo and then treated as if they were durable
- new gate steps landing without synchronized doc updates
- scenario ambitions expanding faster than the repo’s ability to store and compare results
- trusted-local coordination being treated as stronger than it really is

## Exit criteria for this backlog to feel healthy

- the next serious lane turns the stored scenario data into thresholded or comparative proof instead of docs-only claims
- `examples/workflows/` grows beyond the current named families with broader repeated live-web, weak-prompt, fresh-agent, and operator-diagnosis coverage plus repeated comparison reruns
- the first thresholded performance budget is justified from stored evidence and
  expanded beyond the current narrow benchmark manifest
- ownership and capability gating move from visible posture to enforced
  executable contract
- the operator console, dashboard shell, and task plane stop being placeholder nouns
