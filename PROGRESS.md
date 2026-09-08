# Progress — lunchbox

Last updated: 2026-09-08 (pantry milestone)

## Current repository state

v1 + Path B + scan hook + pantry milestone complete. Phase 1 + TUI
milestone + two simplify passes + the README/Omp milestone + the Path B
milestone + the scan-hook milestone + the pantry (thin git installer)
milestone: core CLI, Pi and Omp Path A adapters, the four v1 TUI screens
behind cargo features, the DESIGN §25 README, `--from manifest.toml`
multi-worker runs with run-local agent files, the `scan_command` policy
gate with `--override-scan` (ADR-0004), and `lunchbox add` / `update`
acquiring whole skill repos into `~/.lunchbox/pantry` (ADR-0006 — the
2026-09-08 pivot that narrowed the "not a skill manager" posture to
per-skill management). 114 unit + 36 integration tests green in the
default configuration, 99 + 36 with `--no-default-features`, zero
warnings in both. **Features 001–014 all pass.** Branch `main`; tree
clean after each phase commit.

Distribution (ADR-0005): MIT, git-only install from
https://github.com/nathanpt/lunchbox, tagged `v0.1.0`; crates.io and
prebuilt Release binaries deferred.

## Distribution verification (2026-09-06, this machine)

Repo created and pushed with `gh` (account `nathanpt`, `repo` scope):
`main` + tag `v0.1.0` at `d3b8225`. Clean-root install of the exact
blessed command — `cargo install --locked --git
https://github.com/nathanpt/lunchbox --tag v0.1.0 --root /tmp/lbx-dist` —
succeeded; installed binary reports `lunchbox 0.1.0` (tagged commit
`d3b8225`), and `adapters` detects the real pi 0.84.4 / omp 18.1.11 with
selftests ok. Pre-push: `cargo test` (105+30) and
`cargo test --no-default-features` (90+30) green, zero warnings;
`cargo package --list` accepted the manifest (license/repository/readme/
rust-version present). ADR-0005 confirmation evidence satisfied.

## Pantry milestone verification (2026-09-08, this machine)

Exec-plan: `docs/exec-plans/completed/pantry-milestone.md`; contract in
ADR-0006 (`docs/decisions/0006-thin-git-installer.md`). Binary
`target/debug/lunchbox`; mock-git integration tests run a `git` shell
script on PATH (clone materializes fixtures, `-C … pull --ff-only`
succeeds or simulates divergence, `--version` prints 2.99.0-mock); the
real GitHub flow runs only in the live checks below.

| Check | Command | Result |
|---|---|---|
| Full suite (default features) | `cargo test` | ok — 114 unit + 36 integration, 0 failed, 0 warnings |
| Full suite (CLI-only) | `cargo test --no-default-features` | ok — 99 + 36 integration, 0 failed, 0 warnings |
| feature-014 live acquisition | `HOME=<fake> lunchbox add https://github.com/nathanpt/agent-skills` | flagless success: `added agent-skills`, root `…/pantry/agent-skills/skills` (`skills/` auto-detected), `skills 6` |
| feature-014 resolve | `start --skill grill-me --adapter none` (no `--library`) → `finish` | mounts from the managed pantry (`menu_tokens this run: 15`), workdir gone, `unmounted: true` |
| feature-014 update | `lunchbox update` | `updated agent-skills` (real `git pull --ff-only`) |
| Doctor surface | `doctor` / `doctor --json` | `git 2.53.0` row; `pantries:` section with name/root/count; json `git` + `pantries` keys; `menu_tokens 0` unchanged (managed pantries never enter the without estimate) |
| Fail-closed family | mock-git integration tests | ambiguous repo → `multiple candidate skill roots` + clone removed; clone failure → exit + stderr excerpt; existing name → `already exists`; `update ghost` → `no managed pantry named`; divergence → `exit 128` + `Not possible to fast-forward`; junk dir in pantry → `start` fails naming the pantry, `doctor` reports `error:` and exits 0; empty PATH → `failed to run git clone` + `git (not found)` |
| `--path` override | `add …/ambiguous --path skills` | succeeds where auto-detect would fail; `ambiguous.path` records `skills`; `update` keeps honoring it |
| No drift | byte-compare vs pre-change binary (scratch worktree at `8900e7c`), empty pantry, run-id normalized | `adapters`, `tui preview --json`, `start` human output identical; `doctor` differs only by the `git` row |
| README | doctor snippet vs live output | byte-identical (abbreviations per the note); add example captured live 2026-09-08 |

