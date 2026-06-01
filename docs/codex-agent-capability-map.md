# Codex Agent Capability Map

This document records the verified Codex capability posture that backs this
repo as of **March 12, 2026**.

Verification inputs for this refresh:

- `codex --version` -> `codex-cli 0.114.0`
- `codex features list`
- `codex mcp list`
- `~/.codex/config.toml`
- [`../.codex/config.toml`](../.codex/config.toml)
- [`../AGENTS.md`](../AGENTS.md)
- [`../.codex/AGENTS.md`](../.codex/AGENTS.md)

Reference docs:

- [Codex Multi-agents](https://developers.openai.com/codex/multi-agent/)
- [Codex Config Reference](https://developers.openai.com/codex/config-reference/#configtoml)

## Global Codex posture on the verified host

- model stack
  - `model = "gpt-5.4"`
  - `review_model = "gpt-5.4"`
  - `model_reasoning_effort = "high"`
  - `plan_mode_reasoning_effort = "xhigh"`
  - `model_reasoning_summary = "concise"`
  - `model_verbosity = "medium"`
- execution posture
  - `approval_policy = "never"`
  - `sandbox_mode = "danger-full-access"`
  - `allow_login_shell = true`
  - `forced_login_method = "chatgpt"`
  - `web_search = "live"`
- project instruction discovery
  - `project_doc_fallback_filenames = ["AGENTS.md", "README.md", "README.txt", "README"]`
  - `project_doc_max_bytes = 900000`
- multi-agent scale
  - `agents.max_threads = 64`
  - `agents.max_depth = 8`
  - `agents.job_max_runtime_seconds = 14400`
- repo-relevant feature flags enabled
  - `features.multi_agent = true`
  - `features.runtime_metrics = true`
  - `features.child_agents_md = true`
  - `features.prevent_idle_sleep = true`
  - `features.responses_websockets = true`
  - `features.responses_websockets_v2 = true`
  - `features.artifact = true`
  - `features.image_detail_original = true`
  - `features.guardian_approval = true`
  - `features.js_repl = true`
  - `features.voice_transcription = true`
  - `features.realtime_conversation = true`
  - `features.shell_tool = true`
  - `features.unified_exec = true`
  - `features.shell_snapshot = true`
  - `features.enable_request_compression = true`
  - `features.skill_mcp_dependency_install = true`
  - `features.undo = true`
  - `features.apps = true`
  - `features.apps_mcp_gateway = false`

## MCP and browser capability posture on the verified host

- enabled MCP servers
  - OpenAI Developer Docs
  - GitHub
  - Context7
  - Chrome DevTools MCP
- disabled MCP servers
  - Playwright MCP
  - filesystem MCP
- browser target posture
  - the repo target remains Google Chrome Dev
  - Chrome DevTools MCP is available as an operator aid
  - Playwright MCP is configured in the global file but currently disabled and
    must not be treated as a required dependency for repo workflows

## Repo-scoped capability map

The project adds a role-specialized team in
[`../.codex/config.toml`](../.codex/config.toml),
[`../.codex/agents/`](../.codex/agents/), and mirrored repo instruction files
at [`../AGENTS.md`](../AGENTS.md) and [`../.codex/AGENTS.md`](../.codex/AGENTS.md).

Repo-scoped defaults now intentionally narrow the generic global posture:

- `model_reasoning_summary = "concise"`
- `model_verbosity = "medium"`
- `forced_login_method = "chatgpt"`
- `project_doc_max_bytes = 65536`
- `background_terminal_max_timeout = 3600000`
- `tools.view_image = true`
- `agents.max_threads = 24`
- `agents.max_depth = 3`

Specialized roles include:

- `default`
- `worker`
- `explorer`
- `monitor`
- `reviewer`
- `runtime_owner`
- `cdp_attach_specialist`
- `concurrency_steward`
- `observability_owner`
- `perf_bench_lead`
- `contract_keeper`
- `readiness_contract_owner`
- `browser_operator`
- `release_auditor`
- `pinchtab_gap_hunter`
- `scenario_harness_owner`

These roles are intended to let the main orchestrator spawn narrowly-owned,
high-autonomy sub-agents instead of overusing generic workers. The repo-local
role pack now keeps shared defaults in [`../.codex/config.toml`](../.codex/config.toml)
and limits per-role files to the settings and instructions that truly differ.

Because `features.child_agents_md = true` is enabled in both the top-level and
repo-scoped config, the repo intentionally keeps `.codex/AGENTS.md` in-tree as
the local handoff file for the project package. The authoritative instruction
body stays at the repo root in [`../AGENTS.md`](../AGENTS.md); the `.codex`
copy exists so humans and tooling that enter the package locally still see the
instruction path immediately.

## Popup and prompt handling posture

The current machine allows macOS UI scripting against Google Chrome Dev, which
means Codex is not limited to DOM-only prompt handling.

The confirmed live path for Chrome Dev browser-native prompts is:

- `System Events` can see the app and window, but name-only button enumeration
  is too weak for Chrome Dev prompt handling.
- Button labels can appear in `AXDescription`, `AXTitle`, or `AXValue`, so the
  helper needs native Accessibility traversal rather than plain AppleScript
  `name` matching.
- Traversing the full Chrome Dev application Accessibility tree through
  `ApplicationServices` is more reliable than scanning only `windows`.
- The Rust runtime now owns first-pass remote-debugging sheet recovery in
  `pengu-mesh-cdp`, with repo-owned shell helpers retained for direct diagnosis and
  fallback proof.

Use this escalation order:

1. Product path:
   - prefer the repo runtime's native Chrome Dev prompt recovery
   - keep browser workflows executable without Playwright MCP
2. Direct diagnosis or fallback:
   - [`../scripts/browser/chrome-dialog-click.sh`](../scripts/browser/chrome-dialog-click.sh)
   - [`../scripts/browser/chrome-allow-remote-debugging.sh`](../scripts/browser/chrome-allow-remote-debugging.sh)
   - [`../scripts/browser/chrome-dev-navigate.sh`](../scripts/browser/chrome-dev-navigate.sh)
   - use the Accessibility-backed helper path before falling back to keyboard
     tabbing
3. Optional operator assist:
   - use Chrome DevTools MCP when extra DOM, console, or network inspection
     helps
   - do not assume Playwright MCP availability
4. Verification:
   - confirm the blocking prompt is gone
   - confirm the browser workflow resumes
   - confirm no helper process is left behind

## Latest-doc notes that matter

- `agents.<name>.nickname_candidates` is supported and can improve thread
  readability in large multi-agent runs.
- `agents.job_max_runtime_seconds` is part of the supported multi-agent schema
  and matters for long-lived delegated work.
- repo-scoped config can safely narrow depth, thread count, verbosity, and
  project-doc byte limits relative to the global config
- The built-in `monitor` role is explicitly intended for waiting and polling.
- Project-scoped `.codex/config.toml` files are the documented place for
  project-specific role packs.
- This repo pairs that config with [`.codex/AGENTS.md`](../.codex/AGENTS.md)
  so child-agent instruction discovery matches the repo-root rules.

## Practical interpretation

The verified host already exposes a strong Codex capability surface. The main
operational rule for this repo is narrower:

- use specialist roles consistently
- use `diagnose` for agent self-enablement and `doctor` for operator-facing
  health review
- ship browser capability in the Rust runtime and repo-owned helpers first
- treat external browser MCP servers as optional aids, not product crutches
- keep the repo role pack synchronized with the live Codex surface instead of
  leaving stale assumptions in docs
