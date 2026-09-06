# 0003. Path B pack layout and `--from` manifest semantics

- Status: accepted
- Date: 2026-09-06
- Supersedes: nothing (resolves DESIGN §24's packs open question; §11's MVP blessing is narrowed, not reversed)

## Context and problem

DESIGN §23 step 11 (Path B run-local agents + `--from manifest.toml`
multi-worker packs) leaves several semantics to decide before
implementation: whether `--from` runs mount per-worker packs or a union,
what a supplied manifest's `run_id`/`created_at`/`harness_argv` mean,
flag-vs-manifest precedence, which worker is the parent, and how the
budget gate (§10) applies once workers have different packs. §24 records
the packs question as open ("multi-pack workdir vs union + argv subset;
prefer packs as soon as Path B lands") and §11 blessed the union for the
MVP. `start --from` currently refuses ("manifest-driven multi-worker
runs arrive with Path B") and `Adapter::write_run_agents` bails in all
three adapters, so none of this is load-bearing yet.

## Decision drivers

- A child must not discover a sibling's Skills by walking `..` (§11:
  per-pack subdirs as soon as two workers need different menus).
- The CLI `--skill` form is pinned by README output, integration tests,
  and Path A argv — zero churn there is worth a layout difference.
- DESIGN §14/§20.2: never write standing agent dirs; run-local files die
  with the run.
- Manifest is *intent* (§9); the lock is truth. A reused manifest is a
  template, not a record.
- Fail closed: invalid manifests leave no run dir (§20.3).

## Considered options

1. **`--from` always packs (even single-worker manifests); CLI `--skill`
   form keeps the flat union.** Two layouts, selected by entry path.
2. **Packs everywhere, including the CLI form.** One layout, but
   rewrites every README snippet, Path A argv, and integration test for
   no behavioral gain at one worker.
3. **Union + per-worker argv subsets** (§11's MVP sentence). No
   directory separation: a child walking its workdir root sees sibling
   Skills; Path B agent files would point into a shared root with no
   way to express "only your pack".

## Decision outcome

Option 1, with these rules:

- `start --from <manifest.toml>` mounts `workdir/packs/<worker>/` per
  worker (single-worker manifests included); `start --skill …` keeps the
  flat `workdir/<skill>` union. `workdir` (the root) stays the
  manifest/lock/audit workdir in both cases.
- A supplied manifest is intent only: `run_id`, `created_at`, and
  `harness_argv` are accepted and ignored — regenerated per run. Only
  `task`, `adapter`, `[budget] max_menu_tokens`, and `workers` are read.
- Precedence: `--adapter`/`--task` flags win over the manifest fields;
  manifest `[budget] max_menu_tokens` wins over config. `--skill`
  combined with `--from` is an error (`--skill and --from are mutually
  exclusive; put pins in the manifest workers`).
- **`workers[0]` is the parent.** Its pack is the Path A scan root
  (`workdir/packs/<name>`); `workers[1..]` become run-local agent
  definitions under `runs/<id>/agents/<worker>.md`.
- The budget gate (§10) is enforced **per worker pack**: each worker's
  sum of `description_tokens` against `max_menu_tokens`, the offending
  worker named in the error (`worker '<name>': menu_tokens …`). The
  over-budget message changes for the CLI form too (the single `default`
  worker) — intentional uniformity.
- pi ships DESIGN §14's print-and-snippet fallback: pi-subagents
  (probed 2026-09-06 on 0.84.4) has no per-invocation arbitrary agent
  directory override — no env var, flag, or settings key; discovery is
  builtin/package/user/project scopes only. Writing `~/.pi/agent/agents`
  or a project `.pi/agents` would violate §20.2 and is not an option.
- omp's Path B mode (load via a per-invocation overlay key vs the same
  print fallback) is set by an explicit probe of the installed omp,
  pinned in `adapters --explain`. No auto-fallback at runtime: the
  adapter states one belief honestly.

## Consequences

- The lock gains real per-skill `workers` arrays (names of every worker
  whose pack contains the skill, manifest order); the CLI form yields
  `["default"]`, byte-identical to what a single-worker run wrote
  before.
- `enforce_worker_budget` moves out of `resolve::resolve` so it can run
  after worker mapping but before any run dir is created (fail closed,
  no cleanup needed on budget failure).
- `why` prints one pack line per worker instead of the single default
  line for `--from` runs.
- If a future pi-subagents adds a per-invocation agent-dir flag, a later
  ADR can flip pi to Loaded mode; the generated files' shape is already
  pi-subagents-shaped.
- Option 3's argv-subset idea stays rejected: it cannot express Path B
  isolation at all.

## Confirmation evidence

- `docs/design-docs/DESIGN.md` §9/§11/§16/§18/§19/§24 amended to match
  (§24's packs bullet struck with a resolved pointer here).
- Feature-011/012 verification in PROGRESS.md: packs layout asserted,
  `--skill`+`--from` refusal, per-worker budget error naming the worker,
  byte-identical standing agent trees before/after a Path B run.
