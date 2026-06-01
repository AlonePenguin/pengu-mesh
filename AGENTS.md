# pengu mesh Repo Rules

## Product posture

- The standalone tool is the product. Do not lead with downstream integrations.
- Performance is a product feature. Hot-path abstractions and dependencies must
  justify themselves with measurement, correctness, or both.
- `darwin/arm64` is Tier 1 and the benchmark reference platform.
- Cross-platform support must remain structurally possible in core crates.

## Naming scheme (hard cutover, no aliases)

- Human-facing name: pengu mesh
- GitHub slug: pengu-mesh
- Binaries: pengu-mesh, pengu-mesh-doctor, pengu-mesh-mcp, pengu-mesh-cli
- Crates/packages: pengu-mesh-core, pengu-mesh-mcp, pengu-mesh-macos,
  pengu-mesh-cli, pengu-mesh-state, pengu-mesh-shared, etc.
- Rust modules: pengu_mesh_core, pengu_mesh_mcp, pengu_mesh_macos, etc.
- Never use aliases, abbreviations, or old names.

## Operational principles (govern all work)

1. **Robustness over speed.** Correctness, honest failure reporting, and
   recoverable state over raw performance. Every contract surface must return
   honest results - no silent fallbacks, no false success, no ambiguous states.
2. **Diagnostic trust.** Status, health, diagnose, and doctor are truthful
   read-only probes that never trigger side effects, state changes, or prompts.
   Agents and operators must trust that checking status never changes state.
3. **Agent self-enablement.** The architecture helps agents discover the
   permissions they need and guide host enablement without manual per-step user
   configuration. Zero human configuration is the goal.
4. **Multi-surface meshing.** Browser control is only the first production
   slice. Language, schemas, and contracts must never narrow to "browser tool."

## Agent execution standards

1. Think Before Coding
Don't assume. Don't hide confusion. Surface tradeoffs.

Before implementing:

State your assumptions explicitly. If uncertain, ask.
If multiple interpretations exist, present them - don't pick silently.
If a simpler approach exists, say so. Push back when warranted.
If something is unclear, stop. Name what's confusing. Ask.
2. Simplicity First
Minimum code that solves the problem. Nothing speculative.

No features beyond what was asked.
No abstractions for single-use code.
No "flexibility" or "configurability" that wasn't requested.
No error handling for impossible scenarios.
If you write 200 lines and it could be 50, rewrite it.
Ask yourself: "Would a senior engineer say this is overcomplicated?" If yes, simplify.
3. Goal-Driven Execution
Define success criteria. Loop until verified.

Transform tasks into verifiable goals:

"Add validation" -> "Write tests for invalid inputs, then make them pass"
"Fix the bug" -> "Write a test that reproduces it, then make it pass"
"Refactor X" -> "Ensure tests pass before and after"
For multi-step tasks, state a brief plan:

1. [Step] -> verify: [check]
2. [Step] -> verify: [check]
3. [Step] -> verify: [check]
Strong success criteria let you loop independently. Weak criteria ("make it work") require constant clarification.

## Architecture rules

- Keep core runtime logic in Rust.
- Own the stdio MCP implementation directly in this repo.
- Keep platform-specific code behind narrow shims.
- Treat `reference/upstream/*.METADATA.json` as read-only baseline material
  unless an explicit refresh task updates the comparison reference.

## Structured failure contracts

- Every command that can fail must return a structured failure payload, not an
  opaque error string.
- Structured failures must include: what was attempted, why it failed, what the
  agent could do to recover, and whether retry is likely to help.
- The shared crate (`pengu-mesh-shared`) is the single source of truth for
  failure payload types and recovery heuristic builders. Never duplicate
  failure logic across crates.
- If a new failure type is needed, define it in shared first, then import it in
  core and transport crates.

## API visibility rules

- Helper functions in core crates must be `pub(crate)` unless they are part of
  the documented contract surface.
- Never make a function `pub` solely for test access. Tests within the same
  crate can access `pub(crate)` items.
- If a transport crate (MCP, HTTP, CLI) needs logic from core, either expose it
  through a runtime method or move the shared logic to `pengu-mesh-shared`.
- Never duplicate logic across crates. If two crates need the same function, it
  belongs in shared.

## Dependency rules

- Prefer small, current crates with minimal feature flags.
- No framework-heavy persistence layer.
- No plugin-based MCP bridge.
- Benchmark hot-path candidates before treating them as stable decisions.
- Every new dependency should be documented in
  `docs/dependency-policy.md` or a decision record when it affects a core path.

## Workflow rules

- Keep docs, code, and verification outputs synchronized in the same pass.
- For environment or permission changes, record before and after proof in
  `reports/audit/`, but do not commit raw machine-local bundles unless they
  have been reviewed and intentionally curated.
- When baseline assumptions change, update the relevant doc and ADR in the same
  commit.
- If progress stalls, change strategy: reproduce, isolate, instrument, simplify, compare alternatives, gather more evidence, or research better approaches.
- Maximize your multi-modal capabilities, use your vision as often as possible to check work (screenshots/screencaptures), and all relevant other inherint-capabilities. A single modality is not sufficient for true confirmation of something being complete or final.
- Maximize your ability to orchestrate and delegate parallel and long running tasks by assigning sub-agent ownership and roles as often as possible. Strive to maximize your concurrent progression abilities for all work.
- Prefer high parallelism on large, separable tasks. Keep 20 to 24 sub-agents active when there is enough independent work to justify it, while respecting configured thread and depth limits.
- The host must have `ulimit -n 65536` or higher active before launching high-concurrency swarms. The macOS default is too low for 20+ parallel agents. See the repo-local setup notes if the limit has not been raised.

## Commit discipline

- Run `cargo fmt --all` after every significant edit, before running
  `cargo check`.
- Run `cargo fmt --all --check` before every commit. If it fails, fix
  formatting first.
- Never mix formatting fixes with functional changes in the same commit.
- Commit after each logical unit of work is green, not at the end.
- Use descriptive commit messages that say what was built or fixed and why.
- Push immediately after each commit. Do not batch pushes.
- Every commit must individually pass `cargo fmt --all --check`,
  `cargo check --workspace`, and `cargo test --workspace`.

## Verification expectations

- `cargo fmt --all --check` must pass before every commit.
- `cargo check --workspace` and `cargo test --workspace` should stay green.
- `./scripts/release/local-gate.sh` must pass at every phase boundary and
  before calling work done.
- Smoke scripts (`browser-surface-smoke.sh`,
  `browser-lifecycle-integration.sh`, `diagnose-smoke.sh`,
  `tab-lifecycle-integration.sh`) are real contract verification, not optional.
- Benchmark harnesses must at least compile before a hot-path decision is
  considered ready.
- `pengu-mesh diagnose` is the agent-facing readiness check. It returns
  structured machine-readable remediation guidance.
- `pengu-mesh-doctor` is the operator-facing diagnostic. It provides
  human-readable health information.
- Use `diagnose` when an agent needs to self-enable. Use `doctor` when a human
  operator is debugging.
