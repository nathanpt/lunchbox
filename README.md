# lunchbox

**Your agent only sees these Skills for this job. Then they vanish.**

Lunchbox is a run-scoped skill runtime. It mounts a sealed per-run workdir
containing exactly the Skill packages you pinned, hands it to a worker
(Pi or Omp), and always unmounts — on success, failure, cancel, or crash.

## Install

From the repository (no account, no server, no background indexer — one
static binary):

```sh
cargo install --locked --git <repository-url>
```

`<repository-url>` is a placeholder — replace it with the real repository
URL once published (none is configured yet). From a clone of this
repository instead:

```sh
cargo install --locked --path .
```

## Quick start

From a clone, prefix the commands below with `cargo run --`.

See what your agent's skill menu looks like today — every skill dir the
adapter scans, how many Skills live there, the estimated description-token
cost (`menu_tokens`), and duplicate names across dirs. Read-only; nothing
is mounted. This machine has an empty pantry (snippet paths are shortened:
`~` is your home directory, `<project>` the clone; run ids and hashes
truncated):

```sh
lunchbox doctor
```

```
adapter        pi 0.84.4
skill dirs:
  <project>/.agents/skills   absent
  ~/.agents/skills           absent
menu_tokens    0  (0 skills union)
               no skills found in adapter skill dirs
fattest:
duplicates     none
```

Then run a job with exactly two Skills from the sample pantry:

```sh
lunchbox start \
  --library testdata/skills \
  --skill demo-review \
  --skill demo-scan \
  --adapter none
```

```
run            lbx_…
workdir        ~/.lunchbox/runs/lbx_…/workdir
skills         demo-review@sha256:80d0644a…  demo-scan@sha256:0fc41caa…
menu_tokens    this run: 27
without        0  (no skills found in none skill dirs)
isolation      adapter none — mounted, not spawned
unmount        run lunchbox finish lbx_…   (auto on --wait exit)
```

The workdir contains only `demo-review/` and `demo-scan/`. Take it back:

```sh
lunchbox finish
```

```
finished ~/.lunchbox/runs/lbx_…  (workdir removed, result.json written)
```

The workdir is deleted and `result.json` records `unmounted: true` with the
exact hashes that were mounted.

To actually isolate a run (Path A), lunchbox spawns the harness with skill
discovery off and only the mount visible — for Pi via
`--no-skills --skill <workdir>`, for Omp via a per-run `--config` overlay
pointing `skills.customDirectories` at the workdir:

```sh
lunchbox start --library testdata/skills --skill demo-review \
  --adapter pi --wait -- -- pi -p "review the staged diff"
```

On exit the workdir is unmounted automatically. `lunchbox abort` cancels a
live run; `lunchbox gc` collects runs older than 24 h plus leaked mounts;
`lunchbox why` prints a five-line recap of the last run.

## menu_tokens, before and after

`doctor` reports the cost of the menu your agent would eat without
Lunchbox — its whole pantry. `start` reports what this run actually gets —
only the pinned pack. In the quick start above the pantry was empty
(`menu_tokens 0`) and the run cost `this run: 27`; on a loaded machine the
gap is the point. The estimator is `ceil(chars/4)` over each Skill's name +
description — approximate, but it makes the cost of a fat global menu
visible before you mount anything.

## This is not a skill manager

This is not a skill manager. Point `--library` at Kitter / Skills Manager /
`~/.agents/skills`. Lunchbox does not store, install, version, or sync
Skills: it reads from the pantry you already keep, mounts a per-run subset,
and never writes into your standing skill or agent directories.

## TUI

Lunchbox also ships four terminal screens (default cargo features; the
CLI-only build fails closed on `tui`):

- `lunchbox tui doctor` — the doctor report, navigable.
- `lunchbox tui picker --library <pantry>` — compose a pack, start the run,
  finish it, all from the screen.
- `lunchbox tui preview` — `menu_tokens` for a candidate pack vs
  without-Lunchbox.
- `lunchbox tui policy` — view and edit `allow` / `deny` per config layer.

## gif-script

A text-only stand-in for an animated demo — the quick start, abridged:

```text
$ lunchbox doctor
adapter        pi 0.84.4
menu_tokens    0  (0 skills union)
duplicates     none

$ lunchbox start --library testdata/skills --skill demo-review --skill demo-scan --adapter none
run            lbx_…
workdir        ~/.lunchbox/runs/lbx_…/workdir
skills         demo-review@sha256:80d0644a…  demo-scan@sha256:0fc41caa…
menu_tokens    this run: 27
isolation      adapter none — mounted, not spawned

$ lunchbox finish
finished ~/.lunchbox/runs/lbx_…  (workdir removed, result.json written)
```

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
