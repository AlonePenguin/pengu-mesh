# ADR 0004: Local HTTP control plane

## Decision

Keep a local HTTP control plane as a first-class runtime surface next to MCP.

## Why

- useful for debugging, local automation, and operator tooling
- easier to inspect during bring-up
- separable from the stdio agent surface

