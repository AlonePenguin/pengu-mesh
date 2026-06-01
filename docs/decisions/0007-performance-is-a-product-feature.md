# ADR 0007: Performance is a product feature

## Decision

Apply benchmark-first review to hot dependencies, serialization, artifact paths,
and attach lifecycle.

## Why

- avoids cleanup-driven performance work
- makes runtime cost visible during architecture decisions
- sets the quality bar before the feature surface expands

