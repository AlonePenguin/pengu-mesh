# Security Policy

pengu mesh is a local automation/control-plane project. Security work here is
about preserving user trust on a machine where browser automation, local
artifacts, macOS permissions, and agent coordination meet.

## Supported versions

The public `main` branch is the supported development line until the project
publishes tagged releases.

## Reporting vulnerabilities

Please report suspected vulnerabilities through GitHub Security Advisories for
this repository when available. If advisories are not available, open a GitHub
issue with minimal details and ask for a private contact path.

Do not include secrets, private screenshots, cookies, browser profiles, or
local machine dumps in public issues.

## Current boundaries

- The daemon binds locally by default.
- External browser attach requires `PENGU_MESH_ALLOW_EXTERNAL_ATTACH=1`.
- Holder IDs are trusted-local coordination labels, not authentication.
- `diagnose`, `health`, and `doctor` must remain read-only.
- Raw proof outputs can contain local paths, screenshots, browser metadata, and
  host permission state. They are ignored by default.

## Known security work still open

- Authenticated holder identity.
- Stronger dangerous-capability gating.
- More explicit risk tiers for actions that move from background-safe channels
  to app takeover or global takeover.
- Broader policy around long-lived artifact retention.

## Maintainer checklist for risky changes

- State what host capability is being used.
- State whether the command mutates browser, app, OS, filesystem, or network
  state.
- Return a structured failure with recovery guidance.
- Add tests or smoke proof for the denial path as well as the success path.
- Keep local proof out of git unless it has been curated for public release.
