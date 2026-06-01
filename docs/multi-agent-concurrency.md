# Multi-Agent Concurrency

The runtime now ships a real first-pass coordination model:

- durable SQLite-backed leases scoped to browser instances
- shared observer leases for read-mostly work
- exclusive writer leases for mutation paths
- writer transfer between holders without tearing down instance identity
- TTL-based expiry and lease renewal
- typed conflict reporting through CLI, MCP, HTTP, and event logs
- transactionally serialized writer acquisition in the state layer

Holder IDs are cooperative coordination identifiers inside a trusted local
operator boundary. They are not authentication credentials.

## Coverage Matrix

| Operation | CLI | MCP | HTTP | Lease posture | Notes |
| --- | --- | --- | --- | --- | --- |
| browser_health | `pengu-mesh health` | `browser_health` | `GET /health` | outside model | runtime-wide readiness summary |
| browser_doctor | `pengu-mesh-doctor -- --json` | `browser_doctor` | `GET /doctor` | outside model | runtime-wide diagnostics |
| diagnose | `pengu-mesh diagnose` | `diagnose` | `GET /diagnose` | outside model | side-effect-free readiness and remediation inventory |
| host_access_status | `pengu-mesh host-access-status` | `host_access_status` | `GET /host/access/status` | outside model | machine permission and channel readiness inventory |
| host_access_setup | `pengu-mesh host-access-setup ...` | `host_access_setup` | `POST /host/access/setup` | outside model | machine permission/setup administration |
| profile_list | `pengu-mesh profile-list` | `profile_list` | `GET /profiles` | outside model | local profile inventory |
| profile_create | `pengu-mesh profile-create --name ...` | `profile_create` | `POST /profiles/create` | outside model | local profile creation before any instance exists |
| instance_list | `pengu-mesh instance-list` | `instance_list` | `GET /instances` | outside model | aggregate readiness inventory across the runtime |
| instance_start | `pengu-mesh instance-start ...` | `instance_start` | `POST /instances/start` | writer required | launch creates a new managed instance and writer lease |
| instance_attach | `pengu-mesh instance-attach ...` | `instance_attach` | `POST /instances/attach` | writer required | attach mutates logical instance and continuity state |
| instance_stop | `pengu-mesh instance-stop ...` | `instance_stop` | `POST /instances/stop` | writer required | stop mutates live browser state |
| lease_status | `pengu-mesh lease-status ...` | `lease_status` | `GET /leases` | outside model | coordination-plane inspection |
| lease_acquire | `pengu-mesh lease-acquire ...` | `lease_acquire` | `POST /leases/acquire` | outside model | coordination-plane administration |
| lease_release | `pengu-mesh lease-release ...` | `lease_release` | `POST /leases/release` | outside model | coordination-plane administration |
| lease_transfer | `pengu-mesh lease-transfer ...` | `lease_transfer` | `POST /leases/transfer` | outside model | coordination-plane administration |
| tab_list | `pengu-mesh tab-list ...` | `tab_list` | `GET /tabs` | observer required | refreshes live tab inventory |
| tab_list_actions | `pengu-mesh tab-list-actions ...` | `tab_list_actions` | `GET /tabs/actions` | observer required | describes tab affordances before mutation |
| browser_surface_list | `pengu-mesh browser-surface-list ...` | `browser_surface_list` | `GET /browser/surfaces` | observer required | reads browser-native AX surface state |
| browser_surface_list_actions | `pengu-mesh browser-surface-list-actions ...` | `browser_surface_list_actions` | `GET /browser/surfaces/actions` | observer required | describes action affordances before mutation |
| browser_surface_snapshot | `pengu-mesh browser-surface-snapshot ...` | `browser_surface_snapshot` | `POST /browser/surfaces/snapshot` | observer required | reads native surface state and emits evidence artifacts |
| browser_surface_action | `pengu-mesh browser-surface-action ...` | `browser_surface_action` | `POST /browser/surfaces/action` | writer required | mutates browser-native controls through the macOS substrate |
| tab_open | `pengu-mesh tab-open ...` | `tab_open` | `POST /tabs/open` | writer required | mutates target set |
| tab_close | `pengu-mesh tab-close ...` | `tab_close` | `POST /tabs/close` | writer required | mutates target set |
| tab_action | `pengu-mesh tab-action ...` | `tab_action` | `POST /tabs/action` | writer required | mutates navigation, focus, DOM, or inputs |
| tab_snapshot | `pengu-mesh tab-snapshot ...` | `tab_snapshot` | `POST /tabs/snapshot` | observer required | reads live page state and emits an artifact |
| tab_text | `pengu-mesh tab-text ...` | `tab_text` | `POST /tabs/text` | observer required | reads live page state and emits an artifact |
| tab_screenshot | `pengu-mesh tab-screenshot ...` | `tab_screenshot` | `POST /tabs/screenshot` | observer required | reads live page state and emits an artifact |
| tab_pdf | `pengu-mesh tab-pdf ...` | `tab_pdf` | `POST /tabs/pdf` | observer required | reads live page state and emits an artifact |
| artifact_list | `pengu-mesh artifact-list ...` | `artifact_list` | `GET /artifacts` | outside model | immutable artifact inventory with sha256 and size_bytes metadata |
| artifact_verify | `pengu-mesh artifact-verify ...` | `artifact_verify` | `GET /artifacts/verify` | outside model | side-effect-free hash verification against stored artifact metadata |
| artifact_crop | `pengu-mesh artifact-crop ...` | `artifact_crop` | `POST /artifacts/crop` | observer required | derives a crop from shared artifact state |
| artifact_crop_grid | `pengu-mesh artifact-crop-grid ...` | `artifact_crop_grid` | `POST /artifacts/crop-grid` | observer required | derives bounded crop batches from shared artifact state |
| artifact_handle | none | none | `GET /artifacts/:id` | outside model | immutable artifact metadata lookup |
| capture_start_recording | `pengu-mesh capture-start-recording` | `capture_start_recording` | `POST /capture/start` | outside model | runtime-owned observability metadata |
| capture_stop_recording | `pengu-mesh capture-stop-recording` | `capture_stop_recording` | `POST /capture/stop` | outside model | runtime-owned observability metadata |
| run_list | `pengu-mesh run-list` | `run_list` | `GET /runs` | outside model | immutable replay inventory |
| events_tail | `pengu-mesh events-tail` | `events_tail` | `GET /events` | outside model | append-only event inspection |
| replay_export | `pengu-mesh replay-export` | `replay_export` | `POST /replay/export` | outside model | replay packaging after capture |
| trace_capture | `pengu-mesh trace-capture ...` | `trace_capture` | `POST /trace/capture` | observer required | bounded live evidence capture |
| recording_capture | `pengu-mesh recording-capture ...` | `recording_capture` | `POST /recording/capture` | observer required | bounded live evidence capture |
| tool_catalog | none | none | `GET /tools` | outside model | generic catalog route |
| generic_tool_dispatch | none | none | `POST /tools/:tool` | outside model | delegates to the underlying tool policy |

