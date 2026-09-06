# Progress — lunchbox

Last updated: 2026-09-06 (TUI milestone complete)

## Current repository state

Phase 1 + TUI milestone implemented: core CLI + Pi Path A adapter
(features 001, 002, 007) and the four v1 TUI screens behind cargo
features — doctor, token-cost preview, skill picker, policy review
(features 003–006), each with a `--json` twin and headless snapshot
tests. `src/tui/` (terminal, snap, doctor, preview, picker, policy) with
a state/render/handle_event split; `tui` subcommand parses in every
build and fails closed without the features. Default features include
both TUI features; `--no-default-features` ships the CLI-only binary.
82 unit + 21 integration tests green in both configurations. Features
001–007 pass. Branch `main`; tree clean after each phase commit.

Not yet implemented (per DESIGN §22/§23): v1 exit bar human walkthrough
(feature-008), Omp Path A adapter, Path B run-local agents, `--from`
multi-worker manifests, scan-command hook, `skills/lunchbox/` driver
Skill.

## TUI milestone verification (2026-09-06, this machine)

Exec-plan: `docs/exec-plans/completed/tui-milestone.md`; decisions in
ADR-0002 (`docs/decisions/0002-tui-dependencies.md`).

| Check | Command | Result |
|---|---|---|
| Full suite (default features) | `cargo test` | ok — 82 unit + 21 integration, 0 failed, 0 warnings |
| Full suite (CLI-only) | `cargo test --no-default-features` | ok — 69 unit + 21 integration, 0 failed, 0 warnings |
| Fail-closed TUI | `cargo run --no-default-features -- tui doctor` | non-zero, `this build has no TUI screens; rebuild with default features or --features tui-doctor,tui-menu (…)` (unit-tested per screen in `src/main.rs::tui_tests`) |
| feature-003 doctor twin | `HOME=<fake> … tui doctor --json` vs `doctor --json` | byte-identical (integration test `tui_doctor_json_twin_matches_doctor_json`; snapshot `doctor_screen_two_skills`) |
| feature-004 preview twin | `HOME=<fake> … tui preview --json --skill demo-review --skill demo-scan` | `menu_tokens 27`, `without_menu_tokens 27`, `over_budget false`; single skill → 14 (parity unit test + integration test) |
| feature-005 picker | state-machine test `picker_start_mounts_and_finish_unmounts` + snapshot | exactly one run dir; workdir = exactly the two packages; `f` → workdir gone, `result.json` `unmounted: true`; quit implies finish (no leaked mount); pty smoke select→s→f→q exit 0 |
| feature-006 policy | unit `add_deny_on_project_layer_preserves_format` + integration `feature_006_policy_edit_persists_to_project_layer_and_denies` | project layer gains `deny = ["demo-scan"]` with comments/keys intact; global layer byte-identical; `start --skill demo-scan` → `denied by policy`; pty smoke wrote the entry through the real screen |
| Doctor refactor parity | golden capture before/after `DoctorReport` extraction | human + `--json` stdout byte-identical |
| `prepare_run` refactor | full suite (pins `start --json` + audit contents) | green, no behavior change |

## Phase 1 verification (2026-09-06, this machine)

| Check | Command | Result |
|---|---|---|
| Full test suite | `cargo test` | ok — 67 unit + 15 integration, 0 failed |
| Build warnings | `cargo build` | clean |
| feature-001 loop | `start --library testdata/skills --skill demo-review --skill demo-scan --adapter none` → `finish` → `finish` | exit 0; workdir = exactly the two packages; `menu_tokens this run: 27`; workdir gone; `result.json` `unmounted: true`; second finish idempotent exit 0 |
| Deny gate | `lunchbox.toml` with `deny = ["demo-scan"]`, temp `HOME` | non-zero, message names the denial, no run dir created (integration test `deny_gate_leaves_no_run_dir`) |
| feature-002 | `doctor --json` against `HOME` fixture with both demo packages | skill count 2, `menu_tokens` 27, per-skill 14/13, no run dir created (integration test `feature_002_doctor_reports_tree_without_mounting`) |
| feature-007 | `start --library testdata/skills --skill demo-review --adapter pi --wait -- -- pi --list-models` | exit 0; audit `spawn` argv begins `pi --no-skills --skill <workdir>/demo-review`; workdir gone after exit; `result.json` `outcome: "ok"`, `unmounted: true` |
| Dry run | `… --adapter pi --dry-run -- -- pi --list-models` | exact argv one token per line; mounts; no spawn event; no pid file; `finish` cleans up |
| Adapter selftest | `adapters`, `adapters --explain` | `pi 0.84.4 selftest: ok`; explain prints the flag belief with the 0.84.4/2026-09-06 verification note |
| Crash teardown | `gc` after `touch -d '2 days ago'` | stale run dir removed and printed (integration test `leaked_run_is_collected_by_gc`) |
| Hash golden | unit test `golden_fixture_hash_is_stable` | `sha256:6cffec6f…3c15b0c`, independently recomputed from the spec before implementation |
| SIGINT path | integration test `sigint_forwards_and_aborts` | exit 130, child SIGTERMed, workdir removed, outcome `aborted` |

