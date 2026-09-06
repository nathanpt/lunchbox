# Changelog

Notable, release-significant changes to lunchbox. Format follows
[Keep a Changelog](https://keepachangelog.com/); dates are ISO-8601. Internal
edits that a user would not notice do not belong here.

## [Unreleased]

### Added

- 2026-09-05 — Project foundation (Full tier): agent routing (`AGENTS.md`),
  architecture/progress/changelog documents, `docs/` knowledge store
  (design-docs, decisions, exec-plans, product-specs, generated, references),
  and an empty feature contract (`docs/feature-list.json`). Git repository
  initialized on `main`; nothing committed yet.
- 2026-09-06 — User-authored design added (`docs/design-docs/DESIGN.md`),
  pressure-tested in a design review, and amended: TUI promoted to a
  first-class v1 surface (four Ratatui screens), two-layer config, git-only
  install, v1 exit bar defined. ADR-0001 accepted: Rust, unconditional.
  Feature contract populated (8 features); foundation docs synced.

### Added (Phase 1)

- 2026-09-06 — Phase 1: core MVP + Pi Path A adapter. Two-layer config
  (`~/.lunchbox/config.toml` + `./lunchbox.toml`, project-wins/deny-union/
  allow-intersect/library-prepend), content-addressed Skill hashing,
  library reader, fail-closed resolver (hash pins, deny/allow gates,
  token budget), symlink/copy sealed workdir with always-unmount teardown
  (`finish`/`abort`/`gc`, flock-guarded, idempotent), lifecycle CLI
  (`start`/`status`/`finish`/`abort`/`gc`/`why`/`adapters`/`doctor`,
  `--json` twins), `menu_tokens` estimates, Pi adapter spawning
  `pi --no-skills --skill <workdir>/<skill>` with selftest-guarded flags,
  and the `testdata/skills` demo pantry. Features 001, 002, 007 pass.

### Added (TUI milestone)

- 2026-09-06 — TUI milestone: the four v1 screens (features 003–006) as
  Ratatui surfaces behind default-on cargo features (`tui-doctor`,
  `tui-menu`); `--no-default-features` still ships the CLI-only binary
  and the `tui` subcommand fails closed there. New `tui` subcommand:
  `tui doctor|preview|picker|policy`, each interactive with a
  `--json` twin (`tui doctor --json` byte-identical to `doctor --json`).
  Screens are state/render/handle_event modules snapshot-tested headless
  (`insta` + Ratatui `TestBackend`); the picker mounts/finishes real
  runs in-process (adapter `none`, quit never leaks a mount); policy
  review appends allow/deny entries to exactly the viewed layer with
  `toml_edit` (comments and key order preserved). Core additions:
  `DoctorReport` (single source for doctor CLI + TUI),
  `tokens::preview` (estimate-only pack preview, no gates),
  `library::scan_roots` (first-wins multi-root listing), and the
  `prepare_run` extraction from `start` (no behavior change; reused by
  the picker). ADR-0002 records the dependency decisions. Features
  003, 004, 005, 006 pass.
