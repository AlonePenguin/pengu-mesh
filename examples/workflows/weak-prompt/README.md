# Weak Prompt

This family exercises the tool surface with deliberately under-specified or
ambiguous inputs. Each probe sends a malformed or missing-context request and
asserts that the CLI returns a structured failure payload with actionable
recovery guidance rather than an opaque error string.

What it proves:

- Structured failure payloads are returned for malformed and ambiguous inputs.
- Recovery guidance is present and of sufficient quality to steer an agent back
  on track without human intervention.
- The failure payload preserves enough request context for a follow-on agent to
  understand what was attempted.

`run.sh` covers:

- missing instance + tab via `tab-list-actions`
- missing artifact via `artifact-verify`
- invalid CDP URL via `instance-attach`
- missing tab via `tab-action` (navigate)
- missing run via `replay-export`
- missing instance + bad surface via `browser-surface-action`

The invalid URL probe temporarily sets
`PENGU_MESH_ALLOW_EXTERNAL_ATTACH=1` for that isolated command so the scenario
reaches URL parsing instead of stopping at the external-attach safety gate.

## How to run

```bash
./run.sh
```
