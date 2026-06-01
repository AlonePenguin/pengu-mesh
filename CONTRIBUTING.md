# Contributing to pengu mesh

Thanks for helping make local agent work more trustworthy.

pengu mesh values small, verified changes. A good contribution explains the
problem, changes the narrowest useful surface, and proves the result.

## Start here

1. Read [README.md](README.md) for the product shape.
2. Read [AGENTS.md](AGENTS.md) for repo rules.
3. Check [docs/current-status.md](docs/current-status.md) so your claim matches
   the current implementation.
4. Check [docs/implementation-backlog.md](docs/implementation-backlog.md) for
   useful open work.

## Local setup

```bash
./scripts/dev/bootstrap-rust.sh
make check
make test
npm --prefix web/dashboard ci
npm --prefix web/dashboard run build
```

The full gate is:

```bash
make local-gate
```

It exercises real browser and host behavior, so it may require macOS
permissions and Chrome Dev. If it fails because the host is not ready, include
the `diagnose` or `doctor` output in your notes.

## Pull request expectations

- Keep the change focused.
- Add or update tests for code behavior.
- Update docs when a contract, command, or workflow changes.
- Do not commit raw local proof bundles, browser profiles, screenshots of
  private pages, absolute local paths, API keys, tokens, or machine posture.
- Prefer structured failure payloads over opaque error strings.
- Keep diagnostics side-effect-free.
- Run `cargo fmt --all --check`, `cargo check --workspace`, and
  `cargo test --workspace` before opening a PR.

## Good first issues

Good first contributions are usually:

- docs that make a command clearer
- tests for existing typed failures
- small dashboard improvements that remain read-only
- scenario examples under `examples/workflows/`
- bug reports with exact `diagnose` output and reproduction steps

## Security-sensitive work

If your change touches host permissions, browser attach, native accessibility,
external URLs, file paths, subprocesses, or stored artifacts, read
[SECURITY.md](SECURITY.md) first. The burden is on the change to make risk
visible and recoverable.
