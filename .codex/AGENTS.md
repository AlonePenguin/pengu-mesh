# .codex Subtree Instructions

- This subtree inherits and follows the repo-wide rules in `../AGENTS.md`.
- The host must have `ulimit -n 65536` or higher active before launching high-concurrency swarms. The macOS default is too low for 20+ parallel agents. See the repo-local setup notes if the limit has not been raised.
- No additional `.codex`-only overrides are defined here.
- Keep this file as a local handoff so humans and tooling that enter `.codex/`
  immediately see where the authoritative repo instructions live.
