# Lunchbox README + Omp Path A milestone — features 009, 010

Spec source: `docs/design-docs/DESIGN.md` §13 (Path A · Omp), §19 (adapter trait), §21 (install/tests/layout), §22 (Finish v1 + Next), §23 steps 9–10, §24 open question #1, §25 (README posture), §26 (defaults). Repo state at start: v1 exit bar passed (features 001–008 all `passes: true`), `main` at `ce6e08f`, 84 unit + 21 integration tests green in both feature configs, zero warnings. Host facts probed 2026-09-06: `omp` installed at `/home/nathan/.local/bin/omp` (version 18.1.11 per `~/.omp/agent/last-changelog-version`; `~/.omp/agent/skills/` contains 5 skills: debug, grill-me, handoff, project-foundation, simplify — usable as foreign markers); `pi` 0.84.4 also present. No git remote is configured (repo is local-only).

User choices (2026-09-06): milestone scope = README + Omp Path A (Path B deferred to its own milestone); README gives the TUI its own short section after the CLI loop.

## Context

Close out v1 (README, DESIGN §23 step 9) and add the Omp Path A adapter (step 10): `lunchbox start --adapter omp` must spawn omp with per-invocation skill discovery off and the sealed run workdir as the only skill source, selftest-guarded, with `cargo test` still passing on machines without omp. DESIGN §13 rule: if discovery cannot be verifiably disabled, refuse to claim Path A isolation — the adapter then ships detect/doctor/adapters support and a fail-closed spawn. Omp's mechanism differs from pi's: omp has `--no-skills` and a glob-filter `--skills`, but NO per-path `--skill` flag; the explicit-root mechanism is a per-invocation `--config <file>` overlay setting `skills.customDirectories`, with source-discovery toggles (`enableAgentsUser/Project`, `enableClaude*`, `enableCodexUser`, `enablePi*`) and `disabledProviders` in the same file. `OMP_PROFILE` alone is NOT sufficient (external `~/.claude`/`~/.codex` and project bases still load). The exact overlay recipe is DESIGN §24 open question #1 — it is probed live in Step 3 below and pinned before coding.

## Approach

Execute steps in order; features complete one at a time (AGENTS.md). Flip each feature's `passes` only after its Verification loop passes.

### Step 1 — feature contract entries

Append to `docs/feature-list.json` (both start `passes: false`):

- `feature-009`, category `release`, priority 7, dependencies `[]`, description "v1 README per DESIGN §25: one-liner, install, doctor/start/finish quick start, menu_tokens before/after, not-a-skill-manager note, TUI section", steps:
  1. "Every README command runs verbatim against a fresh clone with a temp HOME and the shown output matches"
  2. "Install section names cargo install --locked --git (placeholder URL, no remote configured) and the from-a-clone path"
  3. "README states it is not a skill manager and points --library at Kitter / Skills Manager / ~/.agents/skills"
- `feature-010`, category `functional`, priority 8, dependencies `["feature-007"]`, description "Omp Path A: start --adapter omp spawns omp with discovery off and the run workdir as the only skill source — or refuses Path A explicitly if isolation cannot be verified (DESIGN §13)", steps:
  1. "adapters lists omp with version and a passing selftest where omp is installed, skipped cleanly where absent"
  2. "start --adapter omp --dry-run prints the exact spawn argv including the per-run --config overlay whose content points skills.customDirectories at the workdir"
  3. "A live spawn loads exactly the mounted skills (foreign pantry names absent), or the adapter refuses to spawn with an explicit error naming the probe result"

### Step 2 — README (feature-009)

