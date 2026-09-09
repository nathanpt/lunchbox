# ADR-0009 — Run-scoped tool selection and `tool_tokens`

Date: 2026-09-08
Status: Accepted

## Context

Tools cost context the same way skill menus do: every enabled tool ships
its definition (name + description + parameter schema) into the prompt,
plus latency. Skill menus got run-scoped control and an honest estimate
(ADR-0008); tool loading was untouched — a spawned worker always got the
harness's full default tool set, and Path B agent files hardcoded a tool
list (`read, grep, find, bash` for pi, `read/grep/glob/bash` YAML for
omp) regardless of what the run needed.

Both harnesses expose native allowlists, so lunchbox does not need a
mechanism of its own — only plumbing and an estimate:

- pi 0.84.4 `--help`: `--tools, -t <tools>` ("Comma-separated allowlist
  of tool names to enable", applies to built-in, extension, and custom
  tools), plus `--exclude-tools`, `--no-tools`, `--no-builtin-tools`.
  Default-enabled builtins probed 2026-09-08 via the extension API
  (`before_agent_start` → `systemPromptOptions.selectedTools`): read,
  bash, edit, write. Registered builtins also include grep, find, ls,
  powershell.
- omp 18.1.14 `--help`: `--tools=<value>` ("Comma-separated list of
  tools to enable (default: all)") and `--no-tools`. Wire capture
  (`before_provider_request` hook, default run): read, bash, edit,
  eval, glob, grep, task, hub, todo, web_search, write reach the
  provider; ast_edit, debug, lsp surface when named in `--tools`;
  goal, init_experiment, run_experiment, log_experiment,
  update_notes are config-gated and did not surface.
- Both agent-file formats accept a per-agent `tools` field (pi
  frontmatter `tools: <csv>`; omp task-agent YAML `tools:` bullet
  list) — DESIGN §14.

## Decision

1. **Selection surface = CLI `--tool <name>` (repeatable) on `start`
   and optional `tools = [...]` per worker in manifests.** The CLI
   form feeds the single `--skill`-branch worker; the manifest form
   feeds each worker. `--tool` and `--from` are mutually exclusive
   (set tools per worker in the manifest). Tool names are pass-through:
   the harnesses are the validators; the only lunchbox-side check is
   the empty-string rejection in manifest validation, before any run
   dir exists.
2. **Posture = opt-in.** No selection means no flags, no tools line in
   agent files, no `tools`/`tool_tokens` output, no JSON keys, no audit
   event — the harness default (all tools) applies, unchanged from
   previous behavior.
3. **Path A flags:** `adapter.isolation_argv` gained a `tools`
   parameter. pi appends `--tools <csv>` after the `--skill` pushes;
   omp appends `--tools=<csv>` after the `--config` overlay. Summaries
   stay static; the run output block carries the tool facts.
4. **Path B agent files:** the worker's `tools` drives the emitted
   `tools:` frontmatter (pi CSV line; omp YAML bullets). Omitted
   entirely when unset — replacing the hardcoded lists, which misstated
   isolation (claiming a restriction the manifest never chose).
5. **Estimate = separate `tool_tokens` line everywhere menu totals
   show**, probe-pinned per tool like ADR-0008: provider payload per
   tool (`{name, description, parameters}` as JSON, `ceil(chars/4)`),
   tables in `src/adapter/tools.rs` with the probe date + versions.
   `max_menu_tokens` keeps gating skills only — a combined budget was
   rejected (user-confirmed this session): tool selection is a
   capability choice, menu size is a context-cost guardrail, and
   coupling them would make a valid small-tool run fail a budget
   calibrated for skills. Human output shows `tools` (selection +
   count) and `tool_tokens    this run: N` with a `(+k unestimated)`
   suffix when k > 0; stderr warns once per name missing from the
   adapter's table (`warning: no token estimate for tool 'x' on
   <adapter> (not a builtin?)` — pi/omp only, never adapter `none`);
   `--json` adds `tools`/`tool_tokens`/`unestimated_tools`; the audit
   gains a `tools` event right after `mounted` with `selected`,
   `estimated_tokens`, `unestimated`. Estimates are per-run
   (workers[0]'s selection — the spawning worker), not per-worker:
   Path B workers' tools are print-mode advisory.
6. **No config policy layer.** Tool selection is a per-run argument
   like skills, not a standing policy — `scan_command` (ADR-0004)
   stays the only pre-mount policy gate.

## Consequences

- Token tables drift with harness updates the same way ADR-0008's
  constants do; unknown names surface as unestimated instead of being
  silently dropped, so a renamed builtin is visible, not hidden.
- omp's config-gated tools (goal, experiment family) and any
  extension-provided tools are unestimated by design; the table pins
  the probed default set only.
- The `--from`/`--tool` split mirrors `--skill`/`--from` (ADR-0003):
  CLI flags shape single-worker runs; manifests are the multi-worker
  surface.
