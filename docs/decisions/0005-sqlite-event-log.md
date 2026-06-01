# ADR 0005: SQLite plus append-only event log

## Decision

Use SQLite metadata plus an append-only event log for replay and diagnostics.

## Why

- local, explicit, and inspectable
- good fit for single-writer discipline
- enables replay bundles without external services

## Current implementation note

The shipped implementation keeps metadata, runs, and append-only events in the
same SQLite store and now exports replay manifests on top of that ordered
timeline in both `manifest_only` and portable bundle modes, with checksum-backed
artifact materialization and doctor validation on the portable path. The same
run/event substrate now carries derived crop grids, trace artifacts, and
bounded screenshot-recording archives without introducing a parallel evidence
store.
