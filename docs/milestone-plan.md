# Milestone Plan

Use these docs together:

- `docs/current-status.md` for short shipped/deferred truth
- `docs/feature-file-map.md` for current file ownership and gap locations
- `docs/implementation-backlog.md` for tracked workstreams and exit criteria

1. Baseline audit and dependency shootout
2. Core daemon bootstrap
3. Core actions and artifact contracts
4. Native stdio MCP
5. Run capture and append-only event log
6. Replay manifests
7. Portable handoff bundles, bounded trace/recording capture, and advanced inspection
8. Lease coverage and shared-resource coordination for the current public surface
   - status: materially advanced and now surfaced as a real coverage matrix
   - shipped: writer/observer enforcement on the current browser and evidence paths, typed conflicts across CLI/MCP/HTTP, transactionally serialized writer acquisition, lease smoke
   - explicit outside-model paths are now documented instead of implied
9. Attach continuity beyond one-shot attach registration
   - status: materially advanced
   - shipped: daemon-root continuity, endpoint-evidence reuse, websocket refresh, stale attached-instance reclaim, outcome/freshness surfacing, attach continuity smoke
   - deferred: stronger identity/auth semantics beyond the trusted-local model
10. Stricter production gate
   - status: materially advanced
   - shipped: fmt/check/test, bench discovery/compile, narrow bench-threshold enforcement over the current JSON/CDP/persistence/artifact manifest, diagnose smoke, lease smoke, continuity smoke, attach continuity smoke, host-access smoke, browser-lifecycle integration, tab-lifecycle integration, evidence-chain smoke, browser-surface smoke, isolated-runtime local health, isolated-runtime local doctor
   - deferred: broader thresholded performance budgets beyond the current narrow manifest
11. Host access and native browser-surface substrate
   - status: first production slice shipped
   - shipped: `pengu-mesh-macos`, machine permission matrix, setup audit/apply flow, read-only `pengu-mesh-doctor --setup-wizard`, settings deeplink inventory, assistive-overlay catalog, native browser-surface list/snapshot/action, host-access and browser-surface gate smokes
   - deferred: generic desktop-control plane, desktop mutation lease model, compiled Apple framework shims instead of the current Python bridge
12. Metrics and scenario lab
   - status: in progress
   - shipped: durable scenario metrics tables, runtime and transport scenario list/detail surfaces, recorder helpers and CLI shims, and first named workflow families under `examples/workflows/`
   - current proof: `startup-readiness`, `evidence-chain`, `structured-failure`, `live-web`, `weak-prompt`, `fresh-agent`, `operator-diagnosis`, and `pinchtab-comparison`; startup-readiness, fresh-agent, evidence-chain, structured-failure, weak-prompt, and operator-diagnosis are in the scenario-gate manifest
   - next: broader live-web coverage plus deeper fresh-agent packs, repeated PinchTab comparisons, and thresholded use of the stored metrics
   - required: durable metrics database, named scenario families, live-web validation, stored PinchTab comparisons, broader weak-prompt, fresh-agent, and operator-diagnosis prompt packs, and in-repo scenario definitions under `examples/workflows/`
13. Authenticated ownership and capability gating
   - status: partial
   - shipped: default safe/elevated/dangerous capability posture is visible in
     health, doctor, and the read-only dashboard; capability preflight is
     exposed over CLI/MCP/HTTP with exact grant hints; host-access apply mode
     and browser-surface global takeover require explicit capability grants
   - deferred: authenticated holder ownership and enforced typed denials across
     the rest of the mutate surface
14. Operator console
15. Durable task plane