1. Create `README.md`, sections in this exact order (DESIGN §25: do not lead with architecture):
   - `# lunchbox` + the one-liner, verbatim: "Your agent only sees these Skills for this job. Then they vanish."
   - **Install**: `cargo install --locked --git <repository-url>` (placeholder — no remote exists yet; add one inline note "replace with the real repository URL once published") and the from-a-clone alternative `cargo install --locked --path .`.
   - **Quick start** (installed-binary form `lunchbox …`, with one preceding line: from a clone, prefix `cargo run --`): `lunchbox doctor` (empty pantry → `menu_tokens 0`), then `lunchbox start --library testdata/skills --skill demo-review --skill demo-scan --adapter none` showing `menu_tokens    this run: 27` and the workdir/skills lines, then `lunchbox finish` (workdir gone). Snippets show the real output text; `doctor` snippet uses a machine with an empty pantry so `menu_tokens 0` is honest.
   - **menu_tokens before/after**: one short paragraph — `doctor` reports what your agent would eat without Lunchbox (its whole pantry); `start` reports what this run actually gets (only the pinned pack); link the two numbers from the quick start.
   - **Not a skill manager**, verbatim sentence: "This is not a skill manager. Point `--library` at Kitter / Skills Manager / `~/.agents/skills`."
   - **TUI** (own short section): `lunchbox tui doctor`, `tui picker --library <pantry>`, `tui preview`, `tui policy` — one line each on what they show/do (picker: compose a pack, start, finish, all from the screen; policy: view/edit allow/deny per config layer).
   - **gif-script**: text-only transcript block ( fenced, the quick-start commands with their outputs abridged) standing in for an animated demo.
2. No other doc changes in this step. Note: `ARCHITECTURE.md`, `AGENTS.md` mention no README — no cross-edits needed.

### Step 3 — live Omp isolation probe (gates Step 4; no src/ changes)

Run on this machine (omp 18.1.11 present). Record results in PROGRESS.md at Step 6 and pin them in the adapter's `explain()`.

1. Pin the surface: `omp --version`; `omp --help | grep -E -- '--no-skills|--skills|--config|--profile'`.
2. Find a deterministic "which skills loaded" observer, trying in order until one works (record which): (a) latest `~/.omp/logs/omp.*.log` lines naming discovered/loaded skills for a run; (b) headless `omp -p --max-time 20 --config <overlay> 'List the names of your available skills. Names only.'` stdout; (c) `--mode json` events naming skill loads.
3. Build a scratch pantry `/tmp/lbx-probe/skills/marker-probe/SKILL.md` (valid frontmatter: `name: marker-probe`, one-line description). Overlay variant V1 at `/tmp/lbx-probe/overlay.yml`:
   ```yaml
   skills:
     enabled: true
     customDirectories:
       - /tmp/lbx-probe/skills
     enableAgentsUser: false
     enableAgentsProject: false
     enableClaudeUser: false
     enableClaudeProject: false
     enableCodexUser: false
     enablePiUser: false
     enablePiProject: false
   disabledProviders: [native, claude, codex, gemini, github, opencode, cursor, agents-md]
   ```
   Run the observer with `--config /tmp/lbx-probe/overlay.yml`. Pass = marker-probe visible AND none of the five foreign names (debug, grill-me, handoff, project-foundation, simplify) AND no managed-skill names.
4. If omp hard-errors on unknown/invalid keys (overlays are strict), drop only the offending keys it names → V2; if foreign names persist, add a `managed-skills`-gate key discovered from `omp config`/docs → V3. Also run V-fail = `--no-skills` + customDirectories once and record that marker-probe is NOT visible (documents why the pi-style flag recipe cannot work).
5. **Winning recipe** = minimal overlay variant that passes step 3's predicate, plus the observer that proved it. This exact YAML (with the workdir path substituted) is what Step 4 writes and what selftest re-verifies.
6. **If no variant isolates**: set outcome REFUSED — Steps 4–5 implement the refusal shape instead (below); feature-010 step 3 passes via the refusal branch.

### Step 4 — Omp adapter (feature-010)

