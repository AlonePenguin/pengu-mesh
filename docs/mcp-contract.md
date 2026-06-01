# MCP Contract

## Core tools

The current native stdio MCP catalog is executable, not placeholder-only. The
runtime-backed tools are:

- `browser_health`
- `browser_doctor`
- `diagnose`
- `capability_preflight`
- `host_access_status`
- `host_access_setup`
- `profile_list`
- `profile_create`
- `instance_list`
- `instance_start`
- `instance_stop`
- `instance_attach`
- `lease_status`
- `lease_acquire`
- `lease_release`
- `lease_transfer`
- `tab_list`
- `browser_surface_list`
- `browser_surface_list_actions`
- `browser_surface_snapshot`
- `browser_surface_action`
- `tab_open`
- `tab_list_actions`
- `tab_close`
- `tab_action`
- `tab_snapshot`
- `tab_text`
- `tab_screenshot`
- `tab_pdf`
- `artifact_list`
- `artifact_verify`
- `artifact_crop`
- `artifact_crop_grid`
- `capture_start_recording`
- `capture_stop_recording`
- `run_list`
- `events_tail`
- `replay_export`
- `trace_capture`
- `recording_capture`

## Response envelope

Every tool response carries:

- `ok`
- `code`
- `message`
- `timestamp`
- `data`

`browser_health` now preserves the runtime readiness envelope instead of
flattening health into an unconditional success response. MCP structured output
also preserves the full envelope rather than only the inner `data` payload.
Known-tool caller and runtime failures are expected to stay inside that
envelope instead of surfacing as transport-level MCP errors.

Caller-correctable failures stay typed instead of collapsing into `internal`.
Current examples include:

- duplicate managed profile creation -> `conflict`
- malformed tab refs like `invalid ref node-42` -> `invalid_input`
- missing tab or browser-surface targets like `selector not found`,
  `ref e99 not found`, or `surface not found: ax:0/4` -> `not_found`
- unsupported schema enum values like unknown `channel`, replay `mode`, or
  lease `mode`, host-access `service`, surface `action`, or execution
  `channel` -> `invalid_input`
- malformed external attach URLs -> `invalid_input`
- unreachable or incomplete attach endpoints like failed `/json/version`,
  failed `/json/list`, or websocket reconnect failures -> `not_ready`
- missing machine permission prerequisites like `requires accessibility permission`
  -> `misconfigured`

Browser-surface failures now also carry structured `data` with:

- `operation`
- `attempted`
- `reason`
- `recovery`
- `retry_likely`

Tab failures carry the same envelope shape:

- `operation`
- `attempted`
- `reason`
- `recovery`
- `retry_likely`

Artifact failures carry the same envelope shape:

- `operation`
- `attempted`
- `reason`
- `recovery`
- `retry_likely`

Non-tab, non-artifact, and non-browser-surface runtime failures now carry the
same shape through `OperationFailurePayload`:

- `operation`
- `attempted`
- `reason`
- `recovery`
- `retry_likely`

Lease-aware browser and capture tools accept an optional `holder_id` so the
same surface can enforce writer or observer ownership without inventing a
second command family.

Host-access and native browser-surface tools are now part of the same catalog:

- `diagnose` returns a side-effect-free machine-readable readiness report with
  permissions, browser channels, service reachability, capability posture, and
  explicit remediation commands.
- `capability_preflight` returns one or all built-in capability policy
  decisions, `ready`, and the exact `PENGU_MESH_CAPABILITY_GRANTS=<capability>`
  hint needed before a trusted local operation.
- `host_access_status` returns platform, tracked services, settings deeplinks,
  assistive overlays, recommended services, and execution-channel readiness.
- `host_access_setup` runs in `audit` or `apply` mode and returns before/after
  status plus per-service setup steps.
- `browser_surface_list` returns browser-native AX surfaces for an instance.
- `browser_surface_list_actions` returns per-surface action contracts,
  required permissions, execution paths, and expected interference.
- `browser_surface_snapshot` returns the native surface tree plus snapshot and
  optional native capture artifacts.
- `browser_surface_action` returns the resolved execution channel,
  interference level, focus impact, and fallback count for a native action.

