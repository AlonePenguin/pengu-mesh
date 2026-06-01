# Agent Execution Charter

## 0. Status

- This is the canonical execution charter for the repository.
- This document is the permanent handoff for the current delivery lane.
- This document replaces any weaker or shorter continuation note.
- This document should be treated as a standing operating contract.
- This document is intentionally line-dense so a fresh agent can use it as a procedural map.
- This document exists because the product bar is now stricter than basic correctness.
- This document exists because the repo must surpass PinchTab in product quality, not just architecture aesthetics.
- This document exists because future work should start from verified truth rather than guessed context.
- This document exists because the user wants proof chains, not narrative optimism.
- This document exists because live behavior now matters as much as compile behavior.
- This handoff was refreshed on 2026-03-12 during the `pengu mesh` hard-cutover lane.
- The verified baseline for this refresh is regenerated through `./scripts/release/local-gate.sh` rather than a committed raw machine-local bundle.
- Historical pre-cutover bundles are private/local evidence and should not be exposed in the public tree.
- Every serious turn should refresh this file after checking both local and remote repo state.

## 1. Mission

- Build the best local agent-access meshing runtime in this lane.
- Make the runtime fast enough that performance is felt, not merely benchmarked.
- Make the runtime simple enough that a weak prompt can still produce strong results.
- Make the runtime observable enough that failures are diagnosed quickly.
- Make the runtime structured enough that fresh agents can self-correct.
- Make the runtime rigorous enough that claims are backed by evidence bundles.
- Make the runtime safe enough that concurrency and attach behavior are explicit rather than accidental.
- Make the runtime agent-native enough that CLI, MCP, and HTTP all preserve typed truth.
- Make the runtime operational enough that local readiness can be trusted through diagnose, health, and doctor.
- Make the runtime better than PinchTab across operator time-to-diagnosis, task execution quality, fresh-agent usability, restart recovery, and auditability.

## 2. Primary Product Standard

- The product standard is not “code compiles.”
- The product standard is not “tests pass.”
- The product standard is not “docs look current.”
- The product standard is not “Rust architecture is cleaner than Go architecture.”
- The product standard is not “MCP exists.”
- The product standard is not “the repo feels promising.”
- The product standard is task dominance under real use.
- The product standard is faster time-to-first-success for a fresh agent.
- The product standard is lower confusion under weak prompts.
- The product standard is lower operator effort during diagnosis.
- The product standard is lower ambiguity in concurrency conflicts.
- The product standard is stronger recovery after browser, daemon, and endpoint churn.
- The product standard is richer evidence with bounded cost.
- The product standard is measured, stored, reviewable behavior over time.
- The product standard is a repo that can defend every important claim with current artifacts.

## 3. What “Surpass PinchTab” Means

- Surpass PinchTab in startup readiness truth.
- Surpass PinchTab in attach continuity truth.
- Surpass PinchTab in fresh-agent usability.
- Surpass PinchTab in operator diagnostics.
- Surpass PinchTab in structured evidence packaging.
- Surpass PinchTab in concurrency clarity.
- Surpass PinchTab in task replay and auditability.
- Surpass PinchTab in route and tool contract consistency.
- Surpass PinchTab in real-world browsing validation.
- Surpass PinchTab in performance guardrails once thresholds are formalized.
- Surpass PinchTab in live regression discipline.
- Surpass PinchTab in purposeful scenario coverage.
- Surpass PinchTab in explainability of failures.
- Surpass PinchTab in discoverability of capabilities.
- Surpass PinchTab in end-user trust and agent trust.

## 4. Current Verified Baseline

- `cargo fmt --all --check` passed in the last serious pass.
- `cargo check --workspace` passed in the last serious pass.
- `cargo test --workspace` passed in the last serious pass.
- `./scripts/release/local-gate.sh` passed in the last serious pass.
- `./scripts/release/local-gate.sh` is the latest full local proof command and writes ignored machine-local output by default.
- Raw proof bundles are not committed unless they have been reviewed and curated for public release.
- The local gate now includes diagnose smoke validation.
- The local gate now includes lease smoke validation.
- The local gate now includes daemon continuity smoke validation.
- The local gate now includes attach continuity smoke validation.
- The local gate now includes host-access capability validation.
- The local gate now includes browser-lifecycle integration validation.
- The local gate now includes tab-lifecycle integration validation.
- The local gate now includes evidence-chain corruption validation.
- The local gate now includes browser-surface native-control validation.
- The local gate now includes startup-readiness scenario validation and scenario-list capture.
- The local gate now includes bench discovery and bench compilation.
- The local gate now captures isolated-runtime health and doctor payloads for the gate run itself.
- Health and doctor emit lease coverage information.
- Health and doctor emit attach continuity outcome and freshness information.
- Health and doctor now emit host-access posture, execution channels, and assistive overlays.
- Lease writer acquisition is transactionally serialized in SQLite.
- Typed lease conflicts are preserved across CLI, MCP, and HTTP.
- Name-only attach reuse was removed from the reuse path.
- Attach reuse now requires endpoint evidence.
- Browser websocket rotation now refreshes stored attach metadata.
- Stale attached instances can now be reclaimed with explicit classification.
- Best-effort tab websocket recovery after refresh now exists for action and capture paths.
- Native browser surfaces can now be listed, snapshotted, and actioned through CLI, MCP, and HTTP.

## 5. Current Evidence Inventory

- Local gate proof is regenerated with `./scripts/release/local-gate.sh`.
- Raw local-gate output is ignored by default because it can contain local paths, screenshots, browser metadata, and host posture.
- Commit only reviewed summaries or explicitly justified artifacts.
- Latest committed visual verification report:
  `reports/visual-verification-20260312T172200Z.md`

## 6. Immediate Reading Order

- Read `AGENTS.md` first.
- Read `docs/current-status.md` second.
- Read `docs/feature-file-map.md` third.
- Read `docs/implementation-backlog.md` fourth.
- Read `docs/vision.md` fifth.
- Read `docs/product-requirements.md` sixth.
- Read `docs/multi-agent-concurrency.md` seventh.
- Read `docs/attach-contract.md` eighth.
- Read `docs/runtime-model.md` ninth.
- Read `docs/mcp-contract.md` tenth.
- Read `docs/observability.md` eleventh.
- Read `docs/performance-budget.md` twelfth.
- Read `docs/benchmarking.md` thirteenth.
- Read `docs/milestone-plan.md` fourteenth.
- Read `docs/baseline/parity-matrix.md` fifteenth.
- Read `docs/baseline/upstream-pinchtab-audit.md` sixteenth.
- Read `crates/pengu-mesh-core/src/lib.rs` seventeenth.
- Read `crates/pengu-mesh-state/src/lib.rs` eighteenth.
- Read `crates/pengu-mesh-shared/src/types.rs` nineteenth.
- Read `crates/pengu-mesh-http/src/lib.rs` twentieth.
- Read `crates/pengu-mesh-mcp/src/lib.rs` twenty-first.
- Read `crates/pengu-mesh/src/main.rs` twenty-second.
- Read `scripts/release/local-gate.sh` twenty-third.
- Read `scripts/release/diagnose-smoke.sh` twenty-fourth.
- Read `scripts/release/lease-smoke.sh` twenty-fifth.
- Read `scripts/release/continuity-smoke.sh` twenty-sixth.
- Read `scripts/release/attach-continuity-smoke.sh` twenty-seventh.
- Read `scripts/release/host-access-smoke.sh` twenty-eighth.
- Read `scripts/release/browser-lifecycle-integration.sh` twenty-ninth.
- Read `scripts/release/tab-lifecycle-integration.sh` thirtieth.
- Read `scripts/release/evidence-chain-smoke.sh` thirty-first.
- Read `scripts/release/browser-surface-smoke.sh` thirty-second.

