# Progress — lunchbox

Last updated: 2026-09-06 (Phase 1 complete)

## Current repository state

Phase 1 implemented: core MVP + Pi Path A adapter. Rust CLI
(`src/main.rs` + `config`/`library`/`hash`/`resolve`/`mount`/`tokens`/`run`/
`adapter{,/none,/pi}` modules), `testdata/skills` demo pantry, 67 unit
tests + 15 integration tests + 15 integration tests, all green. Features 001, 002, 007 pass.
Branch `main`; commits: foundation `c14f6f9`, Phase 1 `05744ba`; tree clean.

Not yet implemented (per DESIGN §22/§23): TUI milestone (features 003–006,
Ratatui behind cargo features), Omp Path A adapter (DESIGN §23 step 10),
Path B run-local agents (DESIGN §23 step 11), `--from` multi-worker
manifests, scan-command hook, `skills/lunchbox/` driver Skill.

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

## Active work

None in flight.

## Blockers and unknowns

| Unknown | Blocks | Resolution path |
|---|---|---|
| Omp per-invocation discovery-off keys (`--skills` filter semantics, `OMP_PROFILE`) | Omp Path A adapter | Probe at adapter implementation (DESIGN §24) |
| Pi `--skill` flag semantics (per-package vs pantry-root) | Pi adapter robustness | Plan contingency: if spawned pi errors or loads nothing, switch `isolation_argv` to single `--skill <workdir>`; current per-package form verified working on 0.84.4 |

## Next useful move

DESIGN §23 step 7 — the TUI milestone (features 003–006): four Ratatui
screens behind cargo features (`tui-menu`, `tui-doctor`), each with a
`--json` twin and snapshot tests. Highest-priority incomplete features in
`docs/feature-list.json` are now 003/004.