## Failure Contracts

- `instance_start`, `instance_attach`, `instance_stop`, `profile_create`,
  `host_access_setup`, `lease_status`, `lease_acquire`, `lease_release`,
  `lease_transfer`, `capture_start_recording`, `capture_stop_recording`,
  `run_list`, `events_tail`, and `replay_export` now return a typed
  `OperationFailurePayload` on runtime failures instead of an opaque error
  string inside an empty envelope.
- `trace_capture` and `recording_capture` now follow the same `TabFailurePayload`
  contract as the other tab-scoped evidence tools.
- `events_tail --run-id <missing>` now fails honestly with `code: not_found`
  instead of succeeding with `run: null`.

## Enforcement Rules

- mutation paths require a writer lease
- shared-read browser and artifact paths with concurrency impact require an
  observer lease
- when no lease exists yet, the runtime auto-acquires the required writer or
  observer lease for the current holder
- when a writer lease exists, additional observers may coexist, but non-holder
  mutations fail with a typed `conflict`
- writer acquisition is serialized through a single SQLite transaction path
  instead of a read-check-write race

## Verification

- `cargo test -p pengu-mesh-state --lib`
- `cargo test -p pengu-mesh-core --lib`
- `cargo test -p pengu-mesh --test lease_matrix_contract`
- `./scripts/release/lease-smoke.sh`
- `./scripts/release/tab-lifecycle-integration.sh`
- `./scripts/release/evidence-chain-smoke.sh`
- `./scripts/release/browser-surface-smoke.sh`

No agent should need to guess whether it owns a tab, capture, or browser
mutation path.