## 7. Current Runtime Truth

- The standalone tool is the product.
- Rust runtime behavior is the primary product surface.
- `darwin/arm64` remains Tier 1.
- Google Chrome Dev remains the primary target browser.
- Chrome DevTools MCP remains an operator aid and not a product dependency.
- `pengu-mesh diagnose` is the first-pass truth source for agent readiness and remediation.
- `pengu-mesh-doctor` remains the first-pass operator-facing truth source for human-readable diagnosis.
- Health and doctor are now richer than before but still not rich enough.
- CLI, MCP, and HTTP must stay contract-aligned.
- The current lease model is intentionally simple and should stay simple.
- The current attach continuity model is stronger than baseline but not complete.
- The macOS substrate is now broader than the old dialog shim but still browser-first, not a general desktop-control plane.
- Trusted-local coordination is still the holder identity model today.
- Trusted-local coordination is not equivalent to authenticated ownership.
- The runtime still lacks a durable task scheduler plane.
- The runtime still lacks an operator console worthy of the broader product goal.
- The runtime now has a first-class scenario metrics database, but it still lacks thresholded performance budgets and broader scenario coverage.
- The runtime now has a first fresh-agent usability lab, but it still lacks broader prompt-pack coverage.
- The runtime still lacks thresholded performance failures in the gate.
- Large real pages can still overflow the current CDP transport envelope on heavyweight snapshot or screenshot paths.

## 8. Current Lease Truth

- Any operation that mutates browser state requires writer access.
- Any operation that reads shared browser or artifact state with concurrency impact requires observer access.
- Every public instance, tab, and artifact operation must be in the model or explicitly outside it.
- The current writer-required public operations are `instance_start`, `instance_attach`, `instance_stop`, `browser_surface_action`, `tab_open`, `tab_close`, and `tab_action`.
- The current observer-required public operations are `tab_list`, `browser_surface_list`, `browser_surface_snapshot`, `tab_snapshot`, `tab_text`, `tab_screenshot`, `tab_pdf`, `artifact_crop`, `artifact_crop_grid`, `trace_capture`, and `recording_capture`.
- Current intentionally outside-model operations are `browser_health`, `browser_doctor`, `host_access_status`, `host_access_setup`, `profile_list`, `profile_create`, `instance_list`, lease-admin operations, `artifact_handle`, `capture_start_recording`, `capture_stop_recording`, `run_list`, `events_tail`, `replay_export`, and generic tool catalog and dispatch.
- The lease matrix is now surfaced in runtime outputs.
- The lease matrix is now contract-tested against MCP tools and HTTP routes.
- The lease matrix is only as good as the inventory discipline that maintains it.
- Any new public tool or route must force a lease coverage decision before it ships.

## 9. Current Attach Continuity Truth

- External attach remains policy-gated behind `PENGU_MESH_ALLOW_EXTERNAL_ATTACH=1`.
- Endpoint-aware reuse is now the default continuity mechanism.
- Browser websocket refresh is now stored when endpoint metadata rotates.
- Reclaimed stale attached identities are now explicitly surfaced.
- Debug-URL reuse with stale browser websocket evidence now classifies as `reclaimed_stale_instance`.
- Successful attach continuity is now published only after both `/json/version` and `/json/list` succeed.
- Failed attach attempts no longer leave a false live continuity record or a new attached instance row behind.
- Health and doctor now emit attach continuity outcome.
- Health and doctor now emit attach continuity freshness.
- Current outcomes include `new_instance`, `reused_existing_instance`, and `reclaimed_stale_instance`.
- Current freshness states include `live`, `stale_instance`, and `stale_endpoint`.
- Name-only reuse no longer qualifies as identity continuity.
- Daemon continuity counters no longer overcount unrelated live instances.
- Tab reconnect is more resilient than before, but still needs broader real-browser coverage.

## 10. Current Verification Truth

- The repo is not allowed to trust compile-only success.
- The repo is not allowed to trust unit tests alone.
- The repo is not allowed to trust docs that are not backed by runtime evidence.
- The repo is not allowed to trust a single modality.
- The repo is expected to combine terminal proof, JSON output, and browser-visible artifacts where useful.
- The repo is expected to keep raw reports under ignored local report paths.
- The repo is expected to publish curated proof summaries in the same pass as behavior changes when durable proof is part of the claim.
- The repo is expected to sync docs with implementation and verification.
- The repo is expected to verify machine-permission claims and native browser-surface claims with stored JSON plus image proof where practical.
- The repo is expected to keep malformed caller input, missing targets, readiness failures, conflicts, and internal failures distinct across CLI, MCP, and HTTP.
- The repo is expected to narrow claims if verification does not support broader claims.
- The repo is expected to treat live-web checks as part of product truth, not a vanity extra.

## 11. Current Strategic Gap List

- There is no authenticated holder identity yet.
- There is no high-risk capability gating yet.
- There is no durable task queue or fairness plane yet.
- There is no operator console with real product value yet.
- There is no broader live-web and operator-diagnosis scenario corpus yet, and fresh-agent still needs broader prompt-pack coverage.
- There is no agent-usability benchmark corpus yet.
- The weak-prompt regression suite has landed, but there is no fresh-agent
  usability corpus yet.
- There is no end-to-end scenario leaderboard yet.
- There is no performance threshold gate yet.
- There is no automated real-web scenario sweep yet.
- There is no repeated or gate-driving comparison harness against PinchTab yet.
- There is no formal measure of operator time-to-diagnosis yet.
- There is no formal measure of agent time-to-success yet.
- There is no formal measure of intervention count per task yet.
- There is no formal measure of network request effort yet.
- There is still a transport-envelope limit exposed by large Google Images and Wikipedia capture flows.

## 12. What The Next Agent Must Not Do

- Do not treat the existing dirty worktree as suspicious by default.
- Do not reset the tree.
- Do not erase unrelated user changes.
- Do not claim victory because `cargo test` passed.
- Do not declare parity with PinchTab from architecture shape alone.
- Do not widen claims without fresh evidence.
- Do not add a new public surface without a lease decision.
- Do not add a new public surface without a verification story.
- Do not add a new public surface without doc updates.
- Do not paper over uncertainty with softer wording.
- Do not substitute ambition for measurement.
- Do not ship another weak continuation note.

## 13. What The Next Agent Must Do

