# Upstream PinchTab Audit

## Snapshot

- upstream repo: `pinchtab/pinchtab`
- imported commit: `804ba5b8fca7ba0e54683f82209ce8de48656a36`
- baseline metadata: `reference/upstream/pinchtab.METADATA.json`
- public source tree: `https://github.com/pinchtab/pinchtab/tree/804ba5b8fca7ba0e54683f82209ce8de48656a36`

## Observed shape

- single Go module with a `cmd/pinchtab` entrypoint
- `internal/` contains the bulk of runtime logic
- dashboard source ships in the same repo under `dashboard/`
- plugin and skill material exists alongside the server

## Relevant module areas

- `internal/api`, `internal/handlers`, `internal/web`
- `internal/bridge`, `internal/engine`, `internal/orchestrator`
- `internal/profiles`, `internal/instance`, `internal/scheduler`
- `dashboard/` and `plugins/`

## Baseline takeaways

- upstream proves the product demand and operator model
- the codebase mixes control-plane, browser, dashboard, and plugin concerns in
  one repository
- the orchestration surface is already broad enough to define a serious parity
  target
- a live local probe on 2026-03-12 against a temporary upstream checkout
  confirmed PinchTab can launch a real server and pass at least `TestHealth`
  and `TestNavigate` integration coverage on this machine
- the same probe also showed that upstream still has broader security and
  operator-envelope expectations than pengu mesh currently ships
