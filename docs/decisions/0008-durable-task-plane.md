# ADR 0008: Durable task plane above leases

## Status

Proposed

## Context

`main` does not yet ship a durable task plane. The current product and status
docs only promise a future plane above the existing lease primitives, with task
ownership, fairness or admission control, cancellation, and replay linkage.

The boundaries already present in the repo matter:

- leases are the live coordination primitive for shared browser resources
- runs, events, and artifacts are the replay and evidence substrate
- scenario runs are proof-oriented workflow records, not operational queue items
- `holder_id` is still a trusted-local coordination identifier, not an
  authentication credential
- `diagnose`, health, and `pengu-mesh-doctor` must stay truthful read-only
  surfaces
- SQLite is already the single durable state store and follows an additive
  migration pattern

The task plane needs to fit those boundaries instead of blurring them. If it
claims stronger fairness, ownership, or replay semantics than the current repo
can actually enforce, it will make the public contract less honest.

## Current implementation note

Today the runtime stops at durable leases plus replay/state storage. There are
no task tables, task tools, task HTTP routes, or queue workers on `main`.
This ADR records the target design so future implementation can land without
pretending the surface already exists.

## Decision

When pengu mesh adds a durable task plane, it will be implemented inside the
existing Rust runtime and SQLite store. The task plane will sit above leases
and below any future operator console.

The first honest slice is:

- durable task records in SQLite
- explicit lifecycle and assignment ownership
- admission control plus best-effort per-holder fairness within one trusted
  local runtime root
- explicit cancellation requests with durable acknowledgement semantics
- replay linkage through the existing run, event, and artifact model
- no public task surface until shared types, runtime behavior, CLI, MCP, HTTP,
  docs, and release proof all ship together

The task plane will not:

- replace lease enforcement
- treat `holder_id` as an authentication or authorization credential
- overload `scenario_runs` as the scheduler
- introduce an external queue or broker
- promise webhooks, DAG orchestration, or cross-host routing in the first slice

## Subagent Handoff Compatibility

The docs/process operating model in
[`../autonomous-operating-model.md`](../autonomous-operating-model.md) is the
handoff shape the future task plane should be able to ingest. Until runtime
task records exist, handoffs remain Markdown artifacts and must not imply
stronger ownership than trusted-local holder IDs.

Future task records should be able to carry:

- `role`
- `task_id`
- `requested_by_holder_id`
- `assigned_holder_id`
- `scope`
- `files_changed`
- `lease_or_capability_decisions`
- `commands_run`
- `artifacts`
- `scenario_or_run_ids`
- `outcome`
- `open_risks`
- `next_owner`

That shape lets today's subagent work become tomorrow's durable task payload
without overloading scenario runs or weakening the lease model.

## Task lifecycle states

The task plane should use explicit durable states rather than one overloaded
status field:

- `queued`: accepted and durable, but not yet assigned to a worker
- `claimed`: assigned to an `assigned_holder_id`, but execution has not started
- `running`: execution has started and the task has been bound to an execution
  run
- `cancellation_requested`: a durable cancellation request exists, but the task
  has not yet reached a terminal outcome
- `succeeded`: terminal success with a result payload
- `failed`: terminal failure with a structured failure payload
- `cancelled`: terminal cancelled outcome

Allowed transitions:

- `queued -> claimed`
- `queued -> cancelled`
- `claimed -> running`
- `claimed -> queued`
- `claimed -> cancelled`
- `claimed -> cancellation_requested`
- `running -> succeeded`
- `running -> failed`
- `running -> cancellation_requested`
- `cancellation_requested -> cancelled`
- `cancellation_requested -> succeeded`
- `cancellation_requested -> failed`

Rationale for the intermediate states:

- `claimed` separates durable admission from actual execution, so the runtime
  can surface stuck ownership or claim recovery explicitly instead of hiding it
  behind a long-lived "running" state
- `cancellation_requested` avoids lying about cancellation success before the
  worker has actually acknowledged the stop request

Terminal states are immutable except for additive replay metadata or operator
notes.

## Ownership and admission model

Task ownership needs to stay aligned with the current lease model instead of
inventing a stronger security claim than `main` can support.

- `requested_by_holder_id` records who asked for the task
- `assigned_holder_id` records who currently owns execution responsibility
- both fields use the same trusted-local identifier space as the current lease
  and daemon operator model
- because holder identity is not authenticated yet, fairness can only be
  best-effort inside one local operator boundary; the first slice should prefer
  bounded admission control over stronger multi-tenant fairness claims
- task ownership does not grant resource ownership; a task that targets a
  browser instance, tab, or native surface still has to acquire or verify the
  required lease before it can transition to `running`
- lease conflicts must stay explicit; if a claimed task cannot obtain the
  required lease, it remains non-terminal and records the conflict as typed
  detail instead of silently spinning

Admission control should stay deliberately simple at first:

- cap queued work per requesting holder
- cap concurrently claimed or running work per assigned holder
- serialize exclusive work by declared target resource when the task names one
- use claim staleness timeouts so abandoned ownership can be returned to
  `queued` with an explicit replay-visible transition

## Cancellation semantics

Cancellation must be durable and truthful rather than a best-effort process
kill.

- cancelling a `queued` task moves it directly to `cancelled`
- cancelling a `claimed` task may move it directly to `cancelled` if execution
  has not started, or to `cancellation_requested` if the worker is already
  entering execution