- Start from current evidence rather than assumptions.
- Rebuild the public-surface matrix whenever a new tool or route appears.
- Run meaningful live checks when behavior changes justify them.
- Compare the product against PinchTab on real tasks, not only specs.
- Push the repo toward stronger defaults and stronger proof.
- Preserve operator truth surfaces as first-class outputs.
- Leave behind stored artifacts that prove changed behavior.
- Treat fresh-agent usability as a core product surface.
- Treat performance enforcement as a future gate, not a future dream.
- Treat real-web task execution as non-optional for the current browser-first slice of pengu mesh.
- Treat the user’s expectations as the operating constraint, not an aspirational note.
- Check `git rev-parse HEAD` and `git rev-parse origin/main` before assuming the repo state.
- Inspect pre-existing dirty files before treating them as part of the active lane.
- Refresh this charter after every serious turn so the next agent starts from current truth.

## 14. Current Code Areas That Matter Most

- `crates/pengu-mesh-core/src/lib.rs` owns the central runtime logic.
- `crates/pengu-mesh-state/src/lib.rs` owns persistence and lease primitives.
- `crates/pengu-mesh-shared/src/types.rs` defines the public contract shapes.
- `crates/pengu-mesh-http/src/lib.rs` exposes the HTTP surface.
- `crates/pengu-mesh-mcp/src/lib.rs` exposes MCP behavior and contract translation.
- `crates/pengu-mesh/src/main.rs` exposes the main CLI control surface.
- `crates/pengu-mesh-cli/src/main.rs` exposes additional CLI affordances and health output behavior.
- `crates/pengu-mesh/tests/lease_matrix_contract.rs` asserts surface coverage truth for leases.
- `scripts/release/local-gate.sh` is the current production gate precursor.
- `scripts/release/lease-smoke.sh` proves representative lease conflict and coexistence behavior.
- `scripts/release/continuity-smoke.sh` proves daemon-root continuity behavior.
- `scripts/release/attach-continuity-smoke.sh` proves restart, rotation, and stale reclaim behavior.
- `scripts/release/host-access-smoke.sh` proves host-access posture and setup auditing.
- `scripts/release/browser-surface-smoke.sh` proves native browser-surface discovery, artifact capture, fallback, and takeover telemetry.

## 15. Current Documentation Files That Matter Most

- `docs/current-status.md` is the short truth statement.
- `docs/feature-file-map.md` is the current feature-to-file ownership map.
- `docs/implementation-backlog.md` is the tracked list of major unshipped work.
- `docs/vision.md` is the product-direction summary.
- `docs/product-requirements.md` is the contract for what must exist.
- `docs/observability.md` is the observability promise.
- `docs/performance-budget.md` is the performance contract seed.
- `docs/benchmarking.md` is the benchmark discipline guide.
- `docs/multi-agent-concurrency.md` is the lease model explanation.
- `docs/attach-contract.md` is the continuity contract explanation.
- `docs/milestone-plan.md` is the roadmap scaffold.
- `scripts/release/README.md` explains the release-lane posture.
- `examples/workflows/README.md` is the current home for in-repo scenario corpus guidance and shipped named families.

## 16. Live PinchTab Findings Already Observed

- A temporary PinchTab checkout can run locally on this machine.
- A direct doctor run revealed a rough operator posture because it failed closed on missing extras and interactive setup assumptions.
- `go test -tags integration ./tests/integration/ -run TestHealth -v -timeout 5m` passed locally.
- `go test -tags integration ./tests/integration/ -run TestNavigate -v -timeout 5m` passed locally.
- The upstream integration harness launched a real server.
- The upstream integration harness launched a real browser.
- Startup showed an exposed security posture warning.
- Initial navigation behavior showed warm-up retries before readiness stabilized.
- Timeout behavior surfaced as a server-side failure after several seconds.
- Upstream still has a broader product envelope in security, operator dashboarding, and job-orchestration surfaces.

## 17. The Most Important Conclusion From The PinchTab Probe

- This repo is already ahead on several core runtime concepts.
- This repo is not yet ahead on the full product system.
- Better core architecture is not enough.
- Better typed contracts are not enough.
- Better replay semantics are not enough.
- Better lease semantics are not enough.
- The repo must ship a better operator experience.
- The repo must ship better security posture.
- The repo must ship better live-task validation.
- The repo must ship a better first-run story.

## 18. Areas Where pengu mesh Already Looks Strong

- Native stdio MCP is a first-class repo-owned product surface.
- Replay export is already stronger and more coherent than baseline.
- Artifact derivation and crop-grid support are already meaningful.
- Lease semantics are explicit instead of implied.
- Continuity semantics are now explicit instead of buried in logs.
- Health and doctor are better integrated into the repo’s truth model.
- Rust-first control plane keeps core behavior local and inspectable.
- The repo already values audit bundles rather than ad hoc console logs.
- The repo is already structured for bounded capture rather than unbounded evidence sprawl.
- The repo already treats Chrome Dev as the primary target instead of pretending every browser is equally important.

## 19. Areas Where pengu mesh Still Trails Or Risks Trailing

- Security and authenticated ownership.
- High-risk capability gating.
- First-run onboarding ergonomics.
- Operator console maturity.
- Task scheduler and queue semantics.
- Real-web scenario coverage breadth.
- Metrics durability and longitudinal analysis.
- Weak-prompt success rate measurement.
- Performance threshold enforcement.
- Repeated comparative audits against PinchTab.

## 20. Non-Negotiable Delivery Rules

- Keep docs, code, and verification synchronized.
- Publish evidence in the same pass as changes.
- Prefer bounded live tests over broad but shallow claims.
- Prefer contract-hardening over cosmetic prose.
- Prefer operator value over abstract completeness.
- Prefer stable typed outputs over stringly ambiguity.
- Prefer repeatable scripts over one-off manual success.
- Prefer current evidence over stale memory.
- Prefer direct runtime proof over architecture assumptions.
- Prefer measured latency over speculative speed claims.

## 21. Required Execution Loop For Every Serious Change

- Inspect the exact code path before changing it.
- Inspect the public contract before changing it.
- Inspect existing tests before changing it.
- Inspect release scripts before changing it.
- Inspect docs that claim current truth before changing it.
- State the intended change before editing.
- Make the smallest coherent change that solves the actual problem.
- Verify the local unit or contract surface immediately after editing.
- Verify the broader gate when the change touches public behavior.
- Publish artifacts if the change matters to operators.
- Narrow the claim if verification is weaker than the intended claim.
- Update the docs while the behavior is still fresh.

## 22. Required Live-Test Loop For Important Browser Changes

- Start with an isolated runtime root.
- Capture health before the live flow.
- Capture doctor before the live flow when readiness posture matters.
- Start or attach a real browser instance.
- Open a real tab against a real site when policy allows.
- Perform at least one state-changing action when action semantics changed.
- Capture text or snapshot output when read semantics changed.
- Capture screenshot output when rendering or artifact semantics changed.
- Stop the instance cleanly.
- Archive every important JSON output.
- Write a human-readable summary into the audit bundle.
- Record the exact command sequence used.

## 23. Required Multi-Modal Validation Rule

