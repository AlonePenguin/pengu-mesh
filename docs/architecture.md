# Architecture

The initial shape is a performance-conscious modular monolith.

## Major subsystems

- `transport`: stdio MCP, local HTTP control plane, event stream
- `browser engine`: CDP transport, attach lifecycle, action execution
- `artifact pipeline`: screenshots, PDFs, derived crops, snapshots, recordings,
  traces
- `coordination`: leases, observers, backpressure, conflict handling
- `state`: SQLite metadata, append-only event log, replay bundles

## Code layout intent

- `crates/pengu-mesh-*` isolate contracts and policy from platform-specific code.
- core crates stay portable; Apple-only behavior belongs behind narrow shims.
- the dashboard remains a later consumer under `web/dashboard/`; the current
  repo carries a read-only health console scaffold there, but it is still not
  the product center or a full operator workflow surface.
