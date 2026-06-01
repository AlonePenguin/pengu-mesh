# Structured Failure

This family records the first named structured-failure corpus for pengu mesh.
Each run probes one major failure path and asserts that the CLI returns the
standard failure envelope with actionable recovery guidance instead of an opaque
string.

`run.sh` covers:

- missing instance via `tab-list`
- missing tab via `tab-snapshot`
- missing artifact via `artifact-verify`
- missing scenario run via `scenario-run-detail`
- external attach disabled via `instance-attach`
- duplicate managed profile via a repeated `profile-create`

Every probe records latency and stores assertions for `ok`, `code`,
`operation`, `attempted`, `reason`, `recovery`, and `retry_likely`.
