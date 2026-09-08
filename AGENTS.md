# AGENTS.md — lunchbox

Routing file for agents working in this repository. Topic details live in the
linked documents; this file stays short on purpose.

## Project overview

`lunchbox` is a run-scoped skill runtime (Rust CLI + feature-gated TUI): it
consumes a Skill pantry and produces a sealed, per-run mount so a worker
sees only the locked Skills for that job. `docs/design-docs/DESIGN.md` is
the source of intent; product narrative will live under
`docs/product-specs/` as it emerges.

- Current phase: v1 + Path B + scan hook + pantry milestone complete
  (features 001–014 all passing), distributed as `v0.1.0` — MIT, git-only
  install per ADR-0005 (https://github.com/nathanpt/lunchbox). Core MVP,
  Pi + Omp Path A adapters, the four TUI screens, the README (DESIGN
  §22/§25), `--from` multi-worker runs with run-local agent files (DESIGN
  §14 print mode, ADR-0003), the `scan_command` policy gate with
  `--override-scan` (ADR-0004), and thin git acquisition — `add` /
  `update` into `~/.lunchbox/pantry` (ADR-0006). Next:
  `skills/lunchbox/` driver Skill (DESIGN §22 "Next").
- Language/runtime: Rust, unconditional —
  `docs/decisions/0001-rust-unconditional.md` (ADR-0001).

## Commands

| Task | Command | Notes |
|---|---|---|
| Setup | `cargo build` | Rust ≥ 1.85 (edition 2024) |
| Run | `cargo run -- <subcommand>` | subcommands per DESIGN §16 |
| Checks | `cargo test` | must pass without Pi/Omp installed (DESIGN §21) |

## Global hard constraints

1. One feature at a time: select a single highest-priority incomplete feature
   from `docs/feature-list.json` and finish it before starting another.
2. Never delete or weaken a requirement in `docs/feature-list.json` to make
   work appear complete. Update status/evidence fields only.
3. New features start failing or unverified, and pass only after their listed
   checks succeed.
4. Clean cutover: migrate every caller; no shims, aliases, or deprecated
   paths left behind.
5. Never hand-edit anything under `docs/generated/`; each generated document
   names its generator.
6. No code comments unless the project explicitly adopts them; prefer clear
   names, tests, and documentation.

## Routing

- Read `ARCHITECTURE.md` before changing system boundaries or major modules.
- Read `docs/feature-list.json` before selecting the next feature.
- Read `docs/design-docs/DESIGN.md` and the rest of `docs/design-docs/`
  before proposing implementation structure.
- Read `docs/decisions/` before changing a decision an ADR constrains.
- Read `docs/exec-plans/active/` when continuing planned work; move finished
  plans to `docs/exec-plans/completed/`.
- Read `docs/product-specs/index.md` when touching user-visible behavior or
  requirements.
- Read `docs/exec-plans/tech-debt-tracker.md` before "while I'm at it"
  cleanup; record debt there instead of silently expanding scope.

## Decisions (ADR trigger)

For decisions that affect architecture, interfaces, data storage, security,
deployment, dependencies, or major user-visible behavior, create or update an
ADR in `docs/decisions/`. Read relevant existing ADRs before changing a
decision they constrain. Supersede old ADRs with new ones; never rewrite
history.

## Completion and reporting

- Run the verification level proportionate to the changed boundary (docs-only
  → link/format checks; isolated logic → focused tests; integration → real
  cross-module exercise; CLI surface → run the actual binary) before marking
  work complete.
- Record the exact command and observed result in `PROGRESS.md`.
- Skipped or unavailable verification is **unverified**, not passing.
- End every session by updating `PROGRESS.md` (state, verification, next
  move) and `CHANGELOG.md` for release-significant changes.
- Leave a clean handoff state: no placeholders, no TODO stubs, no unfinished
  scaffolding in delivered work.