- Terminal output alone is not enough.
- JSON output alone is not enough.
- Screenshots alone are not enough.
- A serious live test should usually combine at least two modalities.
- For browser behavior, JSON plus screenshot is often the right minimum.
- For concurrency behavior, JSON plus log plus summary is often the right minimum.
- For attach continuity, health and doctor JSON are mandatory.
- For evidence capture, inspect the artifact and the metadata that points to it.
- For operator flows, record both machine-readable and human-readable results.
- If one modality disagrees with another, investigate before claiming success.

## 24. Required Contract-Hardening Rule

- Every public surface must have a typed contract.
- Every contract must have a source of truth in code.
- Every contract should be reflected in docs.
- Every contract should be reflected in at least one verification path.
- Every new field should justify itself with operator value, agent value, or debugging value.
- Avoid decorative fields that no test or operator would ever read.
- Prefer explicit enums over free-form strings for important outcomes.
- Prefer stable machine-readable categories over prose in JSON outputs.
- Prefer field addition over semantic overloading of existing fields.
- Preserve backward-compatible semantics when possible.
- When compatibility must change, document it in the same pass.

## 25. Required Fresh-Agent Usability Rule

- Assume the next agent may have poor context.
- Assume the next agent may receive a weak prompt.
- Assume the next agent may not know the repo layout.
- Assume the next agent may not know the product’s sharp edges.
- Design surfaces that reduce the need for repo-specific folklore.
- Design errors that tell the agent what to do next.
- Design diagnostics that reveal missing ownership, missing readiness, or missing permission clearly.
- Design commands and tools that preserve context-rich envelopes.
- Design docs that tell a fresh agent exactly what to run first.
- Measure how many extra steps a fresh agent needs to recover from a bad start.

## 26. Required Operator-Experience Rule

- The operator should know if the daemon is healthy.
- The operator should know if the browser is healthy.
- The operator should know if an attached browser is stale.
- The operator should know if a lease conflict is real and who owns the conflicting lease.
- The operator should know if a capture artifact exists and where it lives.
- The operator should know if a restart reused logical identity or reclaimed stale state.
- The operator should know if an action failure is browser-side, runtime-side, or policy-side.
- The operator should know which validations actually ran.
- The operator should know which claims remain provisional.
- The operator should not need to infer important runtime state from logs alone.

## 27. Required Metrics Program

- Use the durable metrics database as the source of truth for repeated validation.
- Store repeated validation results in that database.
- Store latency metrics in that database.
- Store failure classifications in that database.
- Store scenario outcomes in that database.
- Store agent-usability metrics in that database.
- Store operator-effort metrics in that database.
- Store network-effort metrics in that database.
- Store attach continuity classifications in that database.
- Store lease conflict and coexistence metrics in that database.
- Store environment fingerprint metadata in that database.
- Store benchmark versioning in that database.

## 28. Metrics Database Goals

- Preserve longitudinal truth.
- Enable regression detection over time.
- Enable scenario-by-scenario comparisons.
- Enable branch-by-branch comparisons when needed.
- Enable platform-by-platform comparisons later.
- Enable release candidate evaluation.
- Enable fresh-agent usability trend analysis.
- Enable poor-prompt recovery analysis.
- Enable performance threshold formalization.
- Enable “surpass PinchTab” claims backed by current data.

## 29. Suggested Metrics Database Shape

- A lightweight SQLite database is acceptable for the first iteration.
- Keep schema explicit and versioned.
- Prefer append-only run records with stable IDs.
- Prefer normalized tables when repeated joins are operationally useful.
- Prefer storing raw JSON payload paths instead of duplicating giant blobs.
- Preserve links from scenario runs to audit artifact directories.
- Preserve links from scenario runs to commit SHAs.
- Preserve links from scenario runs to platform fingerprint data.
- Preserve links from scenario runs to tool surface and protocol version.
- Preserve links from scenario runs to human-readable summaries.

## 30. Suggested Metrics Tables

- `scenario_runs`
- `scenario_steps`
- `scenario_assertions`
- `latency_samples`
- `network_samples`
- `resource_samples`
- `agent_effort_samples`
- `operator_effort_samples`
- `continuity_events`
- `lease_events`
- `failure_events`
- `artifacts`
- `environment_fingerprints`
- `pinchtab_comparisons`
- `benchmark_runs`

## 31. Suggested `scenario_runs` Columns

- `id`
- `scenario_name`
- `scenario_family`
- `scenario_version`
- `tool_surface`
- `runtime_root`
- `commit_sha`
- `branch_name`
- `platform`
- `started_at`
- `finished_at`
- `status`
- `summary_path`

## 32. Suggested `scenario_steps` Columns

- `id`
- `run_id`
- `ordinal`
- `step_name`
- `step_kind`
- `command_line`
- `request_path`
- `response_path`
- `started_at`
- `finished_at`
- `status`
- `error_code`

## 33. Suggested `scenario_assertions` Columns

- `id`
- `run_id`
- `step_id`
- `assertion_name`
- `expected_value`
- `actual_value`
- `status`
- `failure_category`
- `notes`

## 34. Suggested `latency_samples` Columns

- `id`
- `run_id`
- `step_id`
- `metric_name`
- `sample_ms`
- `p50_ms`
- `p95_ms`
- `p99_ms`
- `sample_count`
- `capture_method`

## 35. Suggested `network_samples` Columns

- `id`
- `run_id`
- `step_id`
- `request_count`
- `response_bytes`
- `request_bytes`
- `failed_request_count`
- `redirect_count`
- `cross_origin_count`
- `notes`

## 36. Suggested `resource_samples` Columns

- `id`
- `run_id`
- `step_id`
- `cpu_user_ms`
- `cpu_system_ms`
- `rss_bytes`
- `open_fd_count`
- `temp_file_bytes`
- `artifact_bytes`
- `notes`

## 37. Suggested `agent_effort_samples` Columns

- `id`
- `run_id`
- `prompt_quality_band`
- `retry_count`
- `correction_count`
- `extra_context_injections`
- `tool_call_count`
- `failed_tool_call_count`
- `manual_recovery_count`
- `notes`

## 38. Suggested `operator_effort_samples` Columns

- `id`
- `run_id`
- `manual_interventions`
- `minutes_to_diagnosis`
- `minutes_to_recovery`
- `log_files_opened`
- `json_files_opened`
- `screenshots_opened`
- `docs_consulted`
- `notes`

## 39. Suggested `continuity_events` Columns

- `id`
- `run_id`
- `instance_id`
- `outcome`
- `freshness`
- `browser_name`
- `browser_version`
- `debug_url`
- `browser_ws_url`
- `notes`

## 40. Suggested `lease_events` Columns

- `id`
- `run_id`
- `resource_kind`
- `resource_id`
- `holder_id`
- `mode`
- `event_kind`
- `conflicting_holder_id`
- `status`
- `notes`

## 41. Suggested `failure_events` Columns

- `id`
- `run_id`
- `step_id`
- `surface`
- `error_code`
- `error_family`
- `is_retryable`
- `is_policy_related`
- `is_browser_related`
- `notes`

