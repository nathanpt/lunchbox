# lunchbox

**Your agent only sees these Skills for this job. Then they vanish.**

Lunchbox is a run-scoped skill runtime. It mounts a sealed per-run workdir
containing exactly the Skill packages you pinned, hands it to a worker
(Pi today), and always unmounts — on success, failure, cancel, or crash.

## Install

From a clone of this repository:

```sh
cargo install --locked --path .
```

No account, no server, no background indexer. One static binary.

## Quickstart

```sh
lunchbox doctor
```

shows what your agent's skill menu looks like today — every skill dir the
adapter scans, how many Skills live there, the estimated description-token
cost (`menu_tokens`), and duplicate names across dirs. Read-only; nothing
is mounted.

Then run a job with exactly two Skills from the sample pantry:

```sh
lunchbox start \
  --library testdata/skills \
  --skill demo-review \
  --skill demo-scan \
  --adapter none
```

```
run            lbx_20260906_015001_85f2
workdir        ~/.lunchbox/runs/lbx_20260906_015001_85f2/workdir
skills         demo-review@sha256:80d0644a…  demo-scan@sha256:0fc41caa…
menu_tokens    this run: 27
without        0  (no skills found in pi skill dirs)
isolation      adapter none — mounted, not spawned
unmount        run lunchbox finish lbx_20260906_015001_85f2   (auto on --wait exit)
```

The workdir contains only `demo-review/` and `demo-scan/`. Take it back:

```sh
lunchbox finish
```

The workdir is deleted and `result.json` records `unmounted: true` with the
exact hashes that were mounted.

To actually isolate a Pi run (Path A): lunchbox spawns `pi` with skill
discovery off and only the mount passed via `--skill`:

```sh
lunchbox start --library testdata/skills --skill demo-review \
  --adapter pi --wait -- -- pi -p "review the staged diff"
```

On exit the workdir is unmounted automatically. `lunchbox abort` cancels a
live run; `lunchbox gc` collects runs older than 24 h plus leaked mounts;
`lunchbox why` prints a five-line recap of the last run.

## menu_tokens, before and after

`doctor` reports the cost of the menu your agent would see without
Lunchbox; `start` reports `this run` for the pinned set. The estimator is
`ceil(chars/4)` over each Skill's name + description — approximate, but it
makes the cost of a fat global menu visible before you mount anything.

## This is not a skill manager

Lunchbox does not store, install, version, or sync Skills. Point
`--library` at the pantry you already keep — Kitter, Skills Manager,
`~/.agents/skills` — and Lunchbox reads from it, mounts a per-run subset,
and never writes into your standing skill or agent directories.

## Repository map

| Path | Purpose |
|---|---|
| `docs/design-docs/DESIGN.md` | The design — source of intent |
| `AGENTS.md` | Entry point for agents working in this repository |
| `ARCHITECTURE.md` | System shape, boundaries, invariants |
| `PROGRESS.md` | Current state, verification status, next move |
| `CHANGELOG.md` | Release-significant changes |
| `docs/feature-list.json` | Machine-readable feature contract |
| `docs/decisions/` | ADRs for decisions that constrain future work |
| `testdata/skills/` | Sample pantry for the demo above |
