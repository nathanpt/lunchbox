# Lunchbox menu TUI milestone — features 016, 017, 018

Spec source: ADR-0007 (`docs/decisions/0007-menu-tui-surface.md`); user
decisions 2026-09-08 — bare `lunchbox` stays help-only; manifest
discovery is project + home dirs; the TUI is a full manifest editor;
`tui <screen>` is replaced and removed (released as 0.3.0). Acceptance:
`docs/feature-list.json` feature-016/017/018 steps. Repo state at
start: features 001–015 all pass, `main` at `65b309e`, 115 unit + 36
integration tests green default / 100 + 36 CLI-only, zero warnings.

## Context

The CLI stays script-first for CI. Humans get one interactive surface:
`lunchbox menu`, a single full-screen app (Pantry, Manifests, Editor,
Doctor, Policy) over the existing core modules — resolver, pantry
state, manifest model, run preparation. No new data paths, no new
dependencies (`toml_edit` already sits behind `tui-menu`). A
prerequisite bug fix makes `start` fail closed on zero pins so a
fumbled webhook cannot spawn an empty agent.

## Approach

Execute steps in order; one feature at a time (AGENTS.md). Flip each
feature's `passes` only after its verification loop passes.

### Step 1 — feature-016: `start` fails closed with no pins

1. `src/main.rs` / start path: before preparing a run, require at
   least one `--skill` pin or `--from`; otherwise error
   `no skills pinned: pass --skill <name>[@sha256:…], --from <manifest>, or use lunchbox menu`
   — non-zero exit, no run directory created, nothing spawned. Apply
   in plain, `--json`, and `--dry-run` forms.
2. Tests: failing arms for all three forms (assert no run dir, exit
   non-zero, message); regression arms confirming one-pin, multi-pin,
   and `--from` runs unchanged (output shape, spawn behavior).
3. Update README/docs only where they imply pinless start works
   (nowhere expected — the docs always show pins).

### Step 2 — feature-017: `lunchbox menu` app + cutover (breaking)

1. `src/tui/app.rs` (new): mode-stack app state
   (`Pantry | Manifests | Doctor | Policy | ConfirmStart`), shared
   `terminal.rs` loop; `Esc` pops a screen, `q`/`Ctrl-C` quits;
   bottom status bar shows live/last run state and `finish` action.
2. `src/tui/pantry.rs` (new): sections for managed pantries
   (`PantryState` healthy/broken), `library_paths`, default
   `.agents/skills` roots; counts, duplicate names; right pane lists
   the focused root's `ListedSkill`s (name, description, tokens);
   space toggles, `Enter` opens `ConfirmStart` (adapter, exact
   skills+hashes, token cost) — explicit keypress spawns via the same
   code path as `cmd_start`; absorb `picker.rs`'s start/finish flow
   and delete it.
3. `src/tui/manifests.rs` (new): list manifests discovered in
   `./lunchbox/manifests/*.toml` then `~/.lunchbox/manifests/*.toml`
   (project wins on name collision); rows show task, worker count,
   token estimate; `Enter` shows each worker's pack (the exact data
   `start --from` mounts). Read-only in this step.
4. Migrate `doctor.rs` and `policy.rs` content into menu screens
   unchanged in behavior; delete `preview.rs` (its role is the
   confirmation footer + `start --dry-run`).
5. `src/main.rs`: replace the `Tui`/`TuiScreen` clap subcommands with
   `Menu { #[arg(long, value_name = "PATH")] library: Vec<String> }`
   (feature-gated body; CLI-only builds error with the rebuild hint).
   Remove the `tui` subcommand entirely — clean cutover.
6. Tests: keep/adapt existing doctor+policy render tests as menu
   screen tests; new in-process snapshot tests (TestBackend + insta)
   for pantry/manifests screens and the confirmation screen; update
   CLI integration tests that reference `tui` subcommands.
7. CHANGELOG `[Unreleased]` → `### Changed` — breaking: `tui <screen>`
   removed in favor of `lunchbox menu` (0.3.0).

