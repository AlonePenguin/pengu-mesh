# Vision

pengu mesh exists to become the clearest, most trustworthy, and most
agent-native local meshing harness in this lane.

The current implementation is browser-first, but the identity is broader:
agents should be able to mesh across browsers, apps, OS accessibility layers,
MCP servers, HTTP services, and other local control surfaces without losing
typed truth about what succeeded, what failed, and what access is still
missing.

## Better than PinchTab means

- faster attach and reconnect behavior on the design-center machine
- clearer diagnostics and doctor tooling
- native stdio MCP with typed outputs owned by this repo
- stronger evidence capture and replay packaging
- explicit multi-agent lease semantics instead of ad hoc write contention
- local proof-first release discipline owned in-repo, not outsourced to hosted CI
- stronger real-web task execution and validation, not just cleaner internals
- stronger fresh-agent usability under concise, weak, and partially wrong prompts
- stronger operator time-to-diagnosis with health, doctor, replay, and continuity truth
- stronger measured performance with thresholded budgets once baselines stabilize
- stronger comparative evidence against upstream, not one-off confidence claims

## Current operating charter

- `docs/agent-execution-charter.md` is the permanent handoff and execution contract
- future agents should treat it as the primary roadmap for shipping quality,
  live validation, metrics, and PinchTab comparison work

## Non-goals for the initial scaffold

- no downstream project integration playbooks
- no dashboard-first development
- no premature commitment to convenience frameworks on hot paths
