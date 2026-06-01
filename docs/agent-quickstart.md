# Agent Quickstart
Minimum-command runbook for agents using the standalone `pengu-mesh` tool.

Assumptions:
- `pengu-mesh` is on `PATH`.
- In a repo checkout, replace `pengu-mesh` with `cargo run -p pengu-mesh --`.
- Replace `pengu-mesh-mcp` with `cargo run -p pengu-mesh-mcp --`.

## 1. Check Runtime Reachability
```bash
pengu-mesh health
curl -sf http://127.0.0.1:43127/tools
pengu-mesh-mcp --once-tool diagnose
```
`health` is a normal response envelope:
```json
{"ok":true,"code":"ok","message":"runtime health","timestamp":"2026-03-12T00:00:00Z","data":{"operator_id":"operator_...","installations":[{"channel":"chrome_dev","installed":true}]}}
```
`/tools` returns the tool catalog:
```json
{"ok":true,"code":"ok","message":"tool catalog","timestamp":"2026-03-12T00:00:00Z","data":{"tools":[{"name":"diagnose"},{"name":"tab_list_actions"},{"name":"artifact_verify"}]}}
```
If `/tools` fails, the HTTP daemon is not reachable.

## 2. Diagnose Host Readiness
```bash
pengu-mesh diagnose
pengu-mesh-mcp --once-tool diagnose
curl -sf http://127.0.0.1:43127/diagnose
```
Parse the `data` section for readiness and remediation:
```json
{"ok":true,"code":"ok","message":"diagnose report","timestamp":"2026-03-12T00:00:00Z","data":{"schema_version":"diagnose.v1","state":"ready","full_capability":true,"permissions":[{"service":"accessibility","state":"granted","remediation_ids":[]}],"services":[{"id":"http_control_plane","state":"reachable","remediation_ids":[]}],"capabilities":[{"id":"native_surface_observe","state":"ready","blockers":[]}],"remediations":[]}}
```
Prioritize `state`, blocked `capabilities`, missing `permissions`, unreachable `services`, and explicit remediation commands.

## 3. Apply Missing Host Access
```bash
pengu-mesh host-access-setup --mode apply --service accessibility
pengu-mesh host-access-setup --mode apply --service screen_capture
pengu-mesh host-access-setup --mode apply --service listen_event
pengu-mesh host-access-setup --mode audit --service apple_events_chrome_dev
```
Re-run `diagnose` after each change; it is read-only and returns the next safe remediation.

## 4. Start Or Attach A Browser
```bash
pengu-mesh instance-start --name agent-session --channel chrome-dev --headless --holder-id agent-writer
PENGU_MESH_ALLOW_EXTERNAL_ATTACH=1 pengu-mesh instance-attach --name external --cdp-url ws://127.0.0.1:9222/devtools/browser/... --holder-id agent-writer
```
`instance-start` returns a single envelope:
```json
{"ok":true,"code":"ok","message":"instance started","timestamp":"2026-03-12T00:00:00Z","data":{"id":"inst_...","channel":"chrome_dev","debug_http_url":"http://127.0.0.1:52271","browser_ws_url":"ws://127.0.0.1:52271/devtools/browser/..."}} 
```

## 5. Open A Tab And Ask For Its Contract
```bash
pengu-mesh tab-open --instance-id inst_... --url 'data:text/html,<title>Before</title><body>BeforeState</body>' --holder-id agent-writer
pengu-mesh tab-list-actions --instance-id inst_... --tab-id tab_... --holder-id agent-writer
```
`tab-open`:
```json
{"ok":true,"code":"ok","message":"tab opened","timestamp":"2026-03-12T00:00:00Z","data":{"id":"tab_...","instance_id":"inst_...","url":"data:text/html,<title>Before</title><body>BeforeState</body>","websocket_url":"ws://127.0.0.1:52271/devtools/page/..."}} 
```
`tab-list-actions`:
```json
{"ok":true,"code":"ok","message":"tab action catalog","timestamp":"2026-03-12T00:00:00Z","data":{"instance":{"id":"inst_..."},"tab":{"id":"tab_..."},"actions":[{"kind":"navigate","available":true,"required_permissions":[],"detail":"available through tab_action --kind navigate over CDP; requires writer lease and --url <target>; optional --timeout-ms <milliseconds>"}]}}
```

## 6. Run Typed Tab Actions
Navigate with an optional timeout override:
```bash
pengu-mesh tab-action --tab-id tab_... --kind navigate --url 'data:text/html,<title>After</title><body>AfterState</body>' --timeout-ms 250 --holder-id agent-writer
pengu-mesh tab-action --tab-id tab_... --kind evaluate --expression 'document.title' --holder-id agent-writer
```
Navigate success:
```json
{"ok":true,"code":"ok","message":"tab action completed","timestamp":"2026-03-12T00:00:00Z","data":{"tab":{"id":"tab_..."},"requested":{"kind":"navigate","url":"data:text/html,<title>After</title><body>AfterState</body>","timeout_ms":250},"resolved_target":"data:text/html,<title>After</title><body>AfterState</body>","detail":"navigated to data:text/html,<title>After</title><body>AfterState</body>","final_url":"data:text/html,<title>After</title><body>AfterState</body>","load_event_fired":true,"duration_ms":42,"result":null}}
```
Evaluate success:
```json
{"ok":true,"code":"ok","message":"tab action completed","timestamp":"2026-03-12T00:00:00Z","data":{"tab":{"id":"tab_..."},"requested":{"kind":"evaluate","expression":"document.title"},"resolved_target":"page","detail":"evaluated expression over CDP","final_url":null,"load_event_fired":null,"duration_ms":null,"result":"After"}}
```