### Step 3 — feature-018: manifest editor + `--from` name resolution

1. `src/tui/editor.rs` (new): open an existing discovered manifest or
   create one; left pane workers (add/remove/rename, task,
   adapter, `[budget] max_menu_tokens`), right pane the resolved skill
   union grouped by source; toggles bind skills into the focused
   worker's pack; live validation mirroring the `--from` parser rules
   (schema, duplicate worker names, empty packs) shown as blocking
   errors — save is refused while any error is live.
2. Save via `toml_edit` (format-preserving for existing files;
   schema-1 layout for new ones) into `./lunchbox/manifests/` or
   `~/.lunchbox/manifests/` (choice offered; directory created).
   Optional pin-as-hash toggle writes `name@sha256:<64 hex>`.
3. `start --from <name>`: resolve a bare name against the discovery
   dirs (project first, then home); ambiguous/missing names fail
   closed listing candidates; `--from <path>` unchanged.
4. Tests: editor validation arms; TOML round-trip (comments/keys
   outside edited arrays preserved); `--from` name-resolution arms
   (hit, project-wins, missing lists candidates).

## Critical files & anchors

- `src/main.rs` — `CliCommand::Tui`/`TuiScreen` (lines ~74–105)
  replaced by `Menu` in step 2; `Start` guard in step 1.
- `src/tui/` — `picker.rs`/`preview.rs` deleted; `app.rs`,
  `pantry.rs`, `manifests.rs`, `editor.rs` new; `doctor.rs`,
  `policy.rs`, `terminal.rs`, `snap.rs` retained/adapted.
- `src/run/` — manifest model reused by the editor; no schema change.
- `docs/feature-list.json` — features 016–018 (added with this plan).

## Verification

Working dir: repo root; binary `target/debug/lunchbox`; fresh `HOME`
per block; `$T = mktemp -d`.

1. Every step: `cargo test` and `cargo test --no-default-features`
   green, zero warnings.
2. feature-016: `HOME=$T lunchbox start` → non-zero, message names
   `--skill`/`--from`/`menu`, no `~/.lunchbox/runs` entries; `--json`
   and `--dry-run` arms same; one-pin run output identical to
   pre-change (spot-check `skills`/`menu_tokens` lines).
3. feature-017: `HOME=$T lunchbox menu` in a real pty — drive
   Pantry → select skills → ConfirmStart → spawn (`--adapter none`
   against `testdata/skills`) → status bar → finish; `lunchbox tui
   doctor` → "unrecognized subcommand" and help no longer lists `tui`;
   CLI-only build (`--no-default-features`) `menu` → rebuild hint;
   manifests screen lists a fixture in `./lunchbox/manifests/`.
4. feature-018: in the pty, create a two-worker manifest in the
   editor, save to `$T/.lunchbox/manifests/`, exit; `HOME=$T lunchbox
   start --from <name> --adapter none` mounts it; validation error
   blocks save on an empty-pack worker; round-trip a commented TOML
   file byte-compare outside edited arrays.
5. Record the pty walkthrough (as with the v1 exit bar) in
   PROGRESS.md; flip features; CHANGELOG; release as 0.3.0 when the
   milestone lands (separate packaging step, ADR-0005 flow).

## Assumptions & contingencies

- pty automation: drive `lunchbox menu` with a scripted terminal
  (hub pty / tmux); if brittle, fall back to the recorded manual
  walkthrough the v1 exit bar used — in-process snapshot tests carry
  the regression load either way.
- Manifest name collisions: project-first (ADR-0007); if users report
  surprise, flip to fail-closed listing candidates (contract change,
  new ADR revision).
- No new crates; if the editor needs richer table editing than
  ratatui's plain widgets, stay with hand-rolled lists (ADR-0002
  already rejected template scaffolding at this size).
- If `toml_edit` round-trip flakes on exotic manifests (mixed
  comments/inline tables), save-new-files-normally and
  preserve-existing-files is the floor, not best-effort rewriting.
