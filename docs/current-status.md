# Current Status

pengu mesh treats `main` as the single production and implementation branch.

The currently shipped implementation is browser-first, but the product identity
is broader: `pengu mesh` is the local meshing harness around agent access and
truthful control surfaces, not a single-purpose browser tool.

The authoritative doc chain is:

- `README.md` for landing-page posture
- `docs/current-status.md` for short shipped/deferred truth
- `docs/feature-file-map.md` for file ownership and gap locations
- `docs/implementation-backlog.md` for tracked next work
- `docs/repo-hygiene-plan.md` for proof retention and repo cleanup policy
- `docs/milestone-plan.md` for staged roadmap
- `docs/autonomous-operating-model.md` for subagent lanes and handoff rules
- `docs/agent-execution-charter.md` for dense execution and handoff detail

## Shipped on `main`

- Stage 1 runtime on `darwin/arm64`
  - managed Chrome launch
  - instance and tab lifecycle
  - managed profile creation and inventory
  - agent-facing readiness diagnostics through `diagnose`
  - host-access capability matrix and setup flow for macOS-native browser control
  - operator-facing `pengu-mesh-doctor --setup-wizard` flow for read-only
    host-access walkthroughs with remediation commands and settings deeplinks
  - native browser-surface listing, snapshot, and action flows
  - typed tab actions: `navigate`, `evaluate`, `click`, `focus`, `hover`,
    `fill`, `type`, `press`, and `select`
  - richer accessibility snapshot output with viewport, visibility,
    active-target, and bounds metadata
  - full-page screenshot capture in addition to viewport screenshot capture
  - text and PDF capture
- long-lived daemon and local HTTP control plane
  - `pengu-mesh serve`
  - explicit route inventory, including generic `/tools` catalog and dispatch
  - daemon metadata surfaced in health and doctor
  - `pengu-mesh-doctor` follows daemon continuity state when daemon metadata is present
  - built-in capability risk posture surfaced in health and doctor, including
    safe/elevated/dangerous tier counts and policy decisions
  - read-only capability preflight over CLI, MCP, and HTTP at
    `/capabilities/preflight`, including per-capability allow/deny decisions
    and exact `PENGU_MESH_CAPABILITY_GRANTS=<capability>` hints
  - explicit `PENGU_MESH_CAPABILITY_GRANTS` enforcement for
    `host_access_setup` apply mode and browser-surface actions that permit
    global takeover
- lease coordination for the current public browser and evidence surface
  - durable SQLite-backed writer and observer leases
  - transactionally serialized writer acquisition in the state layer
  - typed conflict reporting preserved across CLI, MCP, and HTTP
  - lease coverage matrix surfaced in health and doctor
  - current writer-required paths:
    `instance_start`, `instance_attach`, `instance_stop`,
    `browser_surface_action`, `tab_open`, `tab_close`, `tab_action`
  - current observer-required paths:
    `tab_list`, `browser_surface_list`, `browser_surface_snapshot`,
    `tab_snapshot`, `tab_text`, `tab_screenshot`, `tab_pdf`,
    `artifact_crop`, `artifact_crop_grid`, `trace_capture`,
    `recording_capture`
  - current intentionally-outside paths:
    `browser_health`, `browser_doctor`, `capability_preflight`,
    `host_access_status`,
    `host_access_setup`, `profile_list`, `profile_create`,
    `instance_list`, all lease-admin operations, `artifact_handle`,
    `capture_start_recording`, `capture_stop_recording`, `run_list`,
    `scenario_list`, `scenario_summary`, `scenario_gate`,
    `scenario_run_detail`,
    `events_tail`, `replay_export`, generic tool catalog, and generic tool
    dispatch
- daemon continuity and attach continuity
  - stable daemon operator identity per runtime root
  - active capture run recovery on daemon restart
  - daemon continuity counters scoped to daemon-owned recovered leases
  - stale daemon-owned instance classification surfaced in health and doctor
  - exact endpoint reuse plus stale reclaim when browser websocket evidence drifts
  - explicit attach continuity outcome and freshness in health and doctor:
    `new_instance`, `reused_existing_instance`,
    `reclaimed_stale_instance`, plus `live`, `stale_instance`, and
    `stale_endpoint`
  - name-only attach reuse removed; logical identity now reuses only when
    endpoint evidence matches
  - best-effort tab websocket recovery after refresh for action and capture
    paths
  - failed attach attempts do not publish new continuity success state