## 42. Suggested `artifacts` Columns

- `id`
- `run_id`
- `step_id`
- `artifact_kind`
- `file_path`
- `sha256`
- `byte_count`
- `width`
- `height`
- `mime_type`
- `notes`

## 43. Suggested `environment_fingerprints` Columns

- `id`
- `run_id`
- `platform`
- `arch`
- `os_version`
- `rust_version`
- `cargo_version`
- `chrome_channel`
- `chrome_version`
- `hostname_hash`
- `notes`

## 44. Suggested `pinchtab_comparisons` Columns

- `id`
- `scenario_name`
- `scenario_version`
- `pengu_mesh_run_id`
- `pinchtab_run_id`
- `winner`
- `winner_reason`
- `pengu_mesh_summary_path`
- `pinchtab_summary_path`
- `notes`

## 45. Suggested `benchmark_runs` Columns

- `id`
- `benchmark_family`
- `benchmark_name`
- `platform`
- `commit_sha`
- `started_at`
- `finished_at`
- `status`
- `summary_path`
- `notes`

## 46. Metrics Collection Rules

- Store structured metrics for every named scenario family.
- Store versioned scenario definitions.
- Store both success and failure runs.
- Avoid only storing winners.
- Avoid hiding bad outliers.
- Keep the capture cost bounded.
- Prefer cheap incremental data collection over heroic manual collection.
- Ensure every metric can be traced back to a concrete artifact or JSON output.
- Ensure scenario versions change when semantics change.
- Ensure old results remain interpretable after schema growth.

## 47. Metrics Use Rules

- Use metrics to find regressions.
- Use metrics to find unclear workflows.
- Use metrics to identify repeated operator pain.
- Use metrics to identify repeated agent confusion.
- Use metrics to identify heavy network paths.
- Use metrics to identify high-latency operations.
- Use metrics to decide when performance thresholds are defensible.
- Use metrics to compare our flows with PinchTab flows.
- Use metrics to decide what should move into the gate.
- Use metrics to decide what docs are misleading.

## 48. Metrics Anti-Rules

- Do not collect vanity metrics with no decision use.
- Do not collect unbounded data that will rot.
- Do not collect huge raw payloads when file paths are enough.
- Do not collect metrics without scenario names and versions.
- Do not claim trend insight from one sample.
- Do not formalize thresholds from a single fast run.
- Do not hide failure samples from reports.
- Do not treat missing metrics as equivalent to good metrics.
- Do not make “surpass PinchTab” claims without a comparison artifact.
- Do not let the metrics database become a dead dump.

## 49. Required Performance Program

- Measure before enforcing.
- Enforce after stable baselines exist.
- Keep the first thresholds narrow and defensible.
- Start with hot paths already named in the repo.
- Start on `darwin/arm64`.
- Capture enough repeated samples to make p50 and p95 meaningful.
- Publish the benchmark context with every result bundle.
- Promote budgets into the gate only after noise bounds are understood.
- Fail the gate when the regression is clear and meaningful.
- Keep the thresholds reviewable in docs and code.

## 50. Early Candidate Performance Thresholds

- Daemon cold start time.
- Local health check latency.
- Doctor latency.
- `tab_action` dispatch overhead before browser latency.
- Snapshot and text capture overhead.
- Full-page screenshot overhead excluding browser render time where measurable.
- Event tail retrieval latency.
- Replay export manifest-only latency.
- Portable replay packaging latency.
- Artifact crop and crop-grid materialization latency.

## 51. Performance Evidence Rules

- Record repeated samples, not single anecdotes.
- Record the exact benchmark harness version.
- Record the exact runtime binary or commit under test.
- Record the browser build when browser behavior influences the sample.
- Record whether the browser was cold, warm, or already attached.
- Record whether the runtime root was clean or reused.
- Record enough context that the result can be repeated later.
- Record failure modes as first-class results.
- Record both median and tail latency where possible.
- Record artifact sizes when capture behavior is involved.

## 52. Performance Enforcement Rules

- Thresholds should start as warnings.
- Thresholds should graduate to failures only after repeated stability.
- Thresholds should be versioned in docs.
- Thresholds should be versioned in scripts when enforced.
- Threshold changes should be justified by evidence bundles.
- Threshold regressions should publish a failure explanation.
- Threshold waivers should expire.
- Threshold exceptions should identify the owning issue or follow-up path.
- Threshold enforcement should favor a few critical budgets over many noisy ones.
- Threshold enforcement should never silently disappear.

## 53. Required Live Scenario Families

- Browser startup scenarios.
- Managed-instance lifecycle scenarios.
- External-attach scenarios.
- Restart continuity scenarios.
- Endpoint-rotation scenarios.
- Stale-instance reclaim scenarios.
- Lease conflict scenarios.
- Lease coexistence scenarios.
- Real-web navigation scenarios.
- Real-web interaction scenarios.
- Evidence capture scenarios.
- Fresh-agent usability scenarios.
- Weak-prompt recovery scenarios.
- PinchTab comparison scenarios.
- Operator-diagnosis scenarios.

## 54. Real-Web Scenario Rule

- Use real public pages when safe and lawful.
- Prefer stable flows over fragile novelty sites for core regressions.
- Use at least one search-like workflow.
- Use at least one form-like workflow.
- Use at least one navigation-and-extract workflow.
- Use at least one capture-heavy workflow.
- Use at least one failure-recovery workflow.
- Use at least one awkward-layout workflow.
- Keep the scenarios meaningful for real agent tasks.
- Archive summaries with exact URLs and timestamps.

## 55. Fresh-Agent Scenario Rule

- Use concise prompts.
- Use ambiguous prompts.
- Use sloppy prompts.
- Use partially specified prompts.
- Use prompts that require recovery from missing detail.
- Use prompts that require inspection before acting.
- Use prompts that require extracting useful information from a page.
- Use prompts that require capturing evidence for handoff.
- Use prompts that require dealing with lease conflicts or attach context.
- Use prompts that reveal whether the product teaches the agent what to do next.

## 56. Poor-Prompt Quality Bands

- `clean`
- `concise`
- `underspecified`
- `sloppy`
- `confused`
- `incorrect-assumption`
- `multi-goal`
- `recovery-needed`
- `handoff-fragment`
- `swarm-fragment`

## 57. Operator-Diagnosis Scenario Rule

- Prove the operator can identify readiness state quickly.
- Prove the operator can identify stale attach state quickly.
- Prove the operator can identify writer conflicts quickly.
- Prove the operator can identify missing artifacts quickly.
- Prove the operator can identify endpoint rotation quickly.
- Prove the operator can identify daemon restart effects quickly.
- Prove the operator can identify bad policy configuration quickly.
- Prove the operator can identify whether a failure belongs to runtime or browser.
- Prove the operator can find the relevant artifacts without guesswork.
- Prove the operator does not need repo lore for basic diagnosis.

## 58. Live Scenario Catalog: Startup And Readiness