Holder IDs are cooperative coordination identifiers inside a trusted local
operator boundary. They are not authentication credentials.

### `diagnose` example

```json
{
  "ok": true,
  "code": "ok",
  "message": "diagnose report",
  "timestamp": "2026-03-12T15:24:30Z",
  "data": {
    "schema_version": "diagnose.v1",
    "generated_at": "2026-03-12T15:24:30Z",
    "platform": "macos",
    "state": "degraded",
    "full_capability": false,
    "summary": "3 ready capabilities, 2 blocked, 0 unknown; 1 installed browser channels; 1 reachable services",
    "runtime_root": "/path/to/pengu-mesh/target/pengu-mesh",
    "permissions": [
      {
        "id": "permission:accessibility",
        "service": "accessibility",
        "state": "granted",
        "requestable": true,
        "detail": "Accessibility permission is granted for pengu-mesh",
        "remediation_ids": []
      },
      {
        "id": "permission:listen_event",
        "service": "listen_event",
        "state": "missing",
        "requestable": true,
        "detail": "Listen Event permission is not granted for pengu-mesh",
        "remediation_ids": ["host_access_apply_listen_event"]
      }
    ],
    "browser_channels": [
      {
        "id": "browser_channel:chrome_dev",
        "channel": "chrome_dev",
        "installed": true,
        "managed_launch_ready": true,
        "native_surface_ready": true,
        "app_path": "/Applications/Google Chrome Dev.app",
        "binary_path": "/Applications/Google Chrome Dev.app/Contents/MacOS/Google Chrome Dev",
        "detail": "chrome_dev is installed and native surfaces are ready",
        "remediation_ids": []
      }
    ],
    "services": [
      {
        "id": "native_host_access_probe",
        "state": "reachable",
        "detail": "native host access probe completed successfully",
        "remediation_ids": []
      },
      {
        "id": "http_control_plane",
        "state": "unreachable",
        "detail": "http control plane probe failed for 127.0.0.1:43127: Connection refused (os error 61)",
        "remediation_ids": ["start_http_daemon"]
      }
    ],
    "capabilities": [
      {
        "id": "native_surface_observe",
        "state": "ready",
        "detail": "Accessibility permission is granted and at least one supported browser channel is installed",
        "blockers": [],
        "remediation_ids": []
      },
      {
        "id": "native_global_takeover",
        "state": "blocked",
        "detail": "Listen Event permission is required before global keyboard takeover can be used",
        "blockers": ["permission:listen_event"],
        "remediation_ids": ["host_access_apply_listen_event"]
      },
      {
        "id": "http_control_plane",
        "state": "blocked",
        "detail": "The HTTP control plane is not reachable yet",
        "blockers": ["service:http_control_plane"],
        "remediation_ids": ["start_http_daemon"]
      }
    ],
    "remediations": [
      {
        "id": "host_access_apply_listen_event",
        "title": "Request Listen Event permission",
        "summary": "Run pengu-mesh host-access-setup in apply mode for Listen Event permission.",
        "cli_command": "PENGU_MESH_CAPABILITY_GRANTS=host_access_setup pengu-mesh host-access-setup --mode apply --service listen_event",
        "mcp_tool": "host_access_setup",
        "mcp_arguments": {
          "mode": "apply",
          "services": ["listen_event"]
        },
        "http_method": "POST",
        "http_route": "/host/access/setup",
        "http_body": {
          "mode": "apply",
          "services": ["listen_event"]
        },
        "manual_only": false
      },
      {
        "id": "start_http_daemon",
        "title": "Start HTTP control plane",
        "summary": "Launch the pengu-mesh HTTP daemon so agents can use the HTTP surface.",
        "cli_command": "pengu-mesh serve --bind 127.0.0.1:43127",
        "mcp_tool": null,
        "mcp_arguments": null,
        "http_method": null,
        "http_route": null,
        "http_body": null,
        "manual_only": false
      }
    ]
  }
}
```

### `browser_surface_list_actions` example

