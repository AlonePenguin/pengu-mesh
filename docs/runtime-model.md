# Runtime Model

- `pengu-mesh` is the operator entrypoint for managed launch, attach, tab lifecycle,
  snapshot/text extraction, screenshot/PDF capture, and long-lived daemon mode
  via `pengu-mesh serve`.
- `pengu-mesh-mcp` is the native stdio MCP runtime and exposes the same runtime
  contract through `tools/list` and `tools/call`.
- `pengu-mesh diagnose` is the agent-facing readiness and remediation surface.
- `pengu-mesh-doctor` verifies environment, browser discovery, permission posture,
  runtime paths, lease posture, and current instance state, and
  `pengu-mesh-doctor --setup-wizard` walks host-access prerequisites with
  truthful checks, remediation commands, and settings deeplinks without side
  effects.
- `pengu-mesh-cli` is the lighter operator shell for contract inspection and direct
  tool invocation.
- the local HTTP control plane runs through the same shared runtime contract as
  CLI, MCP, and doctor.

The current runtime is browser-first, but `pengu mesh` is not scoped as a
browser-only tool. The architecture is meant to support broader agent-access
meshing across native apps, OS layers, MCP servers, HTTP surfaces, and other
local control planes without weakening typed contracts or diagnostic truth.

Lasting runtime principles:

- robustness over optimistic shortcutting
- diagnostic surfaces stay read-only and side-effect free
- agent self-enablement should stay structurally possible as permission/setup
  flows deepen

The runtime source of truth is shared across the workspace:

- browser discovery and launch policy live in `pengu-mesh-cdp`
- runtime orchestration lives in `pengu-mesh-core`
- macOS-native host access and browser-surface control live in `pengu-mesh-macos`
- SQLite-backed runtime state lives in `pengu-mesh-state`
- artifact streaming and handles live in `pengu-mesh-artifacts`

On macOS, `pengu-mesh-cdp` still owns the narrow native-dialog recovery path for the
Chrome Dev remote-debugging sheet, but broader machine permission inventory,
native browser-surface discovery, native snapshot capture, and native
browser-surface actions now live in `pengu-mesh-macos`.

Stage 1 uses a local SQLite state store plus on-disk artifacts under
`target/pengu-mesh-runtime/`.

## Current Stage 2 slice

- a capture run created at runtime boot
- an append-only `events` timeline stored in SQLite
- `artifacts.run_id` correlation plus artifact provenance for
  snapshot/text/screenshot/PDF outputs
- replay export in `manifest_only` and `portable` modes under the runtime root
- derived `artifact_crop` and `artifact_crop_grid` outputs for bounded
  screenshot/PDF inspection
- full-page screenshot capture
- bounded `trace_capture` and `recording_capture` flows linked to the active run
- durable SQLite-backed lease coordination with writer and observer modes
- transactionally serialized writer acquisition
- lease visibility through health, doctor, CLI, MCP, and HTTP
- lease conflict reporting on protected browser and evidence-capture paths
- a surfaced lease coverage matrix for every current public operation
- daemon restart continuity for the single-runtime-root model
- recovery of the daemon-owned active run and daemon-owned non-expired leases
  after restart
- daemon continuity counters that stay scoped to daemon-owned recovered leases
  even while health and doctor still expose the full shared lease set
- stale daemon-owned instance classification when prior endpoints no longer respond
- attach continuity outcome and freshness in health and doctor
- endpoint-evidence attach reuse with browser websocket refresh on rotation
- best-effort tab websocket recovery after a refresh when persisted target
  websockets drift
- doctor validation of recent replay bundle completeness and checksum integrity
- best-effort native Chrome Dev remote-debugging sheet recovery during local
  readiness polling
- host-access status and setup flows for Accessibility, Screen Capture, Listen
  Event, Apple Events, and DevToolsSecurity
- surfaced execution-channel readiness across `cdp`, `ax_direct`,
  `apple_events_activation`, `app_scoped_key_post`, and `global_takeover`
- native browser-surface list, snapshot, and action flows wired through CLI,
  MCP, HTTP, health, and doctor
- native browser-surface snapshot artifacts plus native capture artifacts when
  a browser window image is available

## Coordination boundary

- Holder IDs are cooperative coordination identifiers inside a trusted local
  operator boundary.
- The current runtime does not treat holder IDs as authenticated identities.
- Lease posture is meant to prevent accidental contention across local agents,
  not to authorize hostile callers.

## Production gate

The repo-owned gate currently expects:

- `cargo fmt --all --check`
- `cargo check --workspace`
- `cargo test --workspace`
- diagnose smoke
- bench discovery
- bench compilation
- lease smoke
- continuity smoke
- attach continuity smoke
- host-access smoke
- browser-lifecycle integration
- tab-lifecycle integration
- evidence-chain smoke
- browser-surface smoke
- local health check
- local doctor check

Thresholded performance failures remain deferred until stable baselines are
formalized.
