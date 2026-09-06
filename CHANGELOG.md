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

### v1 exit bar

- 2026-09-06 — Feature-008 passed: features 001–007 green in both
  feature configurations, and a human walkthrough of all four TUI
  screens succeeded against real state (doctor, picker → start →
  finish, preview, policy edit persisting to the project layer).
  Recorded in PROGRESS.md. v1 now awaits only the README
  (DESIGN §22/§25).

### v1 complete: README + Omp Path A

- 2026-09-06 — Features 009–010 passed. README rewritten to the DESIGN
  §25 posture: one-liner, `cargo install --locked --git` (placeholder
  URL) + from-a-clone install, doctor/start/finish quick start with real
  output snippets, menu_tokens before/after, the not-a-skill-manager
  note, a TUI section, and a text-only gif-script. New Omp Path A
  adapter: `start --adapter omp` spawns `omp --config <run>/omp-config.yml`
  where the overlay pins `skills.customDirectories` to the sealed workdir
  and disables every discovery source (verified live against omp
  18.1.11: the child listed exactly the mounted skill; foreign pantry
  absent; `--no-skills` provably cannot substitute). `Adapter::
  isolation_argv` now takes the run dir; `adapters` lists none/pi/omp
  with a selftest that skips cleanly where omp is absent.

### Added (Path B milestone)

- 2026-09-06 — Features 011–012 passed. `start --from manifest.toml` runs
  multi-worker jobs: each worker's Skills mount under
  `workdir/packs/<worker>/` (the CLI `--skill` form keeps the flat union),
  `workers[0]` is the parent whose pack becomes the Path A scan root, and
  `workers[1..]` become run-local agent files under `runs/<id>/agents/` —
  pi-subagents frontmatter for pi, omp task-agent format for omp — plus an
  include hint, since neither harness can load agents from an arbitrary
  directory for one invocation (probed: no `agents.*` config key in omp
  18.1.11; no per-invocation override in pi-subagents 0.84.4). Standing
  agent dirs are never written; teardown removes `agents/` with the
  workdir. The token budget is now enforced per worker pack (the offending
  worker is named in the error), the lock lists every worker sharing each
  skill, `start --json` reports per-worker `menu_tokens` and the agents
  outcome, and a new `agents` audit event records adapter, files, and
  loaded true/false. Manifest input is intent only — `run_id`/`created_at`/
  `harness_argv` regenerate per run; `--skill` + `--from` are mutually
  exclusive; `--adapter`/`--task` flags beat manifest fields; manifest
  `[budget]` beats config. ADR-0003 records the semantics.
