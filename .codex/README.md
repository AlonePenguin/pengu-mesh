# Project Codex Multi-Agent Roles

This repo ships a project-scoped Codex package in [`config.toml`](config.toml)
and [`AGENTS.md`](AGENTS.md).

The role pack is tuned for this project's actual work:

- Rust runtime and daemon implementation
- Chrome DevTools / attach / multimodal browser proof
- restart continuity, leases, and SQLite-backed concurrency correctness
- contract parity across CLI, MCP, HTTP, diagnose, and doctor
- benchmark-first performance work on `darwin/arm64`
- local-only release gating and audit evidence
- explicit capability comparison against `reference/upstream/pinchtab.METADATA.json`

Shared project defaults live in [`config.toml`](config.toml), while the
specialist files under [`agents/`](agents/) now override only the settings that
matter for that role. The committed defaults are public-safe: workspace-write
sandboxing and approval on request. Maintainers can opt into broader local
permissions in their own uncommitted config when a trusted release gate needs
them.

The built-in roles are overridden with repo-specific behavior, and the
repo-scoped role pack lives across [`config.toml`](config.toml),
[`agents/`](agents/), and [`AGENTS.md`](AGENTS.md).

Instruction discovery for this repo intentionally keeps both
[`../AGENTS.md`](../AGENTS.md) and [`AGENTS.md`](AGENTS.md):

- [`../AGENTS.md`](../AGENTS.md) remains the repo-wide instruction source for
  every file in the checkout.
- [`AGENTS.md`](AGENTS.md) remains in `.codex/` as a local handoff file that
  points back to the authoritative repo rules instead of re-copying the full
  instruction body.
- The second file is intentional rather than drift: it keeps the project-local
  package self-describing without forcing another full duplicated rule block.

The main role families are:

- `default`, `worker`, `explorer`, `monitor`
- `reviewer`
- `runtime_owner`
- `cdp_attach_specialist`
- `observability_owner`
- `concurrency_steward`
- `perf_bench_lead`
- `contract_keeper`
- `readiness_contract_owner`
- `browser_operator`
- `release_auditor`
- `pinchtab_gap_hunter`
- `scenario_harness_owner`

When the repo is trusted, Codex loads the project-scoped `.codex/` package,
including config, specialist agents, and mirrored agent instructions.

Browser capability in this repo is native-first:

- the product path is the Rust runtime plus repo-owned macOS helpers under
  [`../scripts/browser/`](../scripts/browser/)
- external browser MCP servers are optional investigation aids, not required
  product dependencies
- the current verified host keeps Playwright MCP disabled, so repo workflows
  must remain fully operable without it

The current owned helpers cover Accessibility-backed element pressing plus
GUI-level address-bar navigation for Google Chrome Dev.

The browser roles intentionally split attach mechanics from validation:

- `cdp_attach_specialist` owns Chrome discovery, remote-debugging prompts,
  endpoints, target enumeration, and reconnect continuity.
- `browser_operator` owns post-attach reproduction, screenshots, console and
  network evidence, and UI/control-plane parity checks.

The repo also now gives two previously ownerless gaps an explicit home:

- `readiness_contract_owner` owns `diagnose`, host-access readiness, and
  remediation-contract parity for agent self-enablement.
- `scenario_harness_owner` owns named scenario packs, weak-prompt and
  fresh-agent validation, metrics-backed comparison work, and the path from
  scenario outputs into release proof.

The write-capable roles also encode the repo's real proof chain instead of a
generic “verify it” rule:

- use `pengu-mesh diagnose` first when agent readiness or remediation truth is
  unclear
- use `pengu-mesh-doctor` when the task needs operator-facing health or
  human-readable debugging output
- keep CLI, MCP, HTTP, docs, and audit proof synchronized in the same pass
- treat `cargo check --workspace` and `cargo test --workspace` as the minimum
  verification floor
- require bench discovery or bench compilation before locking hot-path
  decisions
