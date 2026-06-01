# Initialization Audit

## Scope

- verified checkout root at audit time: `/path/to/pengu-mesh`
- repository remote: `AlonePenguin/pengu-mesh`
- upstream baseline snapshot: `pinchtab/pinchtab@804ba5b8fca7ba0e54683f82209ce8de48656a36`
- raw audit bundle: generated under ignored local report storage

## Verified host state

- `sudo -n true` succeeds for the current user
- `DevToolsSecurity -status` reports developer mode enabled
- `rustup` is installed and active with `stable-aarch64-apple-darwin`
- `pengu-mesh-doctor` sees `git`, `gh`, `rustup`, `cargo`, `rustc`, `go`, `jq`,
  `sqlite3`, `security`, and `DevToolsSecurity`
- `Google Chrome.app` and `Google Chrome Dev.app` are installed

## TCC summary

From the filtered export for `com.openai.codex`, `com.apple.Terminal`,
`com.google.Chrome`, and `com.google.Chrome.dev`:

- `com.openai.codex` has Accessibility, Developer Tool, Input Monitoring,
  Screen Recording, and Full Disk Access sender-wide grants in the system DB
- `com.apple.Terminal` also has the same broad sender-wide grants
- `com.google.Chrome.dev` has Accessibility, Developer Tool, Input Monitoring,
  Screen Recording, and Full Disk Access sender-wide grants
- `com.google.Chrome` has Input Monitoring and Screen Recording, but not Full
  Disk Access in the system export
- user-database Apple Events rows already exist for Codex to both
  `com.google.Chrome` and `com.google.Chrome.dev`

## Automation inventory

- app inventory built successfully from installed bundles
- automation matrix built successfully for sender `com.openai.codex`
- broad probe after inventory normalization reported:
  - scanned apps: 81
  - excluded apps: 2
  - seeded in TCC: 39
  - AppleEvents get-name ok: 81
  - version query ok: 81
  - window count ok: 21
  - launch ok: 81
  - running seen after launch: 77

## Notes

- The provided `probe_automation_inventory.py` expects a different inventory
  schema than `build_app_inventory.py` emits. The repo now includes
  `scripts/doctor/normalize_inventory_for_probe.py` to bridge that mismatch for
  repeatable audits.
- No new host permission widening was needed during initialization because the
  required sudo and developer-tool gates were already satisfied.