1. Widen the trait signature in `src/adapter/mod.rs` (omp's overlay needs the run dir; pi/none ignore it):
   `fn isolation_argv(&self, run_dir: &Path, workdir: &Path, skills: &[String], user_argv: &[String]) -> Result<Vec<String>>;`
   Update `src/adapter/pi.rs` and `src/adapter/none.rs` to the new signature (bodies unchanged except the extra `_run_dir: &Path` param) and the single call site in `src/main.rs::start_run` (`let argv = adapter.isolation_argv(run_dir, workdir, &skill_names, harness_argv)?;`).
2. New `src/adapter/omp.rs` mirroring `pi.rs` structure:
   - `name()` → `"omp"`; `detect()` → `super::detect_version("omp")`.
   - `skill_dirs(_cfg)` → `[cwd .agents/skills, $HOME/.agents/skills]` + `$HOME/.omp/agent/skills` and `$HOME/.omp/agent/managed-skills` each only `if is_dir()` (same shape as pi's conditional dir).
   - `isolation_argv(run_dir, workdir, _skills, user_argv)` — outcome VERIFIED (Step 3): write the winning overlay YAML verbatim with `customDirectories: [<workdir>]` to `run_dir/omp-config.yml` (create_dir_all not needed — run dir exists; write error propagates), return `["omp", "--config", "<run_dir>/omp-config.yml"]` + `user_argv` verbatim. Outcome REFUSED: `bail!("omp Path A refused: cannot verifiably disable skill discovery (probed omp 18.1.11, 2026-09-06)")` (date = probe date).
   - `agent_dir_hint()` → `Some($HOME/.omp/agent)`; `write_run_agents(_run_dir)` → keep the existing bail `"Path B not implemented in this build"` (Path B is out of scope).
   - `selftest()`: detect None → `Skipped`; else run `omp --help` (nonzero exit → error) → `Ok` iff help contains `--config`; else `Failed(format!("omp isolation flags drifted (omp {version}: --config={has_config})"))`. If Step 3's observer is cheap and deterministic (log-based), additionally re-run it against a scratch overlay and make a mismatch `Failed`; help-grep alone otherwise, with the limit stated in `explain()`.
   - `explain()`: the belief text — mechanism (per-run `--config` overlay + `skills.customDirectories` + source toggles off), why `--no-skills`/`OMP_PROFILE` are insufficient, and "verified against omp 18.1.11 on 2026-09-06 by <observer>; selftest re-verifies".
3. `src/adapter/mod.rs::resolve_adapter`: replace the `"omp" => bail!("omp adapter arrives after Phase 1")` arm with `"omp" => Ok(Box::new(OmpAdapter))`, add `pub mod omp; pub use omp::OmpAdapter;`, and change the unknown-adapter text to `"unknown adapter '{other}' (available: none, pi, omp)"`.
4. `src/main.rs::cmd_adapters`: change the hardcoded loop to `for name in ["none", "pi", "omp"]`. Nothing else in main.rs (start flow, `without_estimate`, doctor, TUI picker's hardcoded `"none"` all work unchanged).

### Step 5 — tests (feature-010)

1. Update pinned-text unit tests in `src/adapter/mod.rs`: `adapter_resolution` drops the omp-bail assertion (now resolves; assert `resolve_adapter("omp").unwrap().name() == "omp"`) and pins the new unknown-adapter text; `path_b_is_refused` gains `OmpAdapter` alongside none/pi.
2. `tests/cli.rs`: replace `omp_adapter_refused_this_phase` with `omp_spawn_records_audit_and_overlay`, copying the mock-pi pattern: new `mock_omp(scratch_dir) -> PathBuf` helper writing `bin/omp` as `#!/bin/sh` + redirect stdio to /dev/null + `printf '%s\n' "$@" > <scratch>/argv-record` + `sleep 60` (chmod 0o755), and a `PATH`-prepend helper (reuse `path_with_mock_pi`'s shape — parameterize it as `path_with(cwd, "bin")` or clone it; pick one, no duplication). The test: temp HOME + fake tree, `start --library testdata/skills --skill demo-review --adapter omp --no-wait -- -- omp -p hello`; assert success; `only_run` → spawn audit argv prefix `["omp","--config","<run>/omp-config.yml","omp","-p","hello"]`; assert `run/omp-config.yml` exists and contains the workdir path under `customDirectories`; then `abort` → workdir gone, outcome `aborted`. Add `omp_selftest_skips_when_binary_absent`: `adapters` with a PATH stripped of omp's directory (temp dir containing only required binaries via PATH override to a near-empty dir plus /usr/bin:/bin) → omp row prints `not found; selftest skipped`, command still exits 0 (mirrors pi-absent behavior; assert stdout contains `omp` and `skipped`).
3. No snapshot/TUI changes (screens don't surface adapter lists).

### Step 6 — Full verification + bookkeeping

Run the Verification section end to end on a clean tree; flip `passes` for 009 then 010; update `PROGRESS.md` (state, verification table incl. both feature configs and the Step 3 probe outcome, blockers table — replace the Omp row with the resolved recipe or the refusal record, next move = Path B milestone), `AGENTS.md` phase line, `CHANGELOG.md` entry; file this plan at `docs/exec-plans/active/omp-readme-milestone.md` at start, move to `completed/` at end; update `docs/exec-plans/completed/phase-1.md`? — no, history stays. Commit: `README + Omp Path A adapter (features 009-010)`.

## Critical files & anchors

- `src/adapter/omp.rs` (new) — the adapter; modeled line-for-line on `src/adapter/pi.rs` (detect/selftest/explain patterns).
- `src/adapter/mod.rs` — `Adapter::isolation_argv` signature widening (trait + `resolve_adapter` omp arm + unknown-adapter text + `adapter_resolution`/`path_b_is_refused` tests).
- `src/main.rs` — `start_run` call site (pass `run_dir`), `cmd_adapters` loop `["none","pi","omp"]`.
- `tests/cli.rs` — `mock_pi`/`path_with_mock_pi` (~:506-529) as the mock-omp template; `omp_adapter_refused_this_phase` (~:498) replaced.
- `README.md` (new) — section order fixed by Step 2.

## Verification

Working dir: repo root; `. "$HOME/.cargo/env"`. Fake tree = temp `HOME` with `.agents/skills/{demo-review,demo-scan}` copied from `testdata/skills` (same as `fake_tree_home` in tests/cli.rs).

1. `cargo test` and `cargo test --no-default-features` → both green, zero warnings (84+ unit / 21+ integration counts grow by Step 5's tests).
2. Feature-009: `git clone /mnt/dev/projects/lunchbox $TMP/clone && cd $TMP/clone` (local clone stands in for fresh clone; no remote exists) → run every README quick-start command verbatim with a temp `HOME` (installed-binary form via `cargo run --` prefix, or `cargo install --locked --path .` into a temp CARGO_HOME then use `lunchbox`) → outputs match the README snippets (exact strings `menu_tokens    this run: 27`, finish's workdir-gone behavior). Grep README: contains the one-liner, `<repository-url>` placeholder, the not-a-skill-manager sentence, all four `tui` commands.
3. Feature-010: `lunchbox adapters` on this machine → rows for none/pi/omp, omp showing `18.1.11 selftest: ok` (or `skipped` under the stripped-PATH test env); `HOME=<fake> cargo run -- start --library testdata/skills --skill demo-review --adapter omp --dry-run -- -- omp -p hi` → printed argv begins `omp --config <run>/omp-config.yml` and the overlay file exists with the workdir under `customDirectories`; then the live isolation check (VERIFIED outcome only): `HOME=<fake> cargo run -- start --library testdata/skills --skill demo-review --adapter omp --wait -- -- omp -p --max-time 30 'List the names of your available skills. Names only.'` → output contains `demo-review` and none of `debug`/`grill-me`/`handoff`/`project-foundation`/`simplify`; workdir gone after exit; `result.json` `unmounted: true`. REFUSED outcome instead: `start --adapter omp …` exits non-zero with the refusal message naming the probe, and `adapters` still lists omp's version.
4. `HOME=<fake> cargo run -- doctor` still reports pi rows unchanged (no behavior drift from the trait widening).

## Assumptions & contingencies

- **README install URL is a placeholder** because the repo has no remote; swap in the real URL when one exists (one-line change recorded in the README itself). User may supply a real URL before execution — then use it.
- **Probe outcome branches the adapter** (VERIFIED vs REFUSED) per DESIGN §13's refusal rule; both are feature-010-passing outcomes — the implementer does not choose, Step 3's predicate decides.
- **Observer may be LLM-based** (`omp -p` reply): acceptable for the one-time Step 3 probe and the live check in Verification 3, but selftest stays on the deterministic help-grep (plus log-based check only if Step 3 found one), so CI never needs omp or network.
- **Overlay keys may drift across omp versions**: selftest's help-grep catches `--config` disappearing; explain() pins the verified version/date; if a future omp breaks the recipe, `adapters --explain` says so — no auto-fallback, ever (fail closed).
- **detect_version on omp**: `omp --version` output format is assumed semver-bearing like pi's; if `omp --version` prints no parseable semver, `detect()` errors and selftest reports it — fix by extending `extract_semver` only if the format is trivially adjacent; otherwise record and treat as detection failure (doctor shows "not detected").
- **Path B stays refused** in all three adapters this milestone (`write_run_agents` bails unchanged); `--from` bail text unchanged. Both are the next milestone's scope, not silently expanded here.
- **Mock-omp argv test** does not depend on the probe outcome: isolation_argv is exercised through the mock binary; in the REFUSED outcome, replace that test with a refusal-message assertion (start fails, no spawn audit, no overlay file).
