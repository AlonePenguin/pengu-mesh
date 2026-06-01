# ADR 0006: Dashboard is secondary

## Decision

Treat the dashboard as a later consumer, not the initial product center.

## Why

- keeps the daemon, doctor, and MCP surfaces clean
- prevents UI work from setting premature runtime dependencies
- aligns with the docs-first tool-first bootstrapping plan

