# lunchbox

**Your agent only sees these Skills for this job. Then they vanish.**

Every Skill your agent can see costs you on every turn: its name and
description ride the system prompt of each call, so a fat global menu taxes
everything you ask — and every subagent inherits the same folder. Lunchbox
is a run-scoped skill runtime: it mounts a sealed per-run workdir
containing exactly the Skill packages you pinned, hands it to a worker
(Pi or Omp) that sees nothing else, and always unmounts — on success,
failure, cancel, or crash. If you drive Pi or Omp and keep Skills,
lunchbox is how a review helper gets `code-review` and `secrets-scan` for
one job instead of your whole pantry.

## Install

From the repository (no account, no server, no background indexer — one
static binary):

```sh
cargo install --locked --git https://github.com/nathanpt/lunchbox --tag v0.1.0
```

Or the moving tip instead of the tag: `--git
https://github.com/nathanpt/lunchbox`. From a clone of this repository
instead:

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
git            2.53.0
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

Real Skills of your own — see [Adding skills](#adding-skills) below.

## Adding skills

Lunchbox reads Skill packages: a directory holding a `SKILL.md`
frontmatter file (`name`, `description`) plus optional `scripts/`,
references, and assets. Point it at a git repository of those:

```sh
lunchbox add https://github.com/nathanpt/agent-skills
```

```
added          agent-skills
pantry root    ~/.lunchbox/pantry/agent-skills/skills
skills         6
```

(Output from 2026-09-08; paths shortened and the skill count follows the
repository.) The clone lands under `~/.lunchbox/pantry/` and is searched
from then on — the `start` commands in this README work with no
`--library`. Lunchbox auto-detects the pantry inside a repo: Skills at
the repository root, or in a single subdirectory such as `skills/`. If a
repo offers several candidates, name one: `lunchbox add <url> --path
skills`. Keep pantries current with `lunchbox update` (fast-forward
only); stop using one by deleting its directory — nothing is registered
anywhere else.

Skills you already keep work unchanged: `--library <dir>` per run,
`library_paths` in `~/.lunchbox/config.toml` or a checked-in
`./lunchbox.toml`, and the always-scanned `./.agents/skills` and
`~/.agents/skills`. Your configured paths win when the same name exists
in both a managed pantry and your own paths.

## menu_tokens, before and after

`doctor` reports the cost of the menu your agent would eat without
Lunchbox — its whole pantry. `start` reports what this run actually gets —
only the pinned pack. In the quick start above the pantry was empty
(`menu_tokens 0`) and the run cost `this run: 27`; on a loaded machine the
gap is the point. The estimator is `ceil(chars/4)` over each Skill's name +
description — approximate, but it makes the cost of a fat global menu
visible before you mount anything.

## This is not a skill manager

Lunchbox acquires whole Skill repositories (`add` / `update`) and mounts
per-run subsets of them; git stays the version system. It does not
install, version, edit, or remove individual Skills — for that, point
`--library` at Kitter / Skills Manager / `~/.agents/skills` and lunchbox
will read that pantry without ever writing into it.

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