- Start daemon under a fresh runtime root and assert `health` becomes ready.
- Start daemon under a fresh runtime root and assert `doctor` reports expected environment truth.
- Start daemon twice against the same runtime root and assert the second start explains reuse or conflict clearly.
- Start daemon with no browser work and measure cold-start latency.
- Start daemon and immediately run `health`, `doctor`, and `events_tail`.
- Start daemon, stop daemon, and verify no stray process remains.
- Start daemon under an isolated temp root and verify artifacts stay inside the root.
- Start daemon with bench discovery available and verify no gate script regressed.
- Start daemon after a restart and verify continuity counters remain principled.
- Start daemon after deliberate stale state injection and verify stale classification appears.

## 59. Live Scenario Catalog: Managed Lifecycle

- Create a managed instance and verify instance listing reflects it.
- Open a managed tab and verify tab listing reflects it.
- Navigate a managed tab to a stable public page and verify text extraction.
- Close a managed tab and verify it disappears from the listing.
- Stop a managed instance and verify no stale live state remains.
- Restart a managed instance from the same profile and verify the identity behavior is documented.
- Capture a screenshot from a managed tab and verify artifact paths exist.
- Capture PDF from a managed tab and verify artifact paths exist.
- Capture a snapshot from a managed tab and verify bounds and visibility metadata exist.
- Capture text from a managed tab and verify content is non-empty and attributed.

## 60. Live Scenario Catalog: External Attach

- Attach to a browser with external attach disabled and verify policy rejection.
- Attach to a browser with external attach enabled and verify success.
- Attach twice to the same browser with stable endpoint evidence and verify logical reuse.
- Attach twice after browser websocket rotation and verify metadata refresh.
- Attach after stale endpoint evidence and verify freshness classification.
- Attach after stale instance state and verify stale reclaim classification.
- Attach after daemon restart and verify the continuity output is useful.
- Attach after the external browser is closed and verify stale classification.
- Attach after endpoint changes to a different browser and verify no incorrect reuse.
- Attach with malformed endpoint metadata and verify the failure is explicit.

## 61. Live Scenario Catalog: Lease Semantics

- Acquire a writer lease and verify ownership surfaces.
- Acquire an observer lease while a writer lease is held and verify coexistence.
- Attempt a second writer lease and verify typed conflict.
- Attempt a writer-only action without a writer lease and verify conflict.
- Attempt an observer-required action without observer access and verify conflict.
- Transfer or release leases and verify the state transitions are explicit.
- Verify CLI preserves typed lease conflict information.
- Verify MCP preserves typed lease conflict information.
- Verify HTTP preserves typed lease conflict information.
- Verify health and doctor show the coverage matrix that explains the rules.

## 62. Live Scenario Catalog: Real-Web Search Workflows

- Navigate to Google Images home and capture the landing page.
- Fill the Google Images search box and submit a query.
- Verify results page text contains the query intent.
- Capture a full-page screenshot of the results page.
- Capture an accessibility snapshot after results render.
- Open a secondary tab to a second results page and verify tab inventory remains coherent.
- Navigate backwards and verify the tab remains usable.
- Navigate forwards and verify the tab remains usable.
- Reload the results page and verify tab websocket recovery does not silently fail.
- Stop the instance and verify artifacts survive the session.

## 63. Live Scenario Catalog: Real-Web Form Workflows

- Navigate to a public example form page.
- Fill multiple inputs through `tab_action`.
- Use select controls where available.
- Submit the form when safe and verify a result page or confirmation state.
- Capture text before submission.
- Capture text after submission.
- Capture screenshot before submission.
- Capture screenshot after submission.
- Verify invalid selectors fail clearly.
- Verify bad sequencing does not corrupt subsequent actions.

## 64. Live Scenario Catalog: Real-Web Extraction Workflows

- Navigate to a documentation page and extract the main title.
- Extract a list of headings from a long page.
- Capture a screenshot of the top viewport.
- Capture a full-page screenshot of the full document.
- Capture text and compare it against the visible headings.
- Capture snapshot and compare bounds around the main content region.
- Navigate to a second page and compare extraction behavior.
- Verify extraction remains coherent after the page is reloaded.
- Verify extraction errors are explicit when the page fails to load.
- Verify outputs are small enough to remain bounded.

## 65. Live Scenario Catalog: Real-Web Media Workflows

- Navigate to a page with multiple images.
- Capture a screenshot with multiple image tiles visible.
- Capture text from the page and verify alt or label content where available.
- Use artifact crop to isolate one image tile from the screenshot.
- Use crop-grid derivation and verify a deterministic set of derivative artifacts.
- Verify artifact metadata points back to the original capture.
- Verify replay export includes the derived artifact lineage.
- Verify large media pages do not explode memory usage.
- Verify the runtime reports failures cleanly if the page blocks capture.
- Verify the operator can locate the artifacts quickly.

## 66. Live Scenario Catalog: Replay And Evidence

- Run a scenario that emits screenshots, snapshots, and text.
- Tail events during the run and verify ordering remains coherent.
- Export a manifest-only replay bundle and inspect it.
- Export a portable replay bundle and inspect it.
- Verify all referenced artifact files exist.
- Verify checksums are valid where documented.
- Verify replay export remains bounded.
- Verify replay output keeps run IDs and artifact provenance linked.
- Verify doctor can identify replay provenance issues when they are injected.
- Verify handoff-ready summaries remain human-readable.

## 67. Live Scenario Catalog: Restart Recovery

- Start a browser session, restart the daemon, and verify continuity state.
- Start a browser session, kill the browser, and verify stale classification.
- Start a capture flow, restart the daemon, and verify recovery semantics.
- Start an attach session, rotate endpoint metadata, and verify refresh semantics.
- Start an attach session, restart the daemon, and verify logical identity handling.
- Start a lease flow, restart the daemon, and verify recovered leases are classified correctly.
- Verify health after restart.
- Verify doctor after restart.
- Verify old artifacts remain reachable after restart.
- Verify the summary explains what was recovered and what was stale.

## 68. Live Scenario Catalog: Fresh-Agent Tasks

- Give a fresh agent a one-sentence prompt to fetch an image search page and summarize what it sees.
- Give a fresh agent a sloppy prompt to find a page, capture it, and return evidence.
- Give a fresh agent an underspecified prompt that requires reading `health` first.
- Give a fresh agent a prompt that implicitly requires attach rather than managed launch.
- Give a fresh agent a prompt that requires replay export for handoff.
- Give a fresh agent a prompt that requires dealing with a writer conflict.
- Give a fresh agent a prompt that requires comparing two pages.
- Give a fresh agent a prompt that requires capturing a full-page screenshot.
- Give a fresh agent a prompt that requires reading back text from a live page.
- Record what extra context injections were needed.

## 69. Live Scenario Catalog: Swarm Tasks

- Give one agent the writer role and one agent an observer role.
- Have the writer open and navigate a tab while the observer captures evidence.
- Have a second writer attempt a conflicting action and verify conflict surfaces.
- Have one agent export replay while another tails events.
- Have one agent inspect health while another captures screenshot.
- Have one agent diagnose a stale attach while another compares old and new doctor payloads.
- Have one agent update docs based on a failed live test while another gathers artifacts.
- Record where swarm coordination is still awkward.
- Record whether the product helped or hindered the swarm.
- Record what task-plane features are missing.