```json
{
  "ok": true,
  "code": "ok",
  "message": "browser surface action catalog",
  "timestamp": "2026-03-12T15:27:11Z",
  "data": {
    "instance": {
      "id": "inst_01hsmk6s2kq8h6p6m0z1v9b4c5",
      "name": "agent-session",
      "channel": "chrome_dev",
      "mode": "managed",
      "status": "running",
      "debug_http_url": "http://127.0.0.1:43127",
      "browser_ws_url": "ws://127.0.0.1:43127/devtools/browser/7bf5f936-6c98-4f4d-8f57-18d897489102",
      "profile_id": "profile_01hsmk5v2r7v4g6q4v6a6a6c9d",
      "profile_path": "/path/to/pengu-mesh/target/pengu-mesh/profiles/profile_01hsmk5v2r7v4g6q4v6a6a6c9d/01hsmk6s2kq8h6p6m0z1v9b4c5",
      "pid": 49128,
      "last_error": null,
      "created_at": "2026-03-12T15:26:02Z",
      "updated_at": "2026-03-12T15:27:08Z"
    },
    "app_name": "Google Chrome Dev",
    "surface": {
      "id": "ax:0/4",
      "parent_id": "ax:0",
      "path": "0/4",
      "role": "AXTextField",
      "title": "Address and search bar",
      "description": "omnibox",
      "value": "about:blank",
      "window_title": "about:blank",
      "actions": ["set_value", "focus", "confirm", "scroll"],
      "focused": true,
      "enabled": true,
      "channel": "chrome_dev",
      "app_name": "Google Chrome Dev",
      "instance_id": "inst_01hsmk6s2kq8h6p6m0z1v9b4c5"
    },
    "actions": [
      {
        "action": "set_value",
        "available": true,
        "required_permissions": ["accessibility"],
        "expected_interference_level": "background_safe",
        "detail": "preferred path ax_direct requires accessibility; 1 fallback paths are defined",
        "execution_paths": [
          {
            "execution_channel": "ax_direct",
            "available": true,
            "required_permissions": ["accessibility"],
            "interference_level": "background_safe",
            "detail": "preferred direct Accessibility path; ax_direct is ready"
          },
          {
            "execution_channel": "apple_events_activation",
            "available": false,
            "required_permissions": ["accessibility", "apple_events_chrome_dev"],
            "interference_level": "app_takeover",
            "detail": "fallback path activates the app before reusing Accessibility; apple_events_activation is blocked because apple_events_chrome_dev is missing"
          }
        ]
      },
      {
        "action": "scroll",
        "available": false,
        "required_permissions": ["accessibility"],
        "expected_interference_level": "background_safe",
        "detail": "recognized accessibility action; preferred path ax_direct would require accessibility; runtime invocation is not yet implemented",
        "execution_paths": [
          {
            "execution_channel": "ax_direct",
            "available": false,
            "required_permissions": ["accessibility"],
            "interference_level": "background_safe",
            "detail": "recognized accessibility action; runtime invocation is not yet implemented; direct Accessibility path for background-safe action; ax_direct is ready"
          }
        ]
      }
    ]
  }
}
```

### `tab_list_actions` example

