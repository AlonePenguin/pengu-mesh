# Fresh Agent

This family proves cold-start readiness from a completely clean state with no
prior runtime data. It exercises the full bootstrap sequence an agent would
follow when connecting to pengu mesh for the first time.

## What it proves

- Zero-prior-state cold-start usability: every command succeeds against a
  freshly created, empty runtime root.
- Full bootstrap path: health, diagnostics, doctor, host-access checks,
  profile lifecycle, instance lifecycle, and tab inventory all function
  without any pre-existing state.

## How to run

```sh
./run.sh
```

The script creates a temporary runtime root, runs ten steps covering the
complete agent bootstrap sequence, and prints the scenario run id after a
successful finish. It records the current `diagnose.v1` schema contract,
checks that profile and instance inventory become non-empty from an empty
runtime root, and verifies that `tab-list` returns at least the initial browser
tab. Artifacts and the scenario summary stay inside the provided output
directory. The scenario run itself is persisted in the runtime SQLite database
under the isolated runtime root.