## 70. Live Scenario Catalog: Error Taxonomy

- Invalid instance ID.
- Invalid tab ID.
- Missing holder ID when required.
- Writer conflict.
- Observer conflict.
- Browser not ready.
- External attach policy disabled.
- Endpoint metadata mismatch.
- Stale instance.
- Browser websocket stale.

## 71. Live Scenario Catalog: More Error Taxonomy

- Selector not found.
- Timeout during navigation.
- Timeout during capture.
- Screenshot artifact missing on disk.
- Replay export missing artifact.
- Crop request outside bounds.
- Crop-grid request over input limits.
- Browser process exits unexpectedly.
- Daemon socket already bound.
- Runtime root state corruption.

## 72. Required Scenario Outcome Fields

- Scenario name.
- Scenario family.
- Scenario version.
- Runtime surface.
- Browser mode.
- Prompt quality band.
- Start time.
- End time.
- Overall status.
- Summary path.

## 73. Required Scenario Success Fields

- Time to first readiness.
- Time to first browser interaction.
- Time to first successful capture.
- Number of tool calls.
- Number of retries.
- Number of manual interventions.
- Number of artifacts.
- Number of failures recovered.
- Final task completeness.
- Final evidence completeness.

## 74. Required Scenario Failure Fields

- First failure point.
- First failure category.
- First failure surface.
- Human-readable explanation.
- Machine-readable error code.
- Recovery attempted or not.
- Recovery outcome.
- Artifacts collected before failure.
- Artifacts collected after failure.
- Follow-up work required.

## 75. Required PinchTab Comparison Families

- Basic readiness.
- Managed navigation.
- External attach.
- Screenshot capture.
- Text extraction.
- Restart recovery.
- Conflict clarity.
- Operator diagnosis.
- First-run onboarding.
- Weak-prompt task completion.

## 76. Rules For Fair PinchTab Comparison

- Use the same machine when possible.
- Use the same browser family when possible.
- Use the same class of real pages when possible.
- Record environment differences explicitly.
- Use comparable task definitions.
- Use comparable success criteria.
- Store both systems’ evidence bundles.
- Do not cherry-pick only the winning scenario.
- Do not call a tie a win.
- Do not call an unmeasured area a win.

## 77. Required Security Hardening Themes

- Authenticated local ownership.
- Capability gating for dangerous actions.
- Explicit attach allowlists where useful.
- Untrusted-content awareness.
- Better operator warnings around exposed control planes.
- Clear distinction between trusted-local and authenticated semantics.
- Audit logging that remains privacy-aware.
- Safer first-run defaults.
- Clear environment posture reporting.
- Principle-aligned documentation.

## 78. Required Operator Console Themes

- Health summary.
- Doctor summary.
- Live instance inventory.
- Live tab inventory.
- Lease inventory.
- Continuity inventory.
- Artifact inventory.
- Replay bundle inventory.
- Failure timeline.
- Scenario results dashboard.

## 79. Required Task Plane Themes

- Durable queueing.
- Holder-aware job ownership.
- Fairness or admission control.
- Cancellation.
- Retry classification.
- Artifact linkage.
- Scenario linkage.
- Failure linkage.
- Human-readable status.
- Machine-readable status.

## 80. Required First-Run Experience Themes

- One clear install path.
- One clear readiness command.
- One clear first browser task.
- One clear evidence capture task.
- One clear attach task.
- One clear doctor explanation.
- One clear troubleshooting path.
- One clear cleanup path.
- One clear scenario example.
- One clear handoff path.

## 81. Exact Files Changed In The Last Serious Runtime Hardening Pass

- `.codex/README.md`
- `.codex/config.toml`
- `.codex/agents/browser-operator.toml`
- `.codex/agents/cdp-attach-specialist.toml`
- `.codex/agents/concurrency-steward.toml`
- `.codex/agents/contract-keeper.toml`
- `.codex/agents/default.toml`
- `.codex/agents/explorer.toml`
- `.codex/agents/monitor.toml`
- `.codex/agents/observability-owner.toml`
- `.codex/agents/perf-bench-lead.toml`
- `.codex/agents/pinchtab-gap-hunter.toml`
- `.codex/agents/release-auditor.toml`
- `.codex/agents/reviewer.toml`
- `.codex/agents/runtime-owner.toml`
- `.codex/agents/worker.toml`
- `crates/pengu-mesh-cdp/src/lib.rs`
- `docs/agent-execution-charter.md`
- `docs/codex-agent-capability-map.md`
- `docs/upgrade-path.md`

## 82. Exact Runtime Concepts Landed In That Pass

- Shared repo defaults now live in `.codex/config.toml`, and per-role files only override settings that actually differ.
- New read-only `reviewer` and write-capable `observability_owner` roles now exist in the repo-local role pack.
- Explorer, monitor, and PinchTab gap-hunter roles now advertise read-only posture directly in config.
- Browser validation is now explicitly split between `cdp_attach_specialist` and `browser_operator`.
- The repo capability docs now describe the narrowed repo-scoped Codex defaults and the full current role pack.
- `CdpSession` now surfaces JavaScript exception text for `Runtime.evaluate` failures instead of swallowing it behind missing-value errors.
- `CdpSession` now exposes `navigate`, `insert_text`, `dispatch_key`, and full-page screenshot helpers.
- The worktree is no longer carrying this role-pack and CDP lane as uncommitted local drift.
- The permanent handoff charter was refreshed to match the pushed state and current audit bundles.

## 83. Exact Commands Already Proven In That Pass

- `cargo fmt --all --check`
- `cargo check --workspace`
- `cargo test --workspace`
- `./scripts/release/local-gate.sh`
- `git rev-parse HEAD`
- `git rev-parse origin/main`
- `git status --short --branch`
- `git push origin main`

## 84. Exact Commands Already Proven In The Real-Web Smoke

- `cargo run -p pengu-mesh -- instance-start --name live-web --channel chrome-dev --holder-id live-web-agent`
- `cargo run -p pengu-mesh -- tab-open --instance-id <instance_id> --url 'https://www.google.com/imghp?hl=en' --holder-id live-web-agent`
- `cargo run -p pengu-mesh -- tab-text --tab-id <tab_id> --holder-id live-web-agent`
- `cargo run -p pengu-mesh -- tab-snapshot --tab-id <tab_id> --holder-id live-web-agent`
- `cargo run -p pengu-mesh -- tab-screenshot --tab-id <tab_id> --full-page --holder-id live-web-agent`
- `cargo run -p pengu-mesh -- instance-stop --instance-id <instance_id> --holder-id live-web-agent`

## 85. Exact Commands Already Proven In The PinchTab Probe

- `./scripts/doctor.sh < /dev/null`
- `go test -tags integration ./tests/integration/ -run TestHealth -v -timeout 5m`
- `go test -tags integration ./tests/integration/ -run TestNavigate -v -timeout 5m`

## 86. Exact Failures Seen In The Hardening Pass