- Stage 2 observability and replay
  - capture runs at runtime boot
  - append-only SQLite event timeline
  - replay export in `manifest_only` and `portable` modes
  - artifact-to-run correlation
  - artifact integrity verification through `artifact_verify`
  - artifact list inventory with `sha256` and `size_bytes`
  - artifact crop
  - artifact crop grid
  - trace capture
  - recording capture
  - structured failure payload coverage across the MCP dispatcher for current
    tab, artifact, browser-surface, and operation families
- Scenario metrics and first workflow corpus
  - durable SQLite-backed scenario tables for runs, steps, assertions,
    latency, and environment fingerprints
  - runtime query surfaces: `scenario_list`, `scenario_summary`,
    `scenario_gate`, and `scenario_run_detail`
  - scenario summary aggregation by family, status, assertion failure count,
    latency min/median/max, latest run, and latest commit
  - scenario evidence gate over stored runs with policy checks for minimum
    run count, latest status, latest evidence freshness, assertion failures,
    required latency samples, and latency thresholds
  - recorder helpers and CLI shims for shell-driven scenario logging
  - first named workflow families under `examples/workflows/`:
    `startup-readiness`, `evidence-chain`, `structured-failure`, `live-web`,
    `weak-prompt`, `fresh-agent`, `operator-diagnosis`, and
    `pinchtab-comparison`
  - structured PinchTab comparison output tied to commit, branch, platform,
    screenshot artifact metadata, and static comparison-target constants
- local production gate
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
  - `make local-gate`
  - `cargo fmt --all --check`
  - `cargo check --workspace`
  - `cargo test --workspace`
  - bench discovery and bench compilation
  - narrow benchmark-threshold enforcement over `benches/thresholds.json` via
    `scripts/bench/threshold-check.sh`
  - isolated-runtime `health` and `doctor` JSON capture
  - bounded lease conflict/coexistence proof over HTTP
  - bounded daemon restart continuity proof over HTTP
  - bounded attach continuity proof over restart, endpoint rotation, and stale reclaim
  - bounded host-access capability proof with settings deeplink inventory
  - bounded browser-lifecycle proof over attach plus native surface capture
  - bounded tab lifecycle proof over navigate, evaluate, snapshot, screenshot,
    text, artifact inventory, and artifact verification
  - bounded evidence-chain corruption scenario proof with persisted snapshot
    JSON reopening, post-corruption invalidation, and scenario-gate inventory
  - bounded native browser-surface proof with fallback and takeover telemetry
  - bounded startup-readiness scenario proof with stored scenario detail and
    scenario-list, scenario-summary, and scenario-gate inventory
  - multi-family scenario-gate manifest proof for `startup-readiness`,
    `evidence-chain`, `operator-diagnosis`, `structured-failure`, and
    `weak-prompt`, including freshness ceilings for the latest run in each
    family
- autonomous operating model
  - role lanes for proof orchestration, runtime contracts, access ownership,
    browser reality, scenario evidence, metrics comparison, and release audit
  - durable handoff schema under `docs/agent-handoffs/`
  - this is a docs/process layer until the future durable task plane backs it
    with runtime records
- read-only operator console scaffold
  - Vite and React app under `web/dashboard/`
  - typed `/health` consumer showing runtime readiness, continuity, host
    access, route inventory, browser inventory, lease coverage, and capability
    risk posture
  - development proxy support for the local HTTP control plane

## Explicitly deferred on `main`

- authenticated holder identity beyond the current trusted-local coordination model
- broader enforced capability gating beyond the current `host_access_setup`
  apply and browser-surface global-takeover gates
- lease coverage for any future shared resources beyond the current public surface
- chunked or streamed heavyweight capture paths for very large live pages
- broader thresholded performance budgets beyond the current narrow benchmark
  manifest
- operator console beyond the current read-only health scaffold in
  `web/dashboard/`; runtime-backed replay, lease, continuity, and task views
  are still deferred
- durable task plane beyond the current lease primitives, including task
  scheduling and allocation policies
- broader repeated live-web scenario coverage plus broader weak-prompt,
  fresh-agent, and operator-diagnosis packs and repeated PinchTab comparisons
  beyond the first stored harness
- broader desktop-control plane beyond browser-native macOS surfaces
- compiled Apple framework shims instead of the current Python bridge

