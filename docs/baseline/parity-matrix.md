# Parity Matrix

| Capability | Upstream PinchTab | pengu mesh status | Direction |
| --- | --- | --- | --- |
| Local daemon | Present | Shipped | Rust-first daemon via `pengu-mesh serve` |
| HTTP control plane | Present | Shipped | Lean local JSON surface backed by shared runtime |
| Profile management | Present | Shipped | Managed default profiles with runtime-backed inventory |
| Instance lifecycle | Present | Shipped | Start, attach, stop, and state refresh |
| External attach | Present | Shipped | Opt-in with tighter local policy |
| Snapshot/text | Present | Shipped | Runtime-backed capture with event provenance |
| Screenshot/PDF | Present | Shipped | Runtime-backed artifacts with replay linkage |
| Native stdio MCP | Plugin-oriented | Shipped and exceeds upstream | First-class repo-owned product surface |
| Replay bundles | Partial | Shipped and exceeds upstream | Manifest-only plus portable bundles |
| Derived artifact crops | Limited | Shipped and exceeds upstream | Narrow crop and deterministic crop-grid workflows |
| Trace and recording capture | Limited | Shipped and exceeds upstream | Bounded operator-grade evidence capture |
| Multi-agent leases | Implicit | Shipped | Explicit writer and observer coordination with transfer and conflict reporting |
| Restart continuity | Limited | Shipped and exceeds upstream | Stable daemon operator identity, active run recovery, holder-scoped lease recovery, and stale-instance classification |
| Metrics database and scenario leaderboard | Partial | Not yet shipped | Required next milestone for repeatable superiority claims |
| Authenticated ownership and capability gating | Stronger | Partial | Capability risk posture is visible in health, doctor, and dashboard; upstream remains ahead until pengu mesh lands authenticated local control and enforced denials |
| Task scheduling and queue management | Present | Foundation only | Queue primitives exist in core, but no public task plane queues or dispatches work yet |
| Allocation policies (FCFS, round-robin, random) | Present | Foundation only | Policy helpers exist in core, but no runtime scheduling surface ships them yet |
| Semantic element matching | Present | Foundation only | Lexical matcher exists in core, but browser interactions still rely on direct selectors |
| Human-like input emulation | Present | Foundation only | Deterministic human timing helpers exist in CDP, but no shipped tab-action contract uses them yet |
| Ad-block and tracker filtering | Present | Foundation only | CDP block-list and Fetch helpers exist, but no public runtime surface enables them yet |
| Animation suppression | Present | Foundation only | Reduced-motion and CSS suppression helpers exist, but no product contract turns them on |
| Auto-restart strategies | Present | Foundation only | Restart and backoff helpers exist in core, but crash-recovery policy is not shipped |
| HTTP proxy forwarding | Present | Foundation only | Reverse-proxy helper exists, but no typed HTTP route is shipped |
| Operator dashboard | Present | Partial | `web/dashboard/` now carries a read-only health console scaffold, but broader operator workflows are still missing |
| Webhook notifications | Present | Foundation only | Validated delivery helpers exist in core, but no callback contract triggers them |
| Operator console | Present | Partial | Read-only health console exists, but replay, diagnose, lease, continuity, and task workflows are still deferred |