Deviations from the exec-plan, recorded here: the added parallel test
load exposed a pre-existing race — `--no-wait` spawn tests read the mock
harness's argv record immediately after `start` exits, before the child
had necessarily written it (`omp_manifest_overlay_and_agents` failed
once under load); the four racy sites now use a polling
`read_spawn_record` (5 s ceiling; the scanner-record site is synchronous
and unchanged). Verified stable across three consecutive full-suite
runs. The plan's `search_roots` ripple covered exactly the four listed
call sites; the doctor snapshot update was the only pinned-output change
(the expected `git [2.99.0]` line).

Exec-plan: `docs/exec-plans/completed/scan-hook-milestone.md`; contract in
ADR-0004 (`docs/decisions/0004-scan-hook-contract.md`). Fake tree = temp
HOME; project `lunchbox.toml` in a scratch cwd supplies `scan_command`;
binary `target/debug/lunchbox`.

| Check | Command | Result |
|---|---|---|
| Full suite (default features) | `cargo test` | ok — 105 unit + 30 integration, 0 failed, 0 warnings |
| Full suite (CLI-only) | `cargo test --no-default-features` | ok — 90 + 30 integration, 0 failed, 0 warnings |
| feature-013 pass | `scan_command = 'exit 0'` → `start --library testdata/skills --skill demo-review --adapter none --json` | exit 0; lock `scan = "pass"`; audit `{"event":"scan","command":"exit 0","skills":[{"name":"demo-review","scan":"pass"}],"override":false}`; `finish` → `unmounted: true` |
| Invocation shape | `scan_command = 'printf '\''%s'\'' >> $TMP/record'` → single-skill and two-worker `--from` runs | record holds the resolved pantry source path; the two-worker run with `demo-review` in both packs records `demo-review` once (union dedupe); both locks `scan = "pass"` |
| feature-013 fail closed | `scan_command = 'echo findings >&2; exit 3'` | exit 1, stderr `skill 'demo-review' failed scan_command 'echo findings >&2; exit 3' (exit 3): findings`; `~/.lunchbox/runs` absent (scan runs before run-dir creation) |
| Spawn failure | same config, `PATH` = empty dir | exit 1, `failed to run scan_command '…': No such file or directory (os error 2)` |
| feature-013 override | same config + `--override-scan --json` | exit 0; stderr `warning: skill 'demo-review' failed scan_command '…' (overridden by --override-scan)`; lock `scan = "overridden"`; audit `"override":true`; `finish` cleans (`unmounted: true`) |
| JSON purity | `scan_command = 'echo junk'` → `start --json` | full stdout parses as exactly one JSON object (scanner stdout captured, not inherited) |
| Default / no drift | no `scan_command` anywhere | lock `scan = "none"`, no scan audit event; `doctor`, `adapters`, `tui preview --json`, and the README-shape `start` human output byte-identical to the pre-change binary (built from HEAD in a scratch worktree); README untouched |
| Unit gates | resolve tests `scan_*` | pass/none/overridden vocabulary, error names skill+command+`exit 3`+stderr excerpt, spawn-failure bail (integration tests `scan_pass_invokes_scanner_and_locks`, `scan_fail_fails_closed`, `scan_override_proceeds_and_audits`, `scan_json_stays_pure`) |
`doctor`, `tui preview`, and `adapters` never spawn a scanner by design —
`tokens::preview` does not call `resolve`; confirmed empirically by the
byte-identical outputs above.

