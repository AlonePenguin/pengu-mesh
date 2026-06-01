# ADR 0001: Rust-first runtime

## Decision

Use Rust for the core runtime, contracts, and diagnostics.

## Why

- stronger control over hot paths
- clear crate seams for transport, state, and artifacts
- good fit for explicit local binaries and benchmark-driven design