## 7. Capture And Verify Evidence
```bash
pengu-mesh tab-snapshot --tab-id tab_... --holder-id agent-writer
pengu-mesh tab-screenshot --tab-id tab_... --holder-id agent-writer
pengu-mesh tab-text --tab-id tab_... --holder-id agent-writer
pengu-mesh artifact-list --instance-id inst_...
pengu-mesh artifact-verify --artifact-id artifact_...
```
Snapshot:
```json
{"ok":true,"code":"ok","message":"tab snapshot","timestamp":"2026-03-12T00:00:00Z","data":{"artifact":{"id":"artifact_snapshot","path":".../snapshot.json"},"snapshot":{"nodes":[{"ref":"e1","role":"heading","name":"After"}]}}}
```
Screenshot:
```json
{"ok":true,"code":"ok","message":"tab screenshot","timestamp":"2026-03-12T00:00:00Z","data":{"artifact":{"id":"artifact_screenshot","path":".../screenshot.png"}}}
```
Text:
```json
{"ok":true,"code":"ok","message":"tab text","timestamp":"2026-03-12T00:00:00Z","data":{"artifact":{"id":"artifact_text","path":".../text.txt"},"text":"AfterState"}}
```
Artifact inventory:
```json
{"ok":true,"code":"ok","message":"artifact list","timestamp":"2026-03-12T00:00:00Z","data":{"instance_id":"inst_...","run_id":null,"artifacts":[{"id":"artifact_screenshot","kind":"screenshot","path":".../screenshot.png","sha256":"...","size_bytes":1234,"created_at":"2026-03-12T00:00:00Z"}]}}
```
Artifact verification:
```json
{"ok":true,"code":"ok","message":"artifact verify","timestamp":"2026-03-12T00:00:00Z","data":{"id":"artifact_screenshot","path":".../screenshot.png","expected_sha256":"...","actual_sha256":"...","valid":true}}
```
Standalone CLI invocations do not share daemon continuity. When chaining several `pengu-mesh` commands, use `artifact-list --instance-id inst_...` to inventory the full evidence set and reserve `run_id` filters for a single invocation's artifact batch.

## 8. Inspect Native Browser Surfaces
```bash
pengu-mesh browser-surface-list --instance-id inst_... --holder-id agent-writer
pengu-mesh browser-surface-list-actions --instance-id inst_... --surface-id ax:0/... --holder-id agent-writer
pengu-mesh browser-surface-snapshot --instance-id inst_... --root-surface-id ax:0/... --holder-id agent-writer
```
Surface inventory:
```json
{"ok":true,"code":"ok","message":"browser surface list","timestamp":"2026-03-12T00:00:00Z","data":{"app_name":"Google Chrome Dev","surfaces":[{"id":"ax:0/...","role":"AXWindow","channel":"chrome_dev","actions":["focus","set_value","confirm"]}]}}
```
Surface catalog:
```json
{"ok":true,"code":"ok","message":"browser surface action catalog","timestamp":"2026-03-12T00:00:00Z","data":{"surface":{"id":"ax:0/...","role":"AXTextField","channel":"chrome_dev"},"actions":[{"action":"set_value","available":true,"required_permissions":["accessibility"],"expected_interference_level":"background_safe","execution_paths":[{"execution_channel":"ax_direct","available":true}]}]}}
```
Surface snapshot:
```json
{"ok":true,"code":"ok","message":"browser surface snapshot","timestamp":"2026-03-12T00:00:00Z","data":{"snapshot_artifact":{"id":"artifact_ax","path":".../surface.json"},"capture_artifact":{"id":"artifact_capture","path":".../surface.png"},"surfaces":[{"id":"ax:0/...","role":"AXWindow"}]}}
```

## 9. Expect Structured Failures
Missing targets do not return opaque strings.
```bash
pengu-mesh tab-list-actions --instance-id inst_... --tab-id tab_missing --holder-id agent-writer
```
```json
{"ok":false,"code":"not_found","message":"unknown tab tab_missing","timestamp":"2026-03-12T00:00:00Z","data":{"operation":"tab action catalog","attempted":{"instance_id":"inst_...","tab_id":"tab_missing","action_kind":"list_actions"},"reason":"unknown tab tab_missing","recovery":["run pengu-mesh tab-list --instance-id inst_..."],"retry_likely":false}}
```

Use `artifact-list` before destructive evidence work, and use `artifact-verify` before trusting an artifact path that may have been copied or modified outside the runtime.
