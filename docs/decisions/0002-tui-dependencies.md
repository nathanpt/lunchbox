# 0002. TUI dependencies: ratatui + crossterm + color-eyre + toml_edit, insta for snapshots

- Status: accepted
- Date: 2026-09-06
- Supersedes: nothing (ADR-0001 is untouched)

## Context and problem

DESIGN.md §21 pins the v1 TUI stack: Ratatui on crossterm, behind the
`tui-doctor` / `tui-menu` cargo features so `--no-default-features` still
ships a working CLI. The v1 TUI milestone (§22) adds four screens
(doctor, token-cost preview, skill picker, policy review), each with a
`--json` twin and headless snapshot tests. Two implementation needs fall
outside the pinned list: panic-time terminal restore for the TUI path
(a crashed raw-mode screen must not wreck the user's terminal) and
format-preserving edits of user-authored config layer files (the policy
screen appends `allow`/`deny` entries without destroying comments or key
order in `lunchbox.toml` / `~/.lunchbox/config.toml`).

## Decision drivers

- DESIGN §21 pins `ratatui 0.30` + `crossterm 0.29`; TUI is a first-class
  v1 surface.
- `cargo test` must pass with no TUI features compiled and without
  Pi/Omp installed (DESIGN §21, §21 tests).
- Core stays framework-free: TUI crates live only behind the features.
- User-authored TOML must round-trip through policy edits byte-for-byte
  outside the edited array.
- Version policy: "current stable at install time; `Cargo.lock` records
  exact" — same as Phase 1.

## Considered options

1. **Adopt `ratatui` + `crossterm` + `color-eyre` + `toml_edit`, dev-dep
   `insta`** (all optional behind the two features).
2. Same, plus the `config` crate for layer reading/writing.
3. Same as 1 but rewrite layer files wholesale via the existing `toml`
   serializer instead of `toml_edit`.

## Decision outcome

Option 1. Concretely:

- `ratatui` (optional) — rendering; snapshot tests use
  `ratatui::backend::TestBackend`, so CI needs no terminal.
- `crossterm` (optional) — raw mode, alternate screen, event reads.
- `color-eyre` (optional) — installed once per TUI process before the
  terminal enters raw mode; panic hooks restore a sane terminal state.
- `toml_edit` (optional, `tui-menu` only) — format-preserving appends of
  `allow`/`deny` entries to config layer files.
- `insta` (dev-dependency) — snapshot storage/review for buffer dumps.

Two deviations from DESIGN §21's pin list, both deliberate:

- **The `config` crate stays excluded.** Phase 1 already hand-rolled the
  two-layer merge algebra (allow intersection, deny union,
  project-over-global scalars) that `config`'s layering cannot express;
  adding it now would mean a second, divergent config representation.
  `toml_edit` covers the editing need without touching parsing.
- **The `cargo generate ratatui/templates` component template is not
  used.** Four small screens with a shared state/render/handle_event
  pattern; template scaffolding outweighs its value at this size.

Feature wiring (both features default-on per DESIGN §21 "TUI is a
first-class v1 surface"; CLI-only builds opt out with
`--no-default-features`):

```toml
[features]
default = ["tui-doctor", "tui-menu"]
tui-doctor = ["dep:ratatui", "dep:crossterm", "dep:color-eyre"]
tui-menu = ["dep:ratatui", "dep:crossterm", "dep:color-eyre", "dep:toml_edit"]
```

## Consequences

- `cargo test` and `cargo test --no-default-features` both stay green;
  the no-default build compiles no `src/tui` code and the `tui`
  subcommand fails closed with a rebuild hint.
- If ratatui 0.30 / crossterm 0.29 ever drift apart (third-party crates
  pinning crossterm 0.28), DESIGN §21 already anticipates ratatui's
  `crossterm_0_28` compat feature as the fallback.
- Snapshot files are committed under `src/tui/snapshots/`; trailing-space
  drift across environments is handled by insta filters if it ever
  flakes, not preemptively.
- If crate versions drift from the pins at install time, the newest
  co-installable pair wins and `Cargo.lock` records the exact choice.

## Confirmation evidence

- `Cargo.toml` gains the two features and the optional dependencies;
  `cargo test` (default) and `cargo test --no-default-features` both
  pass with no `src/tui` code in the no-default build.
- ADR-0001 remains accurate: language unchanged, only dependencies added.