- cancelling a `running` task always records `cancellation_requested` first
- worker code is responsible for checking cancellation between major steps and
  before starting another browser mutation or capture leg
- if the task completes before observing the cancellation request, the terminal
  state remains `succeeded` or `failed`; cancellation is advisory until it is
  acknowledged
- any implementation that interrupts subprocesses, browser activity, or native
  capture must still record a final terminal outcome and an explanatory replay
  event

The control surfaces stay separate:

- explicit task mutation surfaces will request cancellation
- read-only surfaces such as `diagnose`, health, and doctor may report pending
  cancellation or stuck work, but must never trigger cancellation themselves

## Replay linkage

The task plane should reuse the current replay model rather than inventing a
parallel scheduler trace.

- each task may record `created_run_id` for the capture run that was active
  when the task was accepted
- each task records `execution_run_id` once execution actually starts
- task state changes append `category = "task"` events into the existing
  ordered `events` table
- replay export stays run-centric; task drill-down is layered on top of run
  export instead of creating a second bundle format
- scenario runs remain separate; a future scenario harness may submit tasks,
  but `scenario_runs` and `scenario_steps` stay proof records rather than
  queue internals

To make replay linkage queryable without parsing free-form JSON, the first
schema extension should add a nullable `task_id` column to `events`. Artifact
linkage can continue to flow through `artifacts.run_id` in the first slice. If
task-scoped artifact inventory later becomes a real public need, add
`artifacts.task_id` only when that surface is ready to ship honestly.

## SQLite schema direction

The schema should remain additive and match the current migration style in
`crates/pengu-mesh-state`:

```sql
CREATE TABLE tasks (
    id TEXT PRIMARY KEY,
    task_kind TEXT NOT NULL,
    state TEXT NOT NULL,
    priority INTEGER NOT NULL DEFAULT 0,
    requested_by_holder_id TEXT NOT NULL,
    assigned_holder_id TEXT,
    resource_kind TEXT,
    resource_id TEXT,
    created_run_id TEXT REFERENCES runs(id),
    execution_run_id TEXT REFERENCES runs(id),
    payload_json TEXT NOT NULL,
    result_json TEXT,
    failure_json TEXT,
    outcome_code TEXT,
    cancel_requested_at TEXT,
    cancelled_by_holder_id TEXT,
    created_at TEXT NOT NULL,
    claimed_at TEXT,
    started_at TEXT,
    finished_at TEXT,
    updated_at TEXT NOT NULL
);

CREATE TABLE task_attempts (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks(id),
    attempt INTEGER NOT NULL,
    assigned_holder_id TEXT NOT NULL,
    run_id TEXT REFERENCES runs(id),
    state TEXT NOT NULL,
    started_at TEXT NOT NULL,
    finished_at TEXT,
    outcome_code TEXT,
    failure_json TEXT
);

CREATE UNIQUE INDEX idx_task_attempts_task_attempt
    ON task_attempts (task_id, attempt);

CREATE INDEX idx_tasks_state_priority_created
    ON tasks (state, priority DESC, created_at ASC);

CREATE INDEX idx_tasks_assigned_state
    ON tasks (assigned_holder_id, state, updated_at DESC);

CREATE INDEX idx_tasks_resource_state
    ON tasks (resource_kind, resource_id, state, priority DESC, created_at ASC);

CREATE INDEX idx_events_task_sequence
    ON events (task_id, sequence DESC);
```

Implementation guidance:

- keep scheduler logic in Rust, not in SQLite triggers
- use transactional compare-and-set updates for claim, start, cancel, and
  terminal transitions
- preserve WAL and the current single-writer discipline
- extend existing additive migrations with `ensure_column` before adding any
  dependent indexes
- store structured task failures using the shared failure payload model instead
  of opaque strings

## Consequences

Positive:

- adds durable work coordination without adding external infrastructure
- keeps leases, replay, and proof storage as separate honest subsystems
- gives future operator tooling a truthful task state model to project
- makes cancellation and abandoned ownership visible in replay instead of
  disappearing into logs

Negative:

- adds another state machine to the runtime and state layer
- stronger fairness claims remain blocked until authenticated ownership exists
- task-centric export remains layered on top of run-centric replay rather than
  becoming a first-class bundle on day one
- release proof will need new smoke coverage before any task surface can be
  called shipped

## Alternatives considered

Overload leases as tasks

- rejected because leases coordinate access to shared live resources; they do
  not model queued intent, assignment, retries, or cancellation

Reuse `scenario_runs` as the task plane

- rejected because scenario storage is proof-oriented and assumes named
  workflows, steps, assertions, and summary artifacts rather than operational
  queue ownership

External job queue such as Redis or RabbitMQ

- rejected because it breaks the standalone-tool posture and duplicates durable
  local state outside the existing SQLite store

Webhook-first or dashboard-first scheduler

- deferred because pengu mesh first needs truthful durable local state and
  replay linkage before it layers on operator notification or UI projections

## References

- `docs/current-status.md`
- `docs/product-requirements.md`
- `docs/feature-file-map.md`
- `docs/milestone-plan.md`
- `docs/agent-execution-charter.md`
- `docs/multi-agent-concurrency.md`
- ADR 0005