```json
{
  "ok": true,
  "code": "ok",
  "message": "tab action catalog",
  "timestamp": "2026-03-12T15:31:02Z",
  "data": {
    "instance": {
      "id": "inst_01hsmk6s2kq8h6p6m0z1v9b4c5",
      "name": "agent-session",
      "channel": "chrome_dev",
      "mode": "managed",
      "status": "running",
      "debug_http_url": "http://127.0.0.1:43127",
      "browser_ws_url": "ws://127.0.0.1:43127/devtools/browser/7bf5f936-6c98-4f4d-8f57-18d897489102",
      "profile_id": "profile_01hsmk5v2r7v4g6q4v6a6a6c9d",
      "profile_path": "/path/to/pengu-mesh/target/pengu-mesh/profiles/profile_01hsmk5v2r7v4g6q4v6a6a6c9d/01hsmk6s2kq8h6p6m0z1v9b4c5",
      "pid": 49128,
      "last_error": null,
      "created_at": "2026-03-12T15:26:02Z",
      "updated_at": "2026-03-12T15:30:58Z"
    },
    "tab": {
      "id": "tab_01hsmka2xrw7m8m1mxfwqj8p79",
      "instance_id": "inst_01hsmk6s2kq8h6p6m0z1v9b4c5",
      "target_id": "4E2D6F581D59B1A2F07778C08CB00BC9",
      "title": "After",
      "url": "data:text/html,<html><head><title>After</title></head><body>AfterState</body></html>",
      "websocket_url": "ws://127.0.0.1:43127/devtools/page/4E2D6F581D59B1A2F07778C08CB00BC9",
      "active": true,
      "created_at": "2026-03-12T15:26:08Z",
      "updated_at": "2026-03-12T15:31:01Z"
    },
    "actions": [
      {
        "kind": "navigate",
        "available": true,
        "required_permissions": [],
        "detail": "available through tab_action --kind navigate over CDP; requires writer lease and --url <target>"
      },
      {
        "kind": "evaluate",
        "available": true,
        "required_permissions": [],
        "detail": "available through tab_action --kind evaluate over CDP; requires writer lease and --expression <javascript>"
      },
      {
        "kind": "snapshot",
        "available": true,
        "required_permissions": [],
        "detail": "available through pengu-mesh tab-snapshot --tab-id ...; requires observer lease"
      },
      {
        "kind": "recording",
        "available": true,
        "required_permissions": [],
        "detail": "available through pengu-mesh recording-capture --tab-id ...; requires observer lease"
      }
    ]
  }
}
```

### `TabFailurePayload` example

```json
{
  "ok": false,
  "code": "not_found",
  "message": "unknown tab tab_missing",
  "timestamp": "2026-03-12T15:31:09Z",
  "data": {
    "operation": "tab action catalog",
    "attempted": {
      "instance_id": "inst_01hsmk6s2kq8h6p6m0z1v9b4c5",
      "tab_id": "tab_missing",
      "action_kind": "list_actions"
    },
    "reason": "unknown tab tab_missing",
    "recovery": [
      "run pengu-mesh tab-list --instance-id inst_01hsmk6s2kq8h6p6m0z1v9b4c5"
    ],
    "retry_likely": false
  }
}
```

### `tab_action` navigate example

```json
{
  "ok": true,
  "code": "ok",
  "message": "tab action completed",
  "timestamp": "2026-03-12T16:11:02Z",
  "data": {
    "tab": {
      "id": "tab_01hsmka2xrw7m8m1mxfwqj8p79",
      "instance_id": "inst_01hsmk6s2kq8h6p6m0z1v9b4c5",
      "target_id": "4E2D6F581D59B1A2F07778C08CB00BC9",
      "title": "After",
      "url": "data:text/html,<html><head><title>After</title></head><body>AfterState</body></html>",
      "websocket_url": "ws://127.0.0.1:43127/devtools/page/4E2D6F581D59B1A2F07778C08CB00BC9",
      "active": true,
      "created_at": "2026-03-12T15:26:08Z",
      "updated_at": "2026-03-12T16:11:01Z"
    },
    "requested": {
      "kind": "navigate",
      "ref": null,
      "selector": null,
      "url": "data:text/html,<html><head><title>After</title></head><body>AfterState</body></html>",
      "timeout_ms": 250,
      "expression": null,
      "text": null,
      "value": null,
      "key": null
    },
    "resolved_target": "data:text/html,<html><head><title>After</title></head><body>AfterState</body></html>",
    "detail": "navigated to data:text/html,<html><head><title>After</title></head><body>AfterState</body></html>",
    "final_url": "data:text/html,<html><head><title>After</title></head><body>AfterState</body></html>",
    "load_event_fired": true,
    "duration_ms": 42,
    "result": null
  }
}
```

### `tab_action` evaluate example

