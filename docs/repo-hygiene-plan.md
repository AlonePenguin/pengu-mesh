# Repo Hygiene Plan

This document records the current repo-bloat audit and the cleanup policy that
follows the `.claude/` integration pass on 2026-03-13.

## Goals

- keep `main` as the only production branch and remove repo-local staging
  workspaces once their useful code is integrated
- keep deep verification local-only and repo-owned while using GitHub CI for
  public contributor checks
- preserve only durable proof that advances the product story; prune browser
  caches, duplicate bundles, and superseded local evidence
- keep modern Rust-first product work and discard placeholder or half-wired
  clutter

## Audit Snapshot on 2026-03-13

- `.claude/` measured `48G` and existed only as a local worktree workspace for
  intermediate branch execution
- top-level repo size outside `.git/`, `.claude/`, `target/`, `node_modules/`,
  and `dist/` was dominated by `reports/` at `767136 KiB` (about `749 MiB`)
- the largest tracked files were mostly proof screenshots under
  `reports/local-gate/` plus baseline reference assets that have since been
  replaced with compact metadata
- the largest local-only files lived under `reports/audit/` and were mostly
  Chrome runtime caches or model blobs rather than durable proof:
  `optimization_guide_model_store`, `component_crx_cache`,
  `GraphiteDawnCache`, large replay tarballs, and raw snapshot dumps

## Decisions From This Pass

- integrate only the valuable `.claude` work that improves the current tree:
  narrow benchmark thresholds, the read-only dashboard scaffold, internal
  webhook and proxy helpers, and the browser-surface smoke fix
- keep any imported parity work honest as `foundation` or `partial` until it is
  wired through the public runtime contracts
- keep GitHub CI for fmt/check/test/dashboard build, while
  `./scripts/release/local-gate.sh` remains the deeper environment-sensitive
  production gate
- treat `.claude/` as disposable local execution state, not a permanent repo
  directory

## Immediate Cleanup Sequence

1. Commit and push the integrated tree on `main`.
2. Move `.claude/` to Trash and run `git worktree prune --expire now`.
3. Keep the latest passing local gate bundle and latest visual verification
   report easy to find.
4. Move superseded failed session-local audit bundles to Trash when a newer
   passing bundle proves the same contract.

## Next Cleanup Wave: Tracked Repo Bloat

- keep `reports/local-gate/` ignored by default and commit only reviewed
  summaries or explicitly justified artifacts
- replace duplicate proof bundles with summary Markdown plus representative key
  artifacts instead of carrying full repeated screenshots
- keep upstream baselines as metadata plus public source links unless a
  specific, reviewed fixture is required in-tree
- if tree cleanup is not enough and GitHub repo size stays unhealthy, plan a
  coordinated `git filter-repo` maintenance window for superseded heavy proof
  history rather than casually rewriting `main`

## Proof Retention Rules

- do not treat browser profile caches as durable proof
- do not retain these directories inside promoted proof bundles unless a
  specific investigation explicitly depends on them:
  - `GraphiteDawnCache/`
  - `optimization_guide_model_store/`
  - `component_crx_cache/`
  - equivalent Chrome runtime caches under profile roots
- durable proof should usually be a compact summary plus selected JSON, small
  screenshots, and the exact contract outputs that justify a claim
- promote only the latest or contract-defining bundles from `/tmp` or
  `reports/audit/` into tracked paths

## Workflow Efficiency Follow-Ups

- add a repo-owned prune step before promoting audit bundles so runtime caches
  do not become durable clutter
- add a size-audit helper that reports the largest tracked proof bundles and
  the heaviest ignored local audit directories
- add a small retained-bundle index under `reports/` so each committed bundle
  has an explicit reason to exist
- use archive-or-trash review points after major milestone closures instead of
  letting old bundles accumulate indefinitely
- before publication, prefer a sanitized public tree over exposing old private
  report history

## Done Criteria

- `.claude/` is gone from the repo root
- `git worktree list` no longer references `.claude/worktrees/...`
- superseded failed local audit bundles from the active integration pass are no
  longer sitting in the repo tree
- the retained tracked proof set is curated on purpose rather than by
  accumulation
- the promotion and pruning workflow is documented and repeatable
