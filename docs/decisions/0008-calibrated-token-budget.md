# ADR-0008 — Calibrated Stage A token budget

Date: 2026-09-08
Status: Accepted (supersedes the estimator formula in DESIGN §12)

## Context

`menu_tokens` is a guardrail: it estimates what a menu costs the worker's
baseline context at Stage A (the harness's skill listing; DESIGN §12).
The v1 estimator was `ceil(chars/4)` over `name + "\n" + description`,
documented as approximate. Reviewing it against the real payload showed
two problems: it ignored everything the harness renders around those two
fields, and "approximate" hid a ~3× undercount for small skills.

## Probe (2026-09-08, pi 0.84.4 installed at
~/.local/lib/node_modules/@earendil-works/pi-coding-agent)

Pi's skill discovery renders, per skill:

```
  <skill>
    <name>${skill.name}</name>
    <description>${skill.description}</description>
    <location>${skill.filePath}</location>
  </skill>
```

inside one `<available_skills>` block with a fixed four-line preamble.
So the per-skill payload is **name + description + the full mount path +
XML scaffolding (~99 chars)** — other frontmatter keys are never read
(pi maps skills to `{name, description}` only), while the path and tags
are always paid. Omp Path B differs again: its agent files name the
worker's pack directory and carry no skill text, so skill descriptions
never reach the prompt at all.

## Decision

1. Per-skill estimate = `ceil((chars(name) + 1 + chars(description) +
   204) / 4)` — the 204 chars model the XML scaffolding (99) plus a
   mount-path allowance (105, dominated by `<runs_dir>/<run_id>/workdir/…`
   prefixes). Calibrated to pi Path A.
2. Every menu total adds a one-time preamble of 74 tokens
   (`tokens::with_preamble`, zero for an empty menu).
3. No tokenizer dependency. Accurate BPE needs the target model's
   vocabulary (megabytes, heavy dep tree), and the worker model varies
   per adapter/harness (Claude, GLM, GPT tokenize differently by
   10–20%) — a vocabulary would add bloat to be precisely wrong for most
   workers. The heuristic plus the soft-budget semantics
   (`max_menu_tokens` warns unless `fail_on_budget = true`) is the right
   guardrail; the output stays `~N`.

## Consequences

- Numbers rise accordingly: the two demo skills estimate 65 and 64
  (was 14 and 13); their union reports 203 with the preamble (was 27).
- Budgets get stricter, matching reality for pi Path A. Omp Path B runs
  see an upper bound (their prompt carries no skill text); acceptable
  for a guardrail, and the per-adapter divergence is now documented
  instead of hidden.
- Field names (`description_tokens`, `menu_tokens`) keep their JSON
  contract; semantics are defined by this ADR.
- If pi's rendering changes, re-probe and re-calibrate the two
  constants (`SKILL_LISTING_OVERHEAD_CHARS`, `MENU_PREAMBLE_TOKENS`).