Deviations from the exec-plan, recorded here: the plan's sample recording
scanner `printf '%s' \"$1\" > record` writes `$1` into the config *and*
relies on lunchbox appending the package source as the final argument —
printf then prints both and the record doubles (observed live). Per
ADR-0004 the config string never references `$1`; the tests use
`printf '%s' > record` / `printf '%s\n' >> record` and still prove the
shell-string semantics via the redirection. The unit spawn-failure test
exercises the private `spawn_scan` seam with a nonexistent program (an
in-process PATH mutation would race concurrent scan tests); the real
spawn failure is verified end-to-end above with an empty `PATH`.

## Post-scan simplify pass (2026-09-06)

Three-lane review (reuse / quality / efficiency) of the scan-hook diff
(`21445ab`); quality lane re-verified the whole contract against the real
binary (invocation shape, dedupe, fail-closed placement, vocabulary, JSON
purity, `--dry-run` scans, scan-free doctor/preview, audit order — all
match ADR-0004/DESIGN/feature-013). Accepted and fixed: the
`scan_audit_event` test helper violated the recorded explicit-scripts
convention (parameterizing it was rejected by the Path B simplify pass) —
removed, both call sites inline the read-audit chain like the five
pre-existing sites; `scan_skill` pass-through wrapper deleted (resolve
calls the recorded `spawn_scan` seam directly); the stderr excerpt strips
CR as well as collapsing LF (CRLF scanner stderr kept rendering as a
carriage return inside the "single-line" message — regression-tested);
ADR-0004 drifted from the shipped code (its evidence quoted the abandoned
`printf '%s' "$1" > record` scanner and never stated the no-`$1`-in-config
rule PROGRESS attributed to it) — corrected, and its consequences now
record the unbounded v1 output capture (TD-002) and the point-in-time
scan verdict / live symlink source / read-only-scanner expectation; stray
double blank line removed. Rejected: a parameterized `audit_event` helper
(prior explicit rejection), `twin_stdout` reuse in the new tests (dominant
convention is the inline chain), lock scan enum-izing and `prepare_run`
param bundling (recorded rejections), bounded capture plumbing now
(feature-scale change; TD-002 records the payoff condition). Verified:
`cargo test` (105 unit + 30 integration) and
`cargo test --no-default-features` (90 + 30) green, zero warnings; CR
collapse asserted at unit level and the fail-closed message re-checked
against the rebuilt binary.

## Path B milestone verification (2026-09-06, this machine)

Exec-plan: `docs/exec-plans/completed/path-b-milestone.md`; decisions in
ADR-0003 (`docs/decisions/0003-path-b-packs-and-from-semantics.md`).

