# Workflow Examples

This directory is now the real in-repo home for named scenario families and
workflow proof, not a placeholder for future intent.

Long-lived scenario definitions belong here rather than living only in prompts,
`/tmp`, or ad hoc audit notes.

Each shipped family should have:

- a short family README
- a runnable `run.sh`
- stored assertions and latency samples in the scenario metrics database
- obvious output paths for artifacts or summaries

## Current families

- `startup-readiness/`
  - proves isolated-runtime health and diagnose, managed headless startup,
    screenshot capture, artifact verification, and clean shutdown
  - wired into `scripts/release/local-gate.sh`
- `evidence-chain/`
  - proves snapshot, screenshot, and text capture plus post-corruption
    invalidation without mutating stored artifact metadata
- `structured-failure/`
  - probes named failure paths and records the structured failure envelope for
    each one
- `live-web/`
  - proves that managed Chrome Dev can fetch a real public page, capture
    snapshot/screenshot/text artifacts from live content, and verify the stored
    artifact inventory plus checksums
- `weak-prompt/`
  - probes malformed or missing-context requests and records whether the
    recovery guidance is specific enough for the next agent step
- `fresh-agent/`
  - proves cold-start health, diagnose, doctor, host-access, profile lifecycle,
    managed headless launch, tab inventory, and clean shutdown from an empty
    runtime root
- `operator-diagnosis/`
  - proves that `diagnose`, `health`, `doctor --json`, `host-access-status`,
    and `lease-status` stay mutually consistent and leave behind a durable
    summary path in scenario detail
- `pinchtab-comparison/`
  - measures pengu mesh performance on standard operations and generates a
    structured comparison report tied to commit, platform, artifacts, and the
    PinchTab comparison target

## What still belongs here next

- repeated live-web drills that cover more than the current single-page proof
- weak-prompt recovery drills
- fresh-agent recovery drills
- broader operator-diagnosis drills
- repeatable PinchTab comparison reruns and leaderboard hooks
- reusable workflow setup notes

## What does not belong here

- per-run temporary output
- one-off local scratch scripts
- unverifiable prose plans with no scenario definition

The authoritative tracking docs for the remaining gaps are:

- `docs/implementation-backlog.md`
- `docs/feature-file-map.md`
- `docs/milestone-plan.md`
