# pengu mesh

[![CI](https://github.com/AlonePenguin/pengu-mesh/actions/workflows/ci.yml/badge.svg)](https://github.com/AlonePenguin/pengu-mesh/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

pengu mesh is a local control plane for coding agents.

Plainly: it helps an agent open a browser, understand what is on the screen,
click or type in a coordinated way, collect proof of what happened, and explain
what is wrong when the local machine is not ready.

The project is written in Rust and is currently strongest on Apple Silicon
macOS. Browser control is the first production surface. The design is broader:
the goal is a trustworthy local mesh for browsers, desktop accessibility,
MCP tools, HTTP services, local state, artifacts, and future task coordination.

## Why it matters

Modern coding agents can write code, but real work often depends on the local
machine:

- a web app needs to be opened and inspected
- a browser permission dialog blocks automation
- a screenshot, PDF, trace, or accessibility snapshot is needed as proof
- several agents need to avoid fighting over the same browser
- a failure needs a clear recovery path instead of "something broke"

pengu mesh turns those messy local actions into typed, inspectable contracts.
It favors honest state over silent fallback. A command should say what it tried,
what happened, and what a human or agent can do next.

## What works today

- Managed Chrome Dev launch and attach flows on macOS.
- Instance, profile, and tab lifecycle commands.
- Tab navigation, evaluation, click, focus, hover, fill, type, press, select,
  text extraction, screenshots, full-page screenshots, and PDF capture.
- Native macOS browser-surface listing, snapshots, and actions through the
  Accessibility tree.
- Local daemon with a JSON HTTP control plane.
- Native stdio MCP server owned by this repo.
- SQLite-backed runtime state, events, capture runs, replay manifests, and
  artifact inventory.
- SQLite-backed scenario runs with list/detail/summary/gate surfaces for stored
  status, assertion, latency, commit, family-level evidence, and promotion
  decisions.
- SHA-256 artifact verification, crops, grid crops, trace capture, and bounded
  recording capture.
- Writer and observer leases so agents can coordinate instead of colliding.
- `diagnose`, `health`, and `pengu-mesh-doctor` readiness surfaces.
- Stored scenario-evidence posture surfaced in `diagnose` and
  `pengu-mesh-doctor`, including latest per-family pass or fail visibility.
- Built-in capability risk posture in health, doctor, and the dashboard:
  safe/elevated/dangerous powers are evaluated against the current local policy.
- Read-only capability preflight over CLI, MCP, and HTTP so agents can ask
  which local power is allowed and which `PENGU_MESH_CAPABILITY_GRANTS` value
  is needed before they act.
- Read-only React dashboard scaffold under `web/dashboard/`.
- Local release gate scripts for browser, lease, continuity, evidence-chain,
  host-access, live-web, and scenario smoke checks.

## Install

You need:

- macOS on Apple Silicon for the best-supported path
- Rust stable with `rustfmt` and `clippy`
- Google Chrome Dev for the default browser channel
- Node.js only if you want to run the dashboard

Clone and verify the workspace:

```bash
git clone https://github.com/AlonePenguin/pengu-mesh.git
cd pengu-mesh
./scripts/dev/bootstrap-rust.sh
make check
make test
```

Run the operator doctor:

```bash
make doctor
```

Run the agent-facing readiness report:

```bash
cargo run -p pengu-mesh -- diagnose
```

## First browser run

Create a profile, start a browser instance, open a tab, and capture proof:

```bash
cargo run -p pengu-mesh -- profile-create --name agent-alpha --channel chrome-dev
cargo run -p pengu-mesh -- instance-start --name smoke --channel chrome-dev --holder-id agent-alpha
cargo run -p pengu-mesh -- tab-open --instance-id <instance_id> --url https://example.com --holder-id agent-alpha
cargo run -p pengu-mesh -- tab-text --tab-id <tab_id> --holder-id agent-alpha
cargo run -p pengu-mesh -- tab-screenshot --tab-id <tab_id> --full-page --holder-id agent-alpha
```

Use the IDs printed by each command in the next command. If the machine is not
ready, run:

```bash
cargo run -p pengu-mesh -- host-access-status
cargo run -p pengu-mesh -- host-access-setup --mode audit
cargo run -p pengu-mesh-doctor -- --setup-wizard
```

Those commands are read-only. They describe the missing macOS permissions and
show the recovery steps; they do not silently grant permissions or open system
settings on your behalf.

## Daemon, HTTP, and MCP

Start the local daemon:

```bash
cargo run -p pengu-mesh -- serve
```

The daemon binds locally by default and exposes a JSON HTTP control plane. The
same runtime contracts are also available through the native stdio MCP server:

```bash
cargo run -p pengu-mesh-mcp -- --once-tool events_tail --once-input '{"limit":10}'
```

Holder IDs are cooperative local coordination names. They are not passwords,
tokens, or authentication credentials.

## Dashboard

The dashboard is a read-only health console. It is useful for operators who
want to see readiness, continuity, route inventory, host access, browser state,
lease coverage, and capability risk posture without digging through CLI output.

```bash
cd web/dashboard
npm ci
npm run dev
```

By default Vite proxies API requests to `http://127.0.0.1:43127`. Override the
target with `PENGU_MESH_DASHBOARD_API_ORIGIN` when needed.

## Safety model

pengu mesh is a local tool for a trusted machine. It can launch browsers,
inspect pages, and interact with macOS Accessibility when you grant those
permissions.

Important boundaries:

- It binds locally by default.
- External browser attach is opt-in with `PENGU_MESH_ALLOW_EXTERNAL_ATTACH=1`.
- The default capability policy is visible and safe-only: safe capabilities are
  allowed, elevated capabilities are denied, and dangerous capabilities require
  explicit grants. `host-access-setup --mode apply` and browser surface actions
  that permit global takeover require `PENGU_MESH_CAPABILITY_GRANTS`.
- `capability-preflight`, MCP `capability_preflight`, and
  `/capabilities/preflight` expose the current grant hint without mutating the
  machine.
- Diagnostic commands are designed to be side-effect-free.
- `diagnose` and `pengu-mesh-doctor` read existing stored scenario evidence
  without creating or mutating runtime state when no runtime database exists.
- Health and doctor surfaces should report degraded states honestly.
- Generated proof under `reports/audit/` and `reports/local-gate/` can contain
  local paths, screenshots, browser metadata, and machine posture. Those
  directories are ignored by default; commit only deliberate, reviewed
  summaries.

See [docs/security-model.md](docs/security-model.md) for the current security
posture and the remaining work around authenticated ownership and enforcing the
surfaced capability policy across mutate paths.

## Project map

- [docs/current-status.md](docs/current-status.md): shipped, deferred, and next
  work
- [docs/feature-file-map.md](docs/feature-file-map.md): feature ownership by
  file
- [docs/implementation-backlog.md](docs/implementation-backlog.md): practical
  build backlog
- [docs/runtime-model.md](docs/runtime-model.md): runtime concepts
- [docs/mcp-contract.md](docs/mcp-contract.md): MCP contract examples
- [docs/state-and-replay.md](docs/state-and-replay.md): events, artifacts, and
  replay
- [docs/repo-hygiene-plan.md](docs/repo-hygiene-plan.md): proof retention and
  cleanup policy
- [AGENTS.md](AGENTS.md): repo rules for coding agents

## Verification

Fast checks:

```bash
make fmt
make check
make test
npm --prefix web/dashboard ci
npm --prefix web/dashboard run build
```

Full local release gate:

```bash
make local-gate
```

The full gate exercises real browser and host-access behavior, so it is more
environment-sensitive than unit tests. It writes local proof under ignored
report directories.

## Roadmap

The next high-value work is:

1. Broader repeated real-web, weak-prompt, fresh-agent, and operator-diagnosis
   scenarios.
2. Performance budgets backed by stored measurements, not vibes.
3. Authenticated local ownership and enforced capability gating.
4. A dashboard that moves beyond read-only health into replay, lease,
   continuity, and task views.
5. A durable task plane above leases so agents can schedule work instead of
   only coordinating browser access.

## Contributing

Contributions are welcome. The most useful contributions are small, verified,
and honest about what they prove.

Start with [CONTRIBUTING.md](CONTRIBUTING.md). If you are changing behavior,
include tests or a smoke script. If you are changing docs, keep claims aligned
with the current implementation. If a command can mutate the host, its failure
and recovery story must be explicit.

## License

MIT. See [LICENSE](LICENSE).
