# 0007. Menu TUI surface: `lunchbox menu`, manifest discovery, `tui <screen>` removal

- Status: accepted (planned 2026-09-08; implemented by features 016–018)
- Date: 2026-09-08
- Supersedes: nothing (extends ADR-0002's surface; ADR-0003's `--from` semantics are untouched — the menu emits the same schema-1 manifests)

## Context and problem

The CLI is deliberately script-first: a webhook can call
`lunchbox start --skill … --adapter … --wait -- …` and nothing may ever
prompt. But the human surface fell short. Bare `lunchbox` prints usage
and exits 2. The four v1 TUI screens are single-purpose invocations
(`tui doctor|preview|picker|policy`) that do not navigate to each
other, show no pantries-and-manifests overview, and cannot edit a
manifest. And `lunchbox start` with zero pins silently mounts an empty
skill set and spawns the default adapter (reproduced 2026-09-08: run
created, `menu_tokens this run: 0`, `pi` child spawned) — the
"it started a job I didn't specify" report that motivated this ADR.

## Decision drivers

- Interactive and scripted surfaces must not leak into each other; CI
  callers must stay byte-compatible.
- AGENTS.md clean-cutover rule: no parallel or deprecated entry points.
- Manifest files are user-authored TOML: edits must round-trip
  format-preservingly — `toml_edit` is already an optional dependency
  behind `tui-menu` (ADR-0002).
- One interactive entry point to document, test, and keep feature-gated.

## Considered options

1. Keep `tui <screen>` subcommands; add more screens beside them.
2. Add `lunchbox menu` as a single app; keep `tui <screen>` working as
   aliases.
3. Add `lunchbox menu` as the single interactive app; remove the
   `tui <screen>` subcommands in the same release; bare `lunchbox`
   stays help-only.

## Decision outcome

Option 3 (user-selected 2026-09-08). Concretely:

- **`lunchbox menu`** is the interactive surface: a single full-screen
  app behind the existing `tui-doctor` / `tui-menu` cargo features
  (CLI-only builds get the rebuild hint, as today). Screens: **Pantry**
  (skill roots — managed pantries, `library_paths`, `.agents/skills`
  defaults — with counts, duplicates, and each root's skills; selecting
  skills and confirming starts a run), **Manifests** (discovered
  manifests with task/worker/token summary; per-worker pack view),
  **Editor** (create/edit manifests: workers, packs, task, adapter,
  budget; live fail-closed validation; save), **Doctor**, and **Policy**
  (migrated from `tui policy`). The `tui preview` role is covered by
  the start-confirmation footer and `start --dry-run`.
- **Bare `lunchbox` stays help text.** No TTY detection anywhere: every
  subcommand except `menu` is non-interactive, always.
- **Manifest discovery**: `./lunchbox/manifests/*.toml` then
  `~/.lunchbox/manifests/*.toml`. Same-named manifests: project wins
  (matching the config layering philosophy); `--from <path>` remains
  the explicit escape hatch. `start --from <name>` resolves a bare
  name against these dirs (feature-018).
- **Confirmation gate invariant**: nothing spawns from the menu without
  a screen showing the adapter, the exact pinned skills with hashes,
  and the token cost, requiring an explicit keypress.
- **Prerequisite (feature-016)**: `start` with zero `--skill` pins and
  no `--from` fails closed — non-zero exit, a message naming the fix,
  no run directory, no spawn.
- **Cutover (feature-017)**: the `tui doctor|preview|picker|policy`
  subcommands are deleted when `menu` lands; callers migrate to `menu`
  screens or the existing top-level `doctor --json` / `start
  --dry-run` / `--json`. Snapshot coverage continues in-process
  (ratatui `TestBackend`, per ADR-0002 — no CLI `--json` twins of
  screens are needed).

## Consequences

- Breaking CLI change, released as 0.3.0 to signal it: scripts using
  `tui <screen>` must move to `menu` or top-level flags.
- No new dependencies: ratatui, crossterm, color-eyre, toml_edit
  (already behind `tui-menu` for policy edits — the manifest editor
  reuses it), insta for snapshots.
- The menu adds no new data paths: it renders the same resolver,
  pantry, and manifest model the CLI uses.
- `menu`'s start action funnels through the same run-preparation code
  as `cmd_start`, so CI and interactive runs cannot diverge.

- Addendum (2026-09-08, feature-017 implementation): the menu module is
  compiled only when both `tui-doctor` and `tui-menu` are on (it wraps
  both screens); partial-feature builds get the same rebuild hint as
  CLI-only builds. Dev-only `portable-pty` + `vt100` drive pty E2E
  tests of the real binary; runtime dependencies are unchanged.
- Addendum (2026-09-08, feature-018 implementation): the menu gains the
  manifest editor (workers/packs/task/adapter/budget with live
  fail-closed validation). Saves are format-preserving outside the
  rebuilt `[[workers]]` array (comments inside it are not preserved);
  `start --from <name>` resolves a bare name against the discovery
  dirs, with `--from <path>` unchanged.
