# ADR 0002: macOS arm64 is Tier 1

## Decision

Treat `darwin/arm64` as the design center and first full validation target.

## Why

- it matches the actual operator machine
- it provides the benchmark reference environment for the dependency shootout
- it lets the repo optimize for real usage while preserving cross-platform seams

