# Product Requirements

The product is not complete when it only compiles, tests, and exposes browser
primitives. The product is complete when a fresh agent can use it to perform
real browser tasks with low confusion, strong evidence capture, and fast
diagnosis when things go wrong.

## Core non-negotiables

- the standalone tool is the product
- Rust runtime behavior is primary
- `darwin/arm64` is the design-center performance platform
- Google Chrome Dev is the primary target browser
- `pengu-mesh-doctor` is the first-pass readiness truth source
- CLI, MCP, and HTTP must stay contract-aligned
- live-web validation is part of product truth, not optional garnish
- comparative evidence against PinchTab is required before claiming superiority
- fresh-agent usability is a product surface, not a doc-quality side effect
- stored metrics are required for serious performance and usability claims

## V1 shipped baseline

- local daemon bootstrap
- Chrome discovery and attach model
- profile, instance, and tab lifecycle model
- navigation-grade accessibility snapshot contract definitions
- screenshot contract definitions including full-page capture
- text and PDF contract definitions
- native stdio MCP tool catalog and response envelope
- first-pass doctor and audit output

## V2 shipped baseline

- append-only replay model
- SQLite metadata store
- artifact crop grids and portable replay bundles
- trace and recording capture
- explicit typed failure codes
- stronger artifact bundles for agent handoff

## V3 materially advanced baseline

- multi-agent leases
- writer exclusivity and observer mode
- continuity across daemon restarts
- attach continuity outcome and freshness
- endpoint-aware reuse with endpoint-refresh persistence
- macOS host-access capability matrix and setup flow
- native browser-surface list, snapshot, and action surfaces
- local production gate with diagnose, lease, continuity, attach continuity,
  host-access, browser-lifecycle, tab-lifecycle, evidence-chain, and
  browser-surface proof lanes

## V4 required next baseline

- durable metrics database for scenario, latency, failure, and usability results
- repeatable live-web scenario harness with named scenario families and stored results
- weak-prompt and fresh-agent usability validation
- first thresholded performance budgets after repeated `darwin/arm64` baselines
- recurring comparative scenarios against PinchTab with stored evidence bundles
- stronger attached-browser native-surface identity than the current app-name/PID fallback model

## V5 required product-hardening baseline

- authenticated holder ownership beyond trusted-local coordination
- enforced dangerous-capability gating for high-risk actions beyond the current
  visible risk posture
- operator console that turns doctor, replay, lease, and continuity truth into a clearly better workflow
- durable task plane above leases with ownership, fairness or admission control, cancellation, and replay linkage
- first-run and install posture that beats source-only onboarding
