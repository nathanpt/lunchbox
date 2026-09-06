# Architecture — lunchbox

This file answers the standing architecture questions at the summary level;
`docs/design-docs/DESIGN.md` is the detailed design. Update each section as
decisions land in `docs/decisions/`.

## What is this system?

A run-scoped skill runtime named `lunchbox`: it resolves explicit Skill pins
from a pantry (library), mounts a sealed per-run workdir, hands it to a
worker (parent harness via Path A isolation flags, or run-local subagent
definitions via Path B), and always unmounts. Users and use cases:
DESIGN §4.

## Where does work start?

- Entry point: a CLI invocation — argv in, output/exit code out.
- Command surface: DESIGN §16, implemented in `src/main.rs` (clap derive):
  `doctor`, `start`, `status`, `finish`, `abort`, `gc`, `why`, `adapters`.
- `start` is the spine: config load → resolve pins → create run dir →
  manifest → mount → lock → audit → spawn (adapter-dependent) → teardown.
  Any failure after run-dir creation removes the run dir entirely
  (DESIGN §20.3).

## How do the major components relate?

```
argv → config (two-layer merge) → resolve (pins → Locked)
     → mount (symlink, copy fallback) → run (manifest/lock/audit/result)
     → adapter none: print and exit
     → adapter pi: isolation argv → spawn → wait → auto-finish
```

- `src/config/` — two-layer config (`~/.lunchbox/config.toml` +
  `./lunchbox.toml`): scalars project-wins, `deny` unions, `allow`
  intersects, `library_paths` project-prepended. Unknown key in either
  layer is a hard error.
- `src/library/` + `src/hash/` — Skill identity (frontmatter name, else
  directory name) and the canonical tree hash (golden-tested).
- `src/resolve/` — pin expansion and the policy gates (hash match, deny,
  allow, missing `SKILL.md`, scan hook refusal, token budget) in DESIGN
  §10 order. Fail closed.
- `src/mount/` — symlink per package into `workdir/<name>`; any symlink
  failure switches the whole run to uniform copy mode; copy mode rejects
  symlink escapes outside the package root.
- `src/run/` — run ids, manifest/lock/audit/result schemas, `flock`-guarded
  idempotent teardown, status/gc/latest-run discovery.
- `src/adapter/` — the DESIGN §19 trait; `none` (mount-only) and `pi`
  (Path A: `--no-skills` + one `--skill` per package). `omp` refuses
  until its phase.
- Pure decision logic (merge algebra, pin parsing, frontmatter parsing,
  token estimation, tree hashing) is unit-tested without side effects;
  lifecycle behavior is exercised through the real binary in `tests/cli.rs`
  with temp `HOME`s and a mocked `pi`.

## Boundaries and invariants

- Standing invariant: behave as a normal Unix CLI — exit 0 on success,
  non-zero on failure, usable in pipelines.
- External dependencies: clap, serde, toml, anyhow, serde_json, sha2,
  hex, fs4, time, signal-hook (runtime); tempfile, assert_cmd, predicates,
  parking_lot (dev). TUI crates arrive with the TUI milestone, behind a
  cargo feature, per DESIGN §21.
- Data storage: run state only, under `runs_dir` (default
  `~/.lunchbox/runs/<run_id>/`): `manifest.toml`, `lunchbox.lock`,
  `workdir/`, `audit.jsonl`, `result.json`, `pid` when a child is
  spawned. Formats and locations are fixed by DESIGN §7/§9/§18 (settled
  at the 2026-09-06 design review); schema changes need a new DESIGN
  revision, not silent drift.
- Lunchbox never writes into standing skill or agent directories
  (DESIGN §20.2).

## Where to look next

1. `docs/design-docs/DESIGN.md` — intent and detailed design.
2. `docs/decisions/0001-rust-unconditional.md` — the language decision.
3. `docs/feature-list.json` — the contracted features.
