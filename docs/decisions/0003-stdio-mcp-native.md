# ADR 0003: Native stdio MCP

## Decision

The repo owns the stdio MCP runtime directly.

## Why

- avoids plugin indirection
- keeps tool contracts aligned with runtime semantics
- improves observability and typed failure design

