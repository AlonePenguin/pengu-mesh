# Agent Handoffs

This directory is for durable handoff notes that help a future agent continue
work without reconstructing context from chat history.

Keep raw local proof bundles under ignored report directories such as
`reports/audit/`. Hand-off files should reference proof; they should not paste
large logs, machine-local secrets, browser caches, or bulky artifacts.

## Required Shape

```text
role:
task_id:
requested_by_holder_id:
assigned_holder_id:
commit:
scope:
files_read:
files_changed:
lease_or_capability_decisions:
commands_run:
artifacts:
scenario_or_run_ids:
outcome:
open_risks:
next_owner:
```

## What Belongs Here

- cross-turn execution handoffs
- release-proof summaries that point to local-gate directories
- scenario evidence notes with run IDs and artifact directories
- contract deltas for new CLI/MCP/HTTP surfaces
- unresolved risks that need a named next owner

## What Does Not Belong Here

- raw CI logs
- screenshots, PDFs, traces, or recordings
- copied browser profiles or runtime roots
- speculative strategy with no proof artifact
- private credentials, tokens, cookies, or local machine secrets

