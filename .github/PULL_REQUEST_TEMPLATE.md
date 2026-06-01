## Summary

-

## Verification

- [ ] `cargo fmt --all --check`
- [ ] `cargo check --workspace`
- [ ] `cargo test --workspace`
- [ ] `npm --prefix web/dashboard run build` when dashboard code changes
- [ ] Relevant smoke or local-gate proof for runtime/browser/host behavior

## Safety

- [ ] No secrets, tokens, cookies, private screenshots, browser profiles, or raw
      machine-local proof bundles are committed
- [ ] Diagnostics stay read-only
- [ ] Structured failure and recovery behavior is documented when applicable