## Public contract notes

- Holder IDs are cooperative coordination identifiers inside a trusted local
  operator boundary. They are not authentication credentials.
- `diagnose` is the agent-facing readiness surface. It is expected to stay
  side-effect-free and machine-readable.
- `pengu-mesh-doctor` is the operator-facing readiness and health surface. It
  is expected to stay human-readable and truthful.
- `pengu-mesh-doctor --setup-wizard` remains read-only. It reports current
  host-access checks, remediation commands, and settings URLs without opening
  settings or requesting permissions.
- External attach remains opt-in behind `PENGU_MESH_ALLOW_EXTERNAL_ATTACH=1`.
- Health and doctor are the operator truth surfaces for lease posture,
  daemon continuity, attach continuity classification, host-access posture, and
  capability risk posture.
- Capability preflight is the agent-facing read-only truth surface for
  deciding whether a capability is currently allowed and which explicit grant
  to request before a trusted local mutation.
- `web/dashboard/` is a read-only `/health` consumer; it does not own runtime
  truth or mutate the control plane.
- MCP and HTTP preserve typed caller/readiness failures for duplicate profiles,
  tab target lookup mistakes, malformed attach URLs, native browser-surface
  lookup mistakes, incomplete attach endpoints, and current operation-level
  failures.
- Unsupported schema enum values are rejected as `invalid_input`; they are not
  silently coerced to defaults.
- Direct standalone CLI invocations do not share daemon continuity. When a
  script shells out repeatedly, cross-command artifact inventory should use
  `artifact_list --instance-id`, while `artifact_list --run-id` is only for the
  single invocation that produced that artifact batch.

## Proof Location Policy

- Raw proof belongs under ignored local report paths such as
  `reports/audit/` and `reports/local-gate/`.
- Temporary live-run output may exist under an external `output_dir` during the
  active session.
- Commit only deliberate summaries or small reviewed artifacts that do not
  expose private pages, local usernames, machine posture, browser profiles,
  tokens, cookies, or absolute host paths.
- The latest committed repo-local visual verification report is
  `reports/visual-verification-20260312T172200Z.md`.
- Heavy browser-profile caches and full gate bundles are local-only working
  state. Prune or archive them per `docs/repo-hygiene-plan.md` before treating
  a result as retained evidence.

## Latest verification floor

The current continuation baseline is expected to be verified with:

- `cargo fmt --all --check`
- `cargo check --workspace`
- `cargo test --workspace`
- `./scripts/release/local-gate.sh`
- `cargo run -p pengu-mesh -- diagnose`
- `cargo run -p pengu-mesh -- health`
- `cargo run -p pengu-mesh-doctor -- --json`
- `cargo run -p pengu-mesh-doctor -- --setup-wizard`
- `./scripts/release/diagnose-smoke.sh`
- `./scripts/release/lease-smoke.sh`
- `./scripts/release/continuity-smoke.sh`
- `./scripts/release/attach-continuity-smoke.sh`
- `./scripts/release/host-access-smoke.sh`
- `./scripts/release/browser-lifecycle-integration.sh`
- `./scripts/release/tab-lifecycle-integration.sh`
- `./scripts/release/evidence-chain-smoke.sh`
- `./scripts/release/browser-surface-smoke.sh`
- `npm --prefix web/dashboard run build`
- visual inspection of the relevant screenshot or native-surface capture
  artifacts when browser-facing proof is part of the run

## Next implementation focus

1. extend the current named scenario corpus with broader live-web drills,
   deeper weak-prompt, fresh-agent, and operator-diagnosis packs, and repeated
   PinchTab comparison coverage
2. broaden the current threshold manifest into a defensible thresholded
   performance program only after repeated `darwin/arm64` evidence exists
3. design authenticated holder ownership plus broader enforced capability
   gating on top of the current trusted-local coordination model and first
   explicit dangerous-capability gates
4. expand the read-only dashboard scaffold only after it has runtime-backed
   health, replay, lease, continuity, and task-plane operator value
5. stand up durable task-plane infrastructure on top of the current lease
   primitives, including task queueing, scheduling, and allocation policies
6. close the remaining named PinchTab parity gaps on `main`: semantic element
   matching, human-like input emulation, ad-block and tracker filtering,
   animation suppression, auto-restart strategies, HTTP proxy forwarding, and
   webhook notifications
