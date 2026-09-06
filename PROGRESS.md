# Progress — lunchbox

Last updated: 2026-09-06 (README + Omp Path A milestone)

## Current repository state

v1 complete. Phase 1 + TUI milestone + two simplify passes + the README /
Omp milestone: core CLI, Pi **and Omp** Path A adapters, the four v1 TUI
screens behind cargo features, and the DESIGN §25 README. 84 unit + 22
integration tests green in the default configuration, 69 + 22 with
`--no-default-features`, zero warnings in both. **Features 001–010 all
pass.** Branch `main`; tree clean after each phase commit.

Next (DESIGN §22 "Next"): Path B run-local agents, `--from` multi-worker
manifests, scan-command hook, `skills/lunchbox/` driver Skill.

## v1 exit bar walkthrough (feature-008, 2026-09-06)

Step 1 (features 001–007 pass): proven by `cargo test` and
`cargo test --no-default-features` in both simplify-pass runs (84 unit +
21 integration each, zero warnings). Step 2 (human walkthrough):
performed by the user in a real terminal against real state on
2026-09-06 — doctor on real dirs, picker → start → finish, token
preview, and a policy edit persisting to the project layer in a scratch
project (keeping the global layer untouched); user verdict: "all four
screens behaved". Step 3: this record.

## README + Omp milestone verification (2026-09-06, this machine)

Exec-plan: `docs/exec-plans/completed/omp-readme-milestone.md`.

**Omp isolation probe (outcome VERIFIED).** omp 18.1.11; `--config` loads
a repeatable per-run config overlay. Observer = headless
`omp -p --no-session --no-title --max-time … 'List the names of your
available skills. Names only.'` (logs name no skills — observer (a) dead).
Winning recipe: overlay setting `skills.customDirectories` to the sealed
workdir plus `enableAgentsUser/Project`, `enableClaudeUser/Project`,
`enableCodexUser`, `enablePiUser/Project` all false and
`disabledProviders: [native, claude, codex, gemini, github, opencode,
cursor, agents-md]`. Probes: with the overlay the reply listed exactly
`marker-probe` (2/2 runs); without it the five foreign skills
(debug, grill-me, handoff, project-foundation, simplify) appeared;
`--no-skills` + customDirectories listed NONE — the pi-style flag recipe
cannot work, `--skills` only filters discovered skills, and
`OMP_PROFILE`/`--profile` isolates auth/session state, not discovery.

| Check | Command | Result |
|---|---|---|
| Full suite (default features) | `cargo test` | ok — 84 unit + 22 integration, 0 failed, 0 warnings |
| Full suite (CLI-only) | `cargo test --no-default-features` | ok — 69 unit + 22 integration, 0 failed, 0 warnings |
| feature-009 quick start | clean copy of the tree + installed binary (`cargo install --locked --path .` into a temp CARGO_HOME), empty temp HOME: `doctor` → `finish` → `status` | every snippet matches: `menu_tokens    0  (0 skills union)`, `menu_tokens    this run: 27`, `without 0 (no skills found in none skill dirs)`, finish removes workdir |
| feature-009 README content | grep | one-liner, `<repository-url>` placeholder, verbatim not-a-skill-manager sentence, all four `tui` commands present |
| feature-009 pi example | `start … --adapter pi --wait -- -- pi -p "review the staged diff"`, temp HOME | spawns with isolation flags; pi exits 1 (no API key under temp HOME); teardown ran, `unmounted: true` |
| feature-010 adapters | `lunchbox adapters [--explain]` | `omp 18.1.11 selftest: ok`; explain pins the overlay belief + probe date; omp-absent PATH → `omp not found; selftest skipped`, exit 0 (integration test `omp_selftest_skips_when_binary_absent`) |
| feature-010 dry run | `HOME=<fake> start --library testdata/skills --skill demo-review --adapter omp --dry-run -- -- omp -p hi` | argv = `omp --config <run>/omp-config.yml omp -p hi`; overlay holds the workdir under `customDirectories`; `without ~27 (2 skills on omp global+project)` — omp skill_dirs see the fake tree |
| feature-010 live spawn | `start … --adapter omp --wait -- -- omp -p --max-time 90 'List the names of your available skills. Names only.'` | reply: exactly `demo-review`; none of the five foreign skills (present on this machine — `without ~93 (5 skills on omp global+project)`); exit 0, workdir gone, `result.json` `unmounted: true`. First attempt at `--max-time 30` hit omp's deadline before any reply (title-gen adds a model call); 90 s sufficed |
| Mock-omp spawn | integration test `omp_spawn_records_audit_and_overlay` | audit spawn argv prefix `omp --config <run>/omp-config.yml` + user argv; overlay written; abort → workdir gone, outcome `aborted` |
| Doctor drift | `HOME=<fake> doctor` and `doctor --adapter omp` | pi rows unchanged (2 skills, menu_tokens 27); omp override scans the same fake tree |