- The repo was still carrying a coherent but uncommitted Codex role-pack lane plus a pending `pengu-mesh-cdp` enhancement.
- Two new role files existed locally without being committed, which kept the worktree permanently dirty.
- The first cleanup commit message body lost the literal `.codex` token because the shell evaluated backticks inside the quoted command string.
- These failures were informative and should not be hidden.

## 87. Exact Fixes Applied For Those Failures

- Commit and push the entire role-pack and `pengu-mesh-cdp` lane as a verified cleanup instead of leaving it as ambient local drift.
- Add the missing `reviewer` and `observability_owner` role files to the repo-scoped role pack.
- Update the capability docs so they describe the actual repo-local Codex posture.
- Refresh this charter after the cleanup so the next agent starts from a clean-repo truth.
- Avoid backticks inside shell-wrapped commit messages so literal paths survive intact.
- Refresh this charter after checking local and remote repo state so the permanent handoff stays current.
- Preserve the failure history in the handoff rather than pretending the lane was smooth.

## 88. What Remains Unresolved After The Hardening Pass

- No authenticated holder identity.
- No capability gating for dangerous operations.
- No durable job scheduler plane.
- No operator console that leverages current doctor and replay strengths.
- No thresholded or comparative metrics program.
- No repeated weak-prompt or fresh-agent prompt corpus beyond the first shipped families.
- No performance thresholds enforced as gate failures.
- No repeated PinchTab comparison program beyond the first repo-owned scenario pack.
- No real packaging and first-run story that beats source-first setup.
- No stored agent-usability leaderboard.

## 89. The First Three Deliveries The Next Agent Should Attempt

- Delivery one should formalize a real-web scenario harness with stable scenario IDs, audit bundles, and reproducible outputs.
- Delivery two should add an operator-diagnosis scenario family with repeatable stored results.
- Delivery three should define the first thresholded performance budgets from repeated `darwin/arm64` measurements.

## 90. The Next Three Deliveries After That

- Delivery four should start authenticated local ownership and capability gating.
- Delivery five should build an operator console worth using every day.
- Delivery six should build a durable task plane above leases and below the eventual console.

## 91. First 24-Hour Plan For The Next Agent

- Read this charter.
- Read the current status doc.
- Read the current milestone plan.
- Read the observability and performance docs.
- Inspect the new smoke scripts.
- Inspect the lease matrix test.
- Inspect the latest audit bundles.
- Run or inspect the latest local-gate output and its `scenario-list.json`.
- Recapture or relocate the PinchTab probe into the repo-owned audit area if it is still misplaced.
- Decide the next family to add after `startup-readiness`, `evidence-chain`, `structured-failure`, `weak-prompt`, and `fresh-agent`.
- Decide the next family to add after `startup-readiness`, `evidence-chain`, `structured-failure`, `weak-prompt`, `fresh-agent`, and `pinchtab-comparison`.
- Publish a bounded implementation pass with evidence.

## 92. First 72-Hour Plan For The Next Agent

- Land at least one real-web scenario family.
- Land at least one operator-diagnosis scenario family.
- Land repeatable weak-prompt and fresh-agent prompt packs.
- Record longitudinal data instead of one-off summaries.
- Update the docs to reflect the new source of truth.
- Expand the gate beyond `startup-readiness` only when the next family has repeated green proof.
- Publish a fresh audit bundle with measured results.
- Update this charter or replace it with a stronger version.

## 93. Two-Week Plan Seed

- Formalize the first performance thresholds.
- Land the first authenticated ownership design.
- Land the first capability-gating design.
- Land the first operator console prototype backed by existing runtime truth.
- Promote the comparative PinchTab scenario pack into repeated leaderboard or gate use.
- Land a scenario leaderboard backed by stored metrics.
- Land repeatable weak-prompt and fresh-agent prompt packs.
- Land replay-linked scenario drill-down views.
- Land stronger first-run docs or packaging.
- Land a stricter production gate.

## 94. Questions The Next Agent Should Ask The Code, Not The User

- Which public operations still lack strong live coverage.
- Which outputs are still human-readable but not machine-usable.
- Which failures still require log spelunking.
- Which scenario families can be added with the least architecture churn.
- Which performance numbers are already stable enough to track.
- Which docs still promise too little or too much.
- Which current surfaces confuse a fresh agent.
- Which current errors require better next-step guidance.
- Which current scripts are too ad hoc to serve as real harnesses.
- Which current strengths can become undeniable product advantages.

## 95. Questions The Next Agent Should Answer With Evidence

- How fast is readiness really.
- How fast is first successful action really.
- How fast is first meaningful capture really.
- How often does a weak prompt still succeed.
- How many corrections does a fresh agent need.
- How quickly can an operator diagnose stale attach state.
- How clear are lease conflicts in practice.
- How much slower or faster is pengu mesh than PinchTab on named tasks.
- Which task families already favor pengu mesh.
- Which task families still favor PinchTab.

## 96. Evidence Bundle Minimum For A Serious Future Pass

- One summary markdown file.
- One machine-readable health output.
- One machine-readable doctor output.
- One scenario artifact directory.
- One command log or direct command list.
- One failure note if something broke.
- One doc update set.
- One current-status update if truth changed materially.
- One milestone-plan update if roadmap truth changed materially.
- One pointer from the relevant docs back to the new bundle.

## 97. A Future Pass Is Not Complete If

- It changes behavior but does not publish evidence.
- It adds a public surface but does not update the lease model.
- It adds a public surface but does not update the docs.
- It adds a scenario harness but does not record named results.
- It adds metrics but does not explain how they will be used.
- It changes attach semantics but does not update health and doctor.
- It changes agent UX but does not test with a fresh-agent style prompt.
- It changes performance posture but does not benchmark.
- It claims superiority to PinchTab but does not compare the systems.
- It leaves stale status language behind.

## 98. A Future Pass Is Strong If

- It closes a meaningful product gap.
- It improves operator clarity.
- It improves fresh-agent clarity.
- It adds durable proof.
- It reduces repeated confusion.
- It reduces repeated runtime fragility.
- It formalizes a previously vague standard.
- It turns a prose goal into a measurable result.
- It sharpens a gate.
- It leaves the repo easier to continue.

## 99. A Future Pass Is Excellent If

- It hardens one of the core product truths.
- It adds real-world scenario evidence.
- It creates a repeatable harness rather than a one-off demo.
- It improves both the runtime and the explanation of the runtime.
- It reduces ambiguity for the next agent.
- It improves the comparative position against PinchTab.
- It leaves behind metrics instead of memory alone.
- It turns failure observations into improved standards.
- It makes the product easier to trust.
- It makes the product harder to misunderstand.

## 100. Closing Instruction

- Use this document as a starting contract, not a museum artifact.
- Replace it only with something stronger.
- Update it when the repo’s truth changes materially.
- Cite current evidence when you change it.
- Keep the repo aimed at measurable product superiority, not local maximums of elegance.
- Keep live tests regular.
- Keep performance measurement real.
- Keep operator clarity central.
- Keep fresh-agent usability central.
- Keep pushing until the repo is better in ways that survive scrutiny.
