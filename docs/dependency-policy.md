# Dependency Policy

## Acceptance rules

- default to small, explicit crates with minimal feature flags
- require measurable value on hot paths
- prefer direct control over hidden middleware
- avoid ORM-style abstractions for local SQLite
- avoid plugin layers for MCP
- do not add dependencies or compatibility layers just to preserve legacy code
  paths that the active product no longer needs
- when a dependency only serves stale or partial behavior, remove the behavior
  instead of keeping a backwards-compatibility shim alive

## Phase 0 benchmarks

Before locking hot-path choices, benchmark:

- `serde_json` vs `simd-json` vs `sonic-rs`
- `hyper` control-plane routing cost with and without higher-level wrappers
- direct `rusqlite` patterns for single-writer access
- low-overhead WebSocket/CDP transport candidates

## Current Stage 1 core choices

- `rusqlite` remains the local state store for instance, tab, and artifact
  metadata because it keeps persistence explicit and inspectable without adding
  ORM-style indirection.
- `ureq` is the current local DevTools HTTP client for `/json/version`,
  `/json/list`, `/json/new`, and `/json/close` because the Stage 1 control path
  is local-only and does not justify a heavier async stack yet.
- `tungstenite` is the current Stage 1 WebSocket transport for direct CDP calls
  used by snapshot, text, screenshot, and PDF operations.
- `base64` is used only for Chrome artifact decode paths (`captureScreenshot`,
  `printToPDF`, and screenshot-frame recording capture) so large payloads can
  be written straight to disk after decode without inventing custom codecs.
- `sha2` is used for replay portability and doctor verification so portable
  bundles can prove artifact integrity without a heavier archival format.
- `image` is used with PNG-only support for screenshot and rendered-PDF crop
  generation because the new `artifact_crop` and `artifact_crop_grid` paths
  need deterministic local image slicing without introducing a larger
  multimedia stack.
- `tar` is used for recording capture packaging because bounded screenshot
  frame sequences need a simple streamable archive format that stays local,
  inspectable, and cheap to materialize inside replay bundles.
- `pdftoppm` is now a verified host tool for PDF-to-image rendering during
  PDF crop generation on the active Tier 1 machine; doctor output reports its
  availability explicitly.
- `react`, `react-dom`, `vite`, and `@vitejs/plugin-react` are limited to the
  read-only operator console scaffold under `web/dashboard/`; they are kept off
  the Rust runtime hot path and remain secondary to the CLI, doctor, MCP, and
  HTTP truth surfaces.

These choices are implementation defaults, not permanent doctrine. They stay
approved only while the benchmark and smoke artifacts justify them on
`darwin/arm64`.
