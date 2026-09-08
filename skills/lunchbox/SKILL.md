---
name: lunchbox
description: Drive the lunchbox CLI - mount a sealed Skill set for one job, spawn the worker, always unmount.
---

# Lunchbox driver

Use lunchbox when a bounded job (review, scan, debug, handoff) needs
specific Skill packages and the worker's standing menu would leak
everything else. Lunchbox mounts exactly the pinned packages into a
sealed per-run workdir, hands it to a worker whose skill discovery is
off, and removes the workdir afterwards - on success, failure, cancel,
or crash. You need the `lunchbox` binary on PATH.

## The loop

1. See the standing cost: `lunchbox doctor` (read-only; dirs, counts,
   `menu_tokens`, duplicates).
2. Mount: `lunchbox start --skill <name> [--skill <name>...]
   --adapter <none|pi|omp>`.
3. Run the worker. With adapter pi or omp, pass the harness argv after
   `--` (plus `--wait` to finish on exit); the child sees only the mount.
   With adapter none, lunchbox prints the mount and exits.
4. Always unmount: `lunchbox finish`. Idempotent; `lunchbox abort`
   cancels a live run.

## Commands

- `lunchbox start` - pins to a sealed mount. Repeat `--skill` per
  package; `--skill name@sha256:<64 hex>` pins content for
  reproducibility. `--library <dir>` adds a pantry root for this run;
  managed pantries (`lunchbox add`) are searched without it.
  `--from manifest.toml` mounts per-worker packs instead of a flat
  union. `--dry-run` prints the spawn argv; `--json` prints one
  parseable line.
- `lunchbox status` / `lunchbox why` - current or last run: state,
  packs, token delta, unmounted yes/no.
- `lunchbox finish` / `abort` / `gc` - teardown; `gc` also collects
  runs older than 24 h and leaked mounts.
- `lunchbox add <git-url> [--path <subdir>]` / `lunchbox update [name]`
  - clone or fast-forward whole skill repositories into
  `~/.lunchbox/pantry/`. Whole repos only; git is the version system.

## Errors fail closed

`skill '<name>' not found in any library root`, a hash mismatch,
`denied by policy`, or a failed `scan_command` aborts the start and
leaves no run directory. Fix the pin, the config layer, or the scanner;
retry. Never bypass a scan failure except with an explicit, audited
`--override-scan` - and tell the user you did.

## Discipline

- One run, one job. Never reuse a workdir across jobs.
- Call `finish` even when the worker failed.
- Never write into pantry directories; lunchbox only reads them.
- Prefer hash pins when a run must be reproducible.