```json
{
  "ok": true,
  "code": "ok",
  "message": "tab action completed",
  "timestamp": "2026-03-12T16:11:06Z",
  "data": {
    "tab": {
      "id": "tab_01hsmka2xrw7m8m1mxfwqj8p79",
      "instance_id": "inst_01hsmk6s2kq8h6p6m0z1v9b4c5",
      "target_id": "4E2D6F581D59B1A2F07778C08CB00BC9",
      "title": "After",
      "url": "data:text/html,<html><head><title>After</title></head><body>AfterState</body></html>",
      "websocket_url": "ws://127.0.0.1:43127/devtools/page/4E2D6F581D59B1A2F07778C08CB00BC9",
      "active": true,
      "created_at": "2026-03-12T15:26:08Z",
      "updated_at": "2026-03-12T16:11:05Z"
    },
    "requested": {
      "kind": "evaluate",
      "ref": null,
      "selector": null,
      "url": null,
      "timeout_ms": null,
      "expression": "document.title",
      "text": null,
      "value": null,
      "key": null
    },
    "resolved_target": "page",
    "detail": "evaluated expression over CDP",
    "final_url": null,
    "load_event_fired": null,
    "duration_ms": null,
    "result": "After"
  }
}
```

### `artifact_list` example

```json
{
  "ok": true,
  "code": "ok",
  "message": "artifact list",
  "timestamp": "2026-03-12T16:42:18Z",
  "data": {
    "instance_id": "inst_01hsmk6s2kq8h6p6m0z1v9b4c5",
    "run_id": "run_01hsmk6j6vr0h91s13xghh9n8c",
    "artifacts": [
      {
        "id": "artifact_01hsmkq6p0j5tzr8q7yq2tp8z0",
        "kind": "snapshot",
        "path": "/path/to/pengu-mesh/target/pengu-mesh-runtime/artifacts/snapshots/artifact_01hsmkq6p0j5tzr8q7yq2tp8z0.json",
        "sha256": "9ac9f9da3ab191f6f4e6b8a7a0fc9f4a9423498547646387dca6528b2b859e33",
        "size_bytes": 1482,
        "created_at": "2026-03-12T16:42:17Z"
      },
      {
        "id": "artifact_01hsmkq8y4s0w7cfq69n4tswfh",
        "kind": "screenshot",
        "path": "/path/to/pengu-mesh/target/pengu-mesh-runtime/artifacts/screenshots/artifact_01hsmkq8y4s0w7cfq69n4tswfh.png",
        "sha256": "d9a1ab7b3a922969985ea11eb0fa0d8d1f0a13af2e34bdbcb3cc6048eebf4508",
        "size_bytes": 24873,
        "created_at": "2026-03-12T16:42:18Z"
      }
    ]
  }
}
```

### `artifact_verify` valid true example

```json
{
  "ok": true,
  "code": "ok",
  "message": "artifact verify",
  "timestamp": "2026-03-12T16:42:21Z",
  "data": {
    "id": "artifact_01hsmkq6p0j5tzr8q7yq2tp8z0",
    "path": "/path/to/pengu-mesh/target/pengu-mesh-runtime/artifacts/snapshots/artifact_01hsmkq6p0j5tzr8q7yq2tp8z0.json",
    "expected_sha256": "9ac9f9da3ab191f6f4e6b8a7a0fc9f4a9423498547646387dca6528b2b859e33",
    "actual_sha256": "9ac9f9da3ab191f6f4e6b8a7a0fc9f4a9423498547646387dca6528b2b859e33",
    "valid": true
  }
}
```

### `artifact_verify` valid false example

```json
{
  "ok": true,
  "code": "ok",
  "message": "artifact verify",
  "timestamp": "2026-03-12T17:53:00Z",
  "data": {
    "id": "artifact_text_inst_evidence_chain_chrome_dev_55057_tab_inst_evidence_chain_chrome_dev_55057_b0edc135792a52946d9137a509357de8_2026_03_12t17_53_00_690534z",
    "path": "/tmp/pengu-mesh-local-gate-phase4-20260312T175400Z/evidence-chain-smoke/runtime-root/artifacts/text/artifact_text_inst_evidence_chain_chrome_dev_55057_tab_inst_evidence_chain_chrome_dev_55057_b0edc135792a52946d9137a509357de8_2026_03_12t17_53_00_690534z.txt",
    "expected_sha256": "9a72772f896b72e9d07a9e9afc820beb0af5bb7f6f36e505f01eead8fefeaa0c",
    "actual_sha256": "5eb6e9830c6034c4967bf6d11baa184d85e9fda9408dce82285c0de723ed4d5c",
    "valid": false
  }
}
```

