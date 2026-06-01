# Attach Contract

Attach support must distinguish:

- managed launches
- external browser attach under explicit local opt-in
- daemon-root restart recovery of logical runtime ownership
- endpoint-evidence continuity for repeated attaches to the same browser

External attach remains opt-in behind `PENGU_MESH_ALLOW_EXTERNAL_ATTACH=1`.

## Current continuity rules

- logical attached-instance reuse happens only when endpoint evidence matches
  an existing attached instance exactly
- debug HTTP URL is the first reuse key
- browser websocket URL is the second reuse key
- name-only reuse is not part of the automatic continuity path
- a debug-URL match with stale browser websocket evidence is reported as
  `reclaimed_stale_instance`, not `reused_existing_instance`
- live `/json/version` and `/json/list` must both succeed before a continuity
  update is published
- failed attach attempts do not publish a new continuity outcome and do not
  leave a new attached instance behind in health or doctor
- stale stored attached instances may be reclaimed when endpoint evidence now
  points back to the same logical browser identity

## Operator-visible status

Health and doctor surface attach continuity explicitly:

- `outcome`
  - `new_instance`
  - `reused_existing_instance`
  - `reclaimed_stale_instance`
- `freshness`
  - `none`
  - `live`
  - `stale_instance`
  - `stale_endpoint`
- `last_resolution`
  - `debug_http_url`
  - `browser_ws_url`
  - `new_instance`
- `endpoint_refreshed`
- the last logical instance ID, debug HTTP URL, requested CDP URL, and live
  browser websocket URL

## Recovery behavior

- daemon continuity remains rooted in the daemon runtime root and daemon-owned
  recovered leases
- attach continuity persists across daemon restart when the same runtime root
  reconnects to the same browser
- action and capture paths now retry tab websocket recovery once after a tab
  refresh when persisted target websockets drift

## Verification

- `cargo test -p pengu-mesh-core --lib`
- `./scripts/release/attach-continuity-smoke.sh`

Deeper continuity work beyond the current trusted-local, runtime-root model
remains deferred until stronger ownership and authentication semantics are
designed.