Deviations from the exec-plan, all recorded here: the README existed since
Phase 1, so Step 2 rewrote it to the DESIGN §25 contract instead of
creating one (the old snippet's "pi skill dirs" `without`-line was stale —
real output says "none skill dirs" for `--adapter none`); `start`'s
isolation summary line gained an omp branch (`path A config overlay:
--config <run>/omp-config.yml`) so omp does not print pi's flags — the
plan said "nothing else in main.rs", but the shared line would have been
false for omp; the live isolation check ran with the real HOME instead of
the fake tree (a fake HOME strips omp's auth so no reply is possible, and
the real `~/.omp/agent/skills` pantry is the stronger adversary).

## Post-README/Omp simplify pass (2026-09-06)

Three-lane review (reuse / quality / efficiency) of the milestone diff
(`6f4892a`); accepted and fixed: `skill_dirs` pantry base and the
`--help` selftest scaffold were near-verbatim copies across pi/omp — now
`pantry_base_dirs()` and `help_flag_selftest(binary, version, required)`
in `src/adapter/mod.rs` (pi's drift message byte-identical, verified);
the adapter registry was triplicated (`resolve_adapter`, `cmd_adapters`,
the unknown-adapter text) — now one `ADAPTERS` const; start's isolation
label was a string-match on adapter names in main.rs (mechanism
knowledge leaking out of the trait, introduced by this milestone) — now
`Adapter::isolation_summary()`; ARCHITECTURE.md ("omp refuses", diagram)
and DESIGN §13/§19/§24 still described pre-omp reality — synced
(§19 signature + `isolation_summary`; §24 #1 annotated resolved);
README snippet paths (`~`, `<project>`) read as literal output against
feature-009's "output matches" — one abbreviation note added; omp's
explain() omitted the `disabledProviders` half of the recipe — appended;
`overlay_yaml` was a single escaped string (the security-load-bearing
artifact) — raw-string template, output byte-identical (cmp against
pre-change dry-run); `abort_on_finished_run_is_a_no_op` had lost its
`.stdout(contains("aborted"))` assertion to a mid-session edit mishap —
restored from `ce6e08f`; the test mock orphaned a `sleep 60` child on
abort — `exec sleep 60`. Deferred to TD-003: selftest re-detects (a
duplicate `--version` spawn per present harness in `adapters`, cold
path). Rejected: extracting the audit-parse block in tests (file
convention is explicit scripts), renaming `isolation_argv` (churn; doc
sync instead), restripping the absent-bin test PATH (plan-prescribed).
Verified: `cargo test` (84 unit + 22 integration) and
`cargo test --no-default-features` (69 + 22) green, zero warnings;
`adapters`/`start`/`omp-config.yml` outputs byte-identical to the
pre-change binary after run-id/path normalization.

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
| Omp per-invocation discovery-off keys | Omp Path A adapter | **Resolved 2026-09-06**: per-run `--config` overlay (recipe above); probe VERIFIED against omp 18.1.11 |
| Pi `--skill` flag semantics (per-package vs pantry-root) | Pi adapter robustness | Plan contingency: if spawned pi errors or loads nothing, switch `isolation_argv` to single `--skill <workdir>`; current per-package form verified working on 0.84.4 |

## Next useful move

Path B run-local agents (DESIGN §23 step 11): `write_run_agents` still
bails "Path B not implemented in this build" in all three adapters;
`--from` still refused. Scope it as its own milestone per DESIGN §14,
including the `workdir/packs/<worker>` layout decision (§24).