**Omp task-agent probe (outcome: NO override exists → print mode).**
omp 18.1.11. `omp agents unpack --dir` exports five bundled agents whose
frontmatter is name/description/tools(/spawns/model/thinkingLevel/output) —
no skills or skillPath field. Observer = headless
`omp -p --no-session --no-title --max-time 60 'List the names of your
available task agents. Names only.'`; control listed exactly scout,
reviewer, security-reviewer, task, sonic. V1 overlay
`agents.customDirectories` via `--config`: ignored (marker-agent absent) —
and the binary's full dotted settings-key schema enumeration contains no
`agents.*` key at all. V2 (strings-discovered `PI_CODING_AGENT_DIR`): the
env var relocates the whole agent state root, not just agents — the child
dies with "No models available" because models.db/secrets live under the
real `~/.omp/agent` (and it would fork sessions into a gc-eligible run
dir). V3 `--add-dir`: adds a *workspace* directory (omp --help), not an
agent discovery scope — non-starter. Scratch: `/tmp/lbx-agents-probe/`.
Both adapters therefore ship DESIGN §14's print fallback; `adapters
--explain` pins the probe result and date.

| Check | Command | Result |
|---|---|---|
| Full suite (default features) | `cargo test` | ok — 98 unit + 26 integration, 0 failed, 0 warnings |
| Full suite (CLI-only) | `cargo test --no-default-features` | ok — 83 unit + 26 integration, 0 failed, 0 warnings |
| feature-011 happy path | `start --from $TMP/m.toml --library testdata/skills --adapter none --json` (parent=[demo-review], reviewer=[demo-scan]) | exit 0; `workers` = `[{"name":"parent","menu_tokens":14},{"name":"reviewer","menu_tokens":13}]`; `packs/parent/demo-review` + `packs/reviewer/demo-scan` present, workdir root holds only `packs/`; lock per-skill `workers` exact (`["parent"]` / `["reviewer"]`), lock workdir = run workdir root; `why` prints both `worker parent:` and `worker reviewer:` lines; `finish` → workdir+agents gone, `result.json` `unmounted: true` (integration test `from_manifest_mounts_per_worker_packs`) |
| feature-011 fail-closed | schema 2 / unknown key / dup worker / empty pack / no workers / `--skill`+`--from` / missing file | each non-zero with the pinned stderr (`manifest schema 2 not supported (expected 1)`, `failed to parse manifest`, `duplicate worker name 'w' in manifest`, `worker 'w' has an empty pack`, `manifest has no workers`, `--skill and --from are mutually exclusive…`, `manifest '<path>' not found`); `~/.lunchbox/runs` empty after all (integration test `from_manifest_rejects_invalid`) |
| feature-011 budget | manifest `[budget] max_menu_tokens = 10` + project `lunchbox.toml` `fail_on_budget = true` | `worker 'parent': menu_tokens 14 exceeds max_menu_tokens 10 (fail_on_budget = true)`, exit 1, no run dir; soft path warns per worker and proceeds; manifest budget wins over config and is recorded (unit tests `budget_*`) |
| feature-011 spawn | mock-pi, `start --from … --adapter pi --no-wait -- -- pi -p hi` | audit spawn argv = `["pi","--no-skills","--skill","<run>/workdir/packs/parent/demo-review","pi","-p","hi"]` — only the parent's pack member; abort cleans (integration test `from_manifest_spawn_uses_parent_pack`) |
| feature-012 pi | same manifest, `--adapter pi --dry-run` | `agents/reviewer.md` exactly per DESIGN §14 (`inheritSkills: false`, `skillPath: …/packs/reviewer`, `skills: demo-scan`, `tools: read, grep, find, bash`); stdout `agents 1 run-local (printed; not auto-loaded)` + `note` include hint; audit `{"event":"agents","adapter":"pi","files":["reviewer.md"],"loaded":false}`; `finish` removes `agents/` (integration test `from_manifest_pi_prints_agents`) |
| feature-012 omp (print mode) | same manifest, `--adapter omp --no-wait` (mock + real binary dry-run) | omp-format `agents/reviewer.md` (name/description/tools frontmatter, body names the pack dir); overlay stays skills-only pointing at `packs/parent`; audit agents event `loaded:false`; printed-mode line + omp include hint; abort/finish clean (integration test `omp_manifest_overlay_and_agents`) |
| Standing trees | sha256 of `~/.pi/agent`, `~/.omp/agent/{agents,skills}` before/after pi+omp Path B runs | byte-identical; `~/.pi/agent/agents` and `~/.omp/agent/agents` absent before and after. Whole-`~/.omp/agent` hashing is not a stable oracle on this machine (omp's own `models.db-wal`/`terminal-sessions`/composer caches churn independently of lunchbox), so the guarantee is asserted on the subtrees DESIGN §20.2 protects |
| No drift (CLI form) | `start --library testdata/skills --skill demo-review --skill demo-scan --adapter none` | identical README-snippet output (`menu_tokens    this run: 27`), flat `workdir/<skill>` layout, `packs/` absent; `adapters` rows unchanged |
| TD-002 | `prepare_run`/`PreparedRun` moved to `src/run/`; picker imports `crate::run::` | paid; tracker row removed; ARCHITECTURE.md aligned |
| TD-003 | `selftest(version: Option<&str>)`; `cmd_adapters` passes its detection down | paid; tracker row removed; one `--version` spawn per present harness |

Deviations from the exec-plan, all recorded here: the agents write in
`start_run` runs *before* the summary/json print (not after) so the json
object can carry the `agents` key the plan also requires — audit order still
resolved → mounted → agents → spawn per DESIGN §18; worker packs are
normalized to resolved names after resolve (a hash pin in a pack would
otherwise miss its locked name in `mount_packs`/`build_lock`, and the
generated manifest records resolved names exactly as the CLI form always
did); the budget check ran with cap 10 instead of the plan's illustrative
1500 (the demo pantry's packs are 14/13 tokens, so 1500 cannot trip);
`start --json` also gained a `workers` key for CLI-form runs (single
`default` entry) per the plan's additive-json note, pinned by
`start_json_reports_numbers`; omp's generated agent file is omp-native
format (the probed format has no skills field — the plan's pi template is
not reused for omp, per its contingency; the exact generated file is
asserted in `omp_manifest_overlay_and_agents` and shown in the table above).

## Post-Path-B simplify pass (2026-09-06)

Three-lane review (reuse / quality / efficiency) of the milestone diff
(`1f9abbe`), 20 findings reconciled to 10 accepted. Must-fix (quality lane):
`--from` worker names flowed unvalidated into filesystem paths —
`name = "../../../.pi/agent/agents/backdoor"` escaped the run dir and wrote
a standing agent dir, violating DESIGN §20.2; `validate` now rejects any
name that is not exactly one Normal path component (regression-tested at
unit + CLI level; escape attempt verified to fail closed with nothing
written). Accepted cleanups: `deny_unknown_fields` on nested
`Worker`/`Budget` (a `descritpion` typo inside `[[workers]]` was silently
dropped); `PreparedRun.packs_layout`+`parent_pack` collapsed into one
`scan_root` field (the old conditional re-derived what `parent_pack`
already encoded); `Worker::menu_tokens` is now the single definition shared
by the budget gate and `start --json` (they could silently disagree);
`Worker::default_pack` pins the `default` worker contract at both call
sites; `run::pack_dir` owns the `workdir/packs/<name>` convention;
pi/omp `write_run_agents` scaffolding extracted to
`adapter::write_agent_files` (pantry_base_dirs/help_flag_selftest
precedent); `mount::pack_dests` unifies the symlink/copy walker and bails
loudly on an unresolved pack entry instead of silently skipping;
`normalize_packs` uses `resolve::parse_pin`; the duplicated agent
file-name mapping hoisted to `agent_file_names`; the four inline
`--from` fixtures deduplicated into `two_worker_manifest`. Rejected:
bundling `prepare_run`/`mount_run` params into a request struct (refactor
churn on plan-mandated signatures), an `audit_event` test helper (rejected
by the prior simplify pass — explicit-scripts convention), and indexing
the by-name `find` loops (quantified below the bar at workers ≤ 10 ×
packs ≤ 50). Verified: `cargo test` (100 unit + 26 integration) and
`cargo test --no-default-features` (85 + 26) green, zero warnings; escape
manifest fails closed with no standing-dir write and no run dir; happy
path, agents output, and the CLI form byte-checked unchanged.

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

`skills/lunchbox/` driver Skill (DESIGN §22 "Next", the last v1 item): a
Skill that drives lunchbox itself so an agent can mount its own sealed
run. Both adapters' Path B is print mode until a harness grows a
per-invocation agent-dir override (ADR-0003 records the flip condition).
