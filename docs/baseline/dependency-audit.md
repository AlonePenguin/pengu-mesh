# Dependency Audit

## Upstream top-level direct dependencies

From the recorded PinchTab upstream audit at
`pinchtab/pinchtab@804ba5b8fca7ba0e54683f82209ce8de48656a36`:

- `chromedp/cdproto`
- `chromedp/chromedp`
- `gobwas/ws`
- `shirou/gopsutil/v4`
- `yaml.v3`

## Architecture implications

- Go provides a compact single-binary posture today.
- `chromedp` is productive but keeps the runtime coupled to its control style.
- the same repo also carries dashboard, plugins, npm packaging, and docs
  surfaces, which increases non-runtime scope inside the main product tree.

## pengu mesh direction

- preserve the useful product model
- move to Rust for tighter hot-path control and clearer crate seams
- benchmark each hot dependency class before committing long term
