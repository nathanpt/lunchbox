# Changelog

Notable, release-significant changes to lunchbox. Format follows
[Keep a Changelog](https://keepachangelog.com/); dates are ISO-8601. Internal
edits that a user would not notice do not belong here.

## [Unreleased]

### Added (driver skill)

- 2026-09-08 — Feature-015 passed. `skills/lunchbox/SKILL.md`: a driver
  Skill that teaches a mounted agent the lunchbox loop — `doctor` for
  the standing cost, `start` with explicit pins (the worker sees only
  the mount), `finish` always, fail-closed on errors. The repository is
  itself a pantry: `lunchbox add
  https://github.com/nathanpt/lunchbox` auto-detects `skills/`, and the
  driver Skill resolves with no `--library`. DESIGN §22 "Next (v1.x)"
  is now complete.

## [0.2.0] - 2026-09-08

### Added (packaging)

- 2026-09-08 — v0.2.0 tagged and pushed at
  https://github.com/nathanpt/lunchbox (`cargo install --locked --git
  … --tag v0.2.0`; the tagless form installs the default-branch tip).
  crates.io and prebuilt binaries stay deferred per ADR-0005.

### Added (pantry milestone)

- 2026-09-08 — Feature-014 passed (ADR-0006). `lunchbox add <git-url>`
  clones a whole skills repository into `~/.lunchbox/pantry/<name>` and
  auto-detects the pantry inside it — the repository root, or exactly one
  first-level subdirectory of Skill packages (`skills/`); ambiguous or
  empty repositories fail closed with the clone removed, and `--path
  <subdir>` pins the choice explicitly. Managed pantries join the resolver
  search order after user `library_paths` (which keep first-wins priority)
  and before the `.agents/skills` defaults, so pinned Skills resolve with
  no `--library` once added. `lunchbox update [name]` fast-forward pulls
  each clone and fails loudly on divergence; removal is deleting the
  directory — `add`/`update` never write config. `doctor` gains a `git`
  row and a `pantries:` section (managed pantries never enter the
  without-Lunchbox estimate). This narrows the v1 "not a skill manager"
  posture: acquisition of whole repositories is in scope, per-skill
  install/version/edit stays out (git and Kitter / Skills Manager own
  that). README rewritten: context-cost-led intro with the blast-radius
  support and audience, an "Adding skills" section, and the updated
  not-a-manager boundary.

## [0.1.0] - 2026-09-06

### Added (packaging)

- 2026-09-06 — v0.1.0 tagged and published at
  https://github.com/nathanpt/lunchbox. MIT license (`LICENSE`);
  `Cargo.toml` gains `license`, `repository`, `readme`, and
  `rust-version = "1.85"`. Install is git-only (ADR-0005):
  `cargo install --locked --git https://github.com/nathanpt/lunchbox
  --tag v0.1.0`; crates.io is deferred (`lunchbox` name is taken by an
  unrelated crate), prebuilt Release binaries deferred.

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

### Added (scan-hook milestone)

- 2026-09-06 — Feature-013 passed. A configured `scan_command` now gates
  every start: it runs once per locked skill, pre-mount on the pantry
  source, as a shell string — `sh -c '<scan_command> "$1"' sh <source>` —
  so arguments, quoting, and redirections work naturally and the package
  source path is appended as the final quoted argument. Non-zero exit
  fails closed (skill, command, exit code, and a bounded stderr excerpt
  named; no run dir left behind); `--override-scan` proceeds past a
  failure, prints a warning, and audits it — a CLI-only flag the TUI
  picker cannot set. The lock's `scan` field records the truth: `pass`,
  `none` (no scanner configured), or `overridden` — `fail` never appears
  because a failed scan without override aborts before the lock exists.
  Scanner stdout/stderr is captured, keeping `start --json` one parseable
  line, and a `scan` audit event (command, per-skill results, override)
  follows `resolved`. ADR-0004 records the contract.