## Design decisions settled during implementation

- Config merge algebra (DESIGN §24 open item): scalars project-wins; `deny`
  unions (deduped, global order first); `allow` intersects in global order
  (one-sided passes through, both-empty = allow any); `library_paths`
  project-prepended to global; CLI `--library` prepends ahead of both.
- Hash pin form: `name@sha256:<64 lowercase hex>`; anything else with `@`
  is refused as a tag pin.
- `-- --` handling: clap consumes the first `--`; a leading `--` in the
  collected harness argv is dropped so DESIGN §13's documented form works.
- `without_menu_tokens` in `result.json` is recomputed at teardown from the
  manifest's adapter (same estimator, same dirs) rather than persisted.
- Deviation from plan dependency list: added `parking_lot` (dev-only) for
  the mutex-guarded `HOME` test helper, per repo rule (parking_lot over
  std::sync when unwrapping).
- assert_cmd blocks on inherited stdout pipes: the mock `pi` used in
  integration tests redirects its own stdio to /dev/null so `--no-wait`
  spawns don't wedge the harness for the mock's lifetime.

## Post-Phase-1 simplify pass (2026-09-06)

Three-lane review (reuse / quality / efficiency) of the Phase 1 diff;
accepted and fixed: hash-pin disambiguation arm was dead code
(`find_in_root` inherited `scan_root`'s duplicate-name error, so
"duplicate names resolve via hash pin" never ran — now `find_in_root`
enumerates without the gate; regression-tested + CLI-verified);
`finish`/`abort` now consult `result.json` before touching a pid (no
signaling stale/recycled pids on already-torn-down runs); `--wait`
teardown reuses the in-scope `Config` instead of re-loading (a config
edited mid-run can no longer fail finish or diverge
`without_menu_tokens`); SIGINT/SIGTERM handlers register before
resolve/mount with a post-mount checkpoint (Ctrl-C during a slow start
cleans the run dir instead of leaking it until `gc`); `adapters` runs
each selftest once; doctor scans each skill dir once (typed reports,
one tolerant error policy); `FoundSkill` carries `description` (drops
triple SKILL.md re-reads and two dead fallback branches); dead
`From<Expanded>` impl, doubled `#[test]` attribute, duplicated mount
copy loop, and half-applied `pick` closure removed; integration tests
use the checked-in `testdata/skills` fixtures and a shared mock-PATH
helper. Rejected: `kill -0` polling via libc (dep weight), per-pin scan
caching (CLI scale), lock/status stringly-typing (TOML schema contract).

## Post-TUI simplify pass (2026-09-06)

Three-lane review (reuse / quality / efficiency) of the TUI milestone diff
(`add3404`); 12 findings accepted and fixed: picker teardown now fails
closed (a failed finish keeps the run handle for retry; quitting with an
unmountable run errors non-zero instead of leaking silently — regression
tested via a read-only run dir); `library::scan_roots` propagates scan
errors so the picker twin fails closed on duplicate-name/IO-broken roots
instead of listing `{"library":[]}` exit 0; `tui preview` no longer tells
hash-pin users to "pin by hash" (accurate name-only message) and the
interactive screen preselects `--skill` pins like its `--json` twin;
resolve's not-found error construction is shared with `tokens::preview`;
`append_layer_entry` writes via temp-file + rename (no truncate-in-place
of user-authored TOML); `tui policy` loads each layer once
(`PolicyState::load` delegates to `from_reports`); `global_path()` bails
without HOME instead of returning a relative path; `cmd_doctor` no longer
computes fattest/duplicates on the JSON path; `terminal::install`
releases raw mode when a later setup step fails; integration probes are
cached per feature group (`LazyLock`) instead of one spawn per test;
`config::List` replaces the stringly "allow"/"deny" selector and
`WhichLayer::as_str` replaces Debug-format headers; the demo-tree fixture
is shared by preview/picker tests. Deferred to TD-002: moving
`prepare_run` into `src/run/`. Verified: `cargo test` (84 unit + 21
integration) and `cargo test --no-default-features` (69 + 21) green with
zero warnings; hash-pin/tag-pin messages, dup-root picker failure,
doctor twin byte-identity, no-HOME fail-closed, atomic policy write, and
deny-after-write all exercised against the built binary.

## Active work

None in flight.

## Blockers and unknowns

| Unknown | Blocks | Resolution path |
|---|---|---|
| Omp per-invocation discovery-off keys (`--skills` filter semantics, `OMP_PROFILE`) | Omp Path A adapter | Probe at adapter implementation (DESIGN §24) |
| Pi `--skill` flag semantics (per-package vs pantry-root) | Pi adapter robustness | Plan contingency: if spawned pi errors or loads nothing, switch `isolation_argv` to single `--skill <workdir>`; current per-package form verified working on 0.84.4 |

## Next useful move

Feature-008 (v1 exit bar): the scripted MVP CLI loop re-verified, then
the human walkthrough of all four TUI screens against real state —
doctor on real dirs, picker → start → finish, policy edit persisting to
the right layer — recorded here with date. After the exit bar: Omp
Path A adapter (DESIGN §23 step 10), then Path B run-local agents.