### `ArtifactFailurePayload` example

```json
{
  "ok": false,
  "code": "not_found",
  "message": "unknown artifact artifact_missing",
  "timestamp": "2026-03-12T16:42:29Z",
  "data": {
    "operation": "artifact crop",
    "attempted": {
      "artifact_id": "artifact_missing",
      "instance_id": null,
      "run_id": null,
      "action_kind": "crop"
    },
    "reason": "unknown artifact artifact_missing",
    "recovery": [
      "run pengu-mesh run-list --limit 25"
    ],
    "retry_likely": false
  }
}
```

### `OperationFailurePayload` example

```json
{
  "ok": false,
  "code": "not_found",
  "message": "unknown run run_missing",
  "timestamp": "2026-03-12T17:38:11Z",
  "data": {
    "operation": "replay manifest exported",
    "attempted": {
      "operation": "replay_export",
      "instance_id": null,
      "holder_id": null,
      "detail": "run_id=run_missing mode=manifest_only"
    },
    "reason": "unknown run run_missing",
    "recovery": [
      "run pengu-mesh run-list --limit 25"
    ],
    "retry_likely": false
  }
}
```

## Continuity payloads

Health and doctor payloads may surface:

- daemon continuity metadata
  - recovered run
  - reused daemon operator identity
  - recovered daemon-owned lease count
  - stale daemon-owned instance IDs
- attach continuity metadata
  - `outcome`
  - `freshness`
  - `last_resolution`
  - `endpoint_refreshed`
  - last logical instance and endpoint fields

Daemon continuity counters stay daemon-scoped even though lease listings still
show the full shared runtime state.

## Stage 2 capture tools

- `capture_start_recording` returns the current active run or starts a new one
  if the previous run has already been stopped.
- `capture_stop_recording` marks the current run complete and preserves its
  event timeline for later inspection.
- `run_list` returns recent capture runs.
- `events_tail` returns the bounded ordered event stream for a run and now
  returns `not_found` for an explicit unknown `run_id` instead of succeeding
  with `run: null`.
- `artifact_list` returns immutable artifact inventory for the whole runtime or
  filtered by instance, run, or both, with `sha256` and `size_bytes` metadata
  for each entry.
- `artifact_verify` re-reads the artifact file and compares its current bytes
  against the stored SHA-256 without mutating runtime state.
- `artifact_crop` derives a new screenshot artifact from a screenshot or PDF
  source using normalized `0..999` crop bounds and an optional PDF `page_index`.
- `artifact_crop_grid` derives a bounded deterministic grid of screenshot
  crops from a screenshot or PDF source.
- `replay_export` defaults to `manifest_only` and can also run in `portable`
  mode.
- `trace_capture` captures a bounded Chrome trace artifact for a tab.
- `recording_capture` captures a bounded screenshot recording archive for a tab.
- `tab_list_actions` returns the tab capability catalog before mutation.
- `tab_action` executes typed browser actions and currently supports
  `navigate`, `evaluate`, `click`, `focus`, `hover`, `fill`, `type`, `press`, and `select`.
- `tab_action` navigation responses now include `final_url`,
  `load_event_fired`, and `duration_ms`.
- direct standalone CLI invocations do not share daemon continuity; when a
  script shells out to `pengu-mesh` repeatedly, use `artifact_list --instance-id`
  to inventory the full evidence set and `artifact_list --run-id` for the
  specific invocation that produced one artifact batch.
- `tab_snapshot` returns a richer accessibility-oriented page map.
- `tab_screenshot` accepts `full_page`.

## Multi-agent lease tools

- `lease_status` returns the current active leases for an instance or for the
  whole runtime if no `instance_id` is supplied.
- `lease_acquire` creates or renews a writer or observer lease with a bounded TTL.
- `lease_release` drops one holder's active lease set for an instance, or just
  the requested mode when `mode` is supplied.
- `lease_transfer` moves an active writer lease from one holder to another.
- Lease conflicts return `code: conflict`.
