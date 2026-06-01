# Local Gate Output

Raw local-gate proof bundles may be written here, but they are ignored by
default because they can contain local paths, screenshots, browser metadata,
and machine posture.

Use [`../../scripts/release/promote-local-gate-bundle.sh`](../../scripts/release/promote-local-gate-bundle.sh)
only when a raw local-gate output directory needs a curated durable form.

Before forcing any generated report into git:

- keep summaries, machine-readable JSON, status files, and proof artifacts such
  as screenshots, snapshots, and extracted text
- drop disposable browser profile caches, runtime SQLite databases, and `.log`
  noise that make the raw bundle large but do not improve reviewability
- remove private page content, local usernames, absolute host paths, secrets,
  tokens, cookies, and machine-specific posture that does not need to be public

Raw gate runs may begin in `/tmp` or another temporary output directory during
execution. Public docs should claim only proof that has been reviewed and can
be regenerated.
