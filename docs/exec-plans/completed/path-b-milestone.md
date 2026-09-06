# Lunchbox Path B milestone — features 011, 012

Spec source: `docs/design-docs/DESIGN.md` §9 (manifest/lock schemas), §10 (resolver rules), §11 (sealed workdir/packs), §13 (Path A spawn rules), §14 (Path B — verbatim contract), §16 (`start --from`, teardown of `agents/`), §18 (audit/result), §19 (adapter trait), §20 (guarantees), §24 (open questions), §26 (defaults). Repo state at start: `main` at `8b21936`, features 001–010 all `passes: true`, 84 unit + 22 integration tests green in both feature configurations, zero warnings. Host facts probed 2026-09-06 (planning session): `pi` 0.84.4 with extensions installed including `npm:pi-subagents` at `~/.pi/agent/npm/node_modules/pi-subagents` (its `docs/agents.md` documents frontmatter fields `inheritSkills`, `skills`, `skillPath` — relative `skillPath` resolves from the agent definition file; discovery scopes: builtin, package, user `~/.pi/agent/agents/**/*.md`, project `.pi/agents/**/*.md` + legacy `.agents/**/*.md`; `agentScope` selects scopes; **no per-invocation arbitrary-dir override exists** — no env var, no flag, no settings key). `omp` 18.1.11: `omp agents unpack [--user|--project|--dir <path>]` exports bundled task agents (`~/.omp/agent/agents` default, `./.omp/agents` project); omp agent-loading config key for a custom dir **unprobed** (execution Step 3 probes it). No git remote; repo local-only.

User choices (2026-09-06): `--from` runs always use per-worker packs (`workdir/packs/<worker>/`), even single-worker manifests; the CLI `--skill` form keeps today's flat union layout (zero churn to README/tests/Path A argv); pi Path B ships DESIGN §14's print-and-snippet fallback (no standing-file writes, ever); one milestone delivers both features 011 and 012.

## Context

DESIGN §23 step 11: Path B run-local agents + `--from manifest.toml` multi-worker packs. Today `start --from` bails `manifest-driven multi-worker runs arrive with Path B` (src/main.rs:349-351) and `Adapter::write_run_agents` bails `Path B not implemented in this build` in all three adapters (trait stub at src/adapter/mod.rs:37-38, `#[allow(dead_code)]`, zero production callers). Run-local agents already have a reserved, torn-down location: `teardown` removes `run_dir/agents/` today (src/run/mod.rs:365-369). TD-002 (move `prepare_run`/`PreparedRun` into `src/run/`) and TD-003 (`selftest` takes the detected version) payoff conditions both trigger this milestone.

## Approach

Execute steps in order; features complete one at a time (AGENTS.md). Flip each feature's `passes` only after its Verification loop passes.

### Step 1 — feature contract entries + plan filing

Append to `docs/feature-list.json` (both start `passes: false`; key order `id, category, description, steps, passes, priority, dependencies`):

- `feature-011`, category `functional`, priority 9, dependencies `["feature-001"]`, description "`--from manifest.toml` multi-worker runs mount per-worker packs under workdir/packs/<worker> with per-worker lock mapping and budget (DESIGN §9/§11/§24)", steps:
  1. "start --from <manifest> with two workers mounts workdir/packs/<worker>/ containing exactly each worker's pack; the lock lists every worker name sharing each skill; why reports each worker's pack"
  2. "Invalid manifests fail closed with no run dir left behind: unknown key, schema != 1, no workers, duplicate worker name, empty pack, --skill combined with --from"
  3. "The token budget is enforced per worker pack (DESIGN §10) and the offending worker is named in the error"
- `feature-012`, category `functional`, priority 10, dependencies `["feature-011"]`, description "Path B run-local agents: multi-worker starts write runs/<id>/agents/<worker>.md pointing at the worker's pack; omp loads them for the invocation only or the adapter prints the files plus an include hint (DESIGN §14 fallback); standing agent dirs are never written", steps:
  1. "A two-worker start writes agents/<worker>.md per DESIGN §14 (pi-subagents frontmatter: inheritSkills false, skillPath -> the worker's pack dir, explicit skills list) and teardown removes agents/"
  2. "omp loads the run-local agents for this invocation only (live-verified, foreign agents absent) or prints the files and the include hint and claims no load — audit records which"
  3. "adapters --explain states each adapter's Path B belief; ~/.pi/agent and ~/.omp/agent are byte-identical before and after a Path B run"

File this plan at `docs/exec-plans/active/path-b-milestone.md`.

### Step 2 — ADR-0003 + probe-independent DESIGN amendments

1. New `docs/decisions/0003-path-b-packs-and-from-semantics.md` (MADR format like 0001/0002, status accepted, date 2026-09-06) recording: `--from` runs always mount `workdir/packs/<worker>/` (single-worker included) while the CLI `--skill` form keeps the flat union (DESIGN §11's MVP blessing); manifest input is intent only — `run_id`/`created_at`/`harness_argv` in a supplied manifest are accepted and ignored, regenerated per run; precedence `--adapter`/`--task` flag over manifest field, manifest `[budget] max_menu_tokens` over config; `--skill` + `--from` mutually exclusive; **workers[0] is the parent** — its pack is the Path A scan root, workers[1..] become run-local agents; budget enforced per worker (§10); pi ships print-and-snippet mode (no arbitrary-dir agent loading exists in pi-subagents 2026-09-06); omp mode set by the Step 3 probe, no auto-fallback (fail/print honestly).
2. DESIGN.md amendments (resolution-note style like the omp §24 precedent):
   - §9: `Worker` gains optional `description` (used as the agent description; default `Lunchbox run-local agent for worker <name>`); note that a `--from` manifest's `run_id`/`created_at` are regenerated.
   - §11 + §24: packs question resolved — `--from` always packs, CLI form union; strike §24's last bullet with a resolved pointer to ADR-0003.
   - §16: `--from` precedence/conflict rules (one short block quoting ADR-0003's rules).
   - §18: audit gains event `{"event":"agents","adapter":<name>,"files":[<file names>],"loaded":<bool>}` written after `write_run_agents`.
   - §19: `write_run_agents(run_dir, agents) -> AgentFiles` (Rust shape below) replaces the sketch's `(run_dir, workers, pack_dirs)`.
   - §14's omp mechanism outcome + exact fallback text land in Step 6 (needs the probe result).

### Step 3 — omp task-agent probe (gates the omp adapter mode; no src/ changes)

Scratch under `/tmp/lbx-agents-probe/`. Record results in PROGRESS.md at Step 6 and pin them in omp's `explain()`.

1. `omp agents unpack --dir /tmp/lbx-agents-probe/bundled --json` → read one bundled `.md` → record the real frontmatter fields (name/description/…/any skills or tools fields). This is the omp agent-file format the adapter generates.
2. Write a marker agent `<name: marker-agent, one-line description>` into `/tmp/lbx-agents-probe/custom/` using that format.
3. Observer: `cd /tmp/lbx-agents-probe && omp -p --no-session --no-title --max-time 60 'List the names of your available task agents. Names only.'` — run once with no overlay (control: record the natively-listed set) and once per variant:
   - V1 overlay `agents: { customDirectories: [ /tmp/lbx-agents-probe/custom ] }` via `--config`;
   - V2: key name discovered from `strings ~/.local/bin/omp | grep -iE 'agent.*(dir|director)|customAgent'` if V1 fails;
   - V3: `--add-dir /tmp/lbx-agents-probe/custom` (documents a non-starter).
4. Pass = marker-agent listed AND the native set otherwise unchanged. Winning variant = omp Loaded mode.
5. Loaded mode only: rerun the Path A skills observer once with the COMBINED overlay (Path A skills block + agents block) to confirm skills isolation is unbroken (`marker-probe` skill visible, the five foreign skills absent).
6. If no variant passes: omp ships print mode (Step 5's UNVERIFIED branch). No third mode, no auto-fallback.

### Step 4 — `--from` core + packs (feature-011)

1. `src/run/mod.rs`:
   - `Worker` gains `#[serde(default, skip_serializing_if = "Option::is_none")] pub description: Option<String>`.
   - New `#[derive(Deserialize)] #[serde(deny_unknown_fields)] pub struct ManifestInput { pub schema: u32, pub task: String, pub adapter: String, #[serde(default)] pub run_id: Option<String>, #[serde(default)] pub created_at: Option<String>, #[serde(default)] pub harness_argv: Option<Vec<String>>, #[serde(default)] pub budget: Option<Budget>, pub workers: Vec<Worker> }` (the three Option fields are accepted-and-ignored — regenerated per run). New `pub fn read_manifest_input(path: &Path) -> Result<ManifestInput>`: missing file → bail `manifest '{path}' not found`; TOML error → `.context(format!("failed to parse manifest '{path}'"))`. New `ManifestInput::validate()`: `schema != 1` → `manifest schema {n} not supported (expected 1)`; `workers.is_empty()` → `manifest has no workers`; duplicate name → `duplicate worker name '{n}' in manifest`; empty pack → `worker '{n}' has an empty pack`.
   - `build_manifest(run_id, task, adapter, harness_argv, max_menu_tokens, workers: Vec<Worker>)` — takes workers instead of `skill_names`; the CLI call site passes `vec![Worker { name: "default".into(), pack: skill_names, description: None }]`.
   - `build_lock(run_id, mount_mode, workdir, locked, workers: &[Worker])` — per-skill `workers` = names of workers whose pack contains the skill, in manifest order, deduped (CLI form yields `["default"]`, byte-identical to today).
   - Budget: move the union check out of `resolve::resolve` into new `pub fn enforce_worker_budget(workers: &[Worker], locked: &[Locked], max_menu_tokens: u64, fail_on_budget: bool) -> Result<()>` in `src/resolve/mod.rs` — per-worker sum of its skills' `description_tokens`; over cap + fail → bail `worker '{w}': menu_tokens {m} exceeds max_menu_tokens {x} (fail_on_budget = true)`; over cap without fail → eprintln warning per worker. `resolve::resolve` drops its internal `enforce_budget` call; update the unit tests that pin the old union message to call `enforce_worker_budget` (they test the gate, not the plumbing).
   - TD-002: move `PreparedRun`, `prepare_run`, `mount_run` from `src/main.rs` into `src/run/mod.rs` (`pub(crate)`); `src/tui/picker.rs` switches to `crate::run::{PreparedRun, prepare_run}` (only import + call-site path change; picker still passes one default worker and empty argv). `PreparedRun` gains `workers: Vec<Worker>`, `worker_tokens: Vec<WorkerTokens>`, `packs_layout: bool`, `parent_pack: PathBuf`; new `pub struct WorkerTokens { pub name: String, pub menu_tokens: u64 }`.
   - `prepare_run(cfg, adapter_name, task, workers: Vec<Worker>, max_menu_tokens: u64, libraries, harness_argv)`: resolve the deduped union of all workers' pins (first-seen order across workers in manifest order) → new_run_id → mount_run. `mount_run`: `packs_layout = workers.len() > 1 || from-manifest` — concretely: cmd_start passes `packs_layout` via a new `prepare_run` param `use_packs: bool` (true only for the `--from` path; CLI form false). Mount: `use_packs` → `mount::mount_packs(locked, &workdir.join("packs"), &workers, mode)`; else today's `mount::mount(locked, &workdir, mode)`. `parent_pack` = `use_packs ? workdir/packs/<workers[0].name> : workdir`. `menu_tokens` stays the union sum. Then `enforce_worker_budget` (after lock write, before audit? No — before any write: enforce right after resolve, so failures leave no run dir; mount_run's error path already cleans, but budget failure should precede run-dir creation: call `enforce_worker_budget` inside `prepare_run` immediately after `resolve`). Audits `resolved`/`mounted` unchanged in shape.
2. `src/mount/mod.rs`: `pub fn mount_packs(locked: &[Locked], packs_root: &Path, workers: &[Worker], mode: MountMode) -> Result<MountMode>` — create `packs_root`; per worker create `packs_root/<name>` and link/copy each of its skills using the same per-package helpers `mount()` uses; ANY symlink failure → clean `packs_root` and redo all workers in copy mode (uniform all-or-nothing run-wide, same rule as `mount()`; returns the final `MountMode`). A skill in two workers is linked in both packs.
3. `src/main.rs::cmd_start`: replace the `--from` bail with: if `--from` and `--skill` non-empty → bail `--skill and --from are mutually exclusive; put pins in the manifest workers`; `let input = run::read_manifest_input(from)?.validate()?` — actually `validate(&self)`; `adapter_name = args.adapter.clone().unwrap_or(input.adapter.clone())`; `task = args.task.clone().unwrap_or(input.task.clone())`; `max_menu_tokens = input.budget.map(|b| b.max_menu_tokens).unwrap_or(cfg.max_menu_tokens)`; workers = `input.workers`; `use_packs = true`. CLI path: workers = default worker from `args.skills`, `use_packs = false`, budget/task/adapter as today. `start --json` output gains `"workers": [{"name": …, "menu_tokens": …}]` (always present, single entry for CLI form) — update the integration tests that pin `start --json` keys to expect the new field. Human summary output otherwise unchanged.
4. `src/main.rs::start_run`: `let scan_root = if prepared.packs_layout { &prepared.parent_pack } else { workdir };` → `adapter.isolation_argv(run_dir, scan_root, &parent_skills, harness_argv)` where `parent_skills = prepared.workers[0].pack.clone()` (CLI form: all locked names — identical today). `lock.workdir` and the mounted audit stay the run workdir root.
5. Tests (features 011): unit — `read_manifest_input` happy path + each validate bail text; `build_lock` worker mapping incl. a shared skill listing both worker names in manifest order; `enforce_worker_budget` per-worker message + warning path; `mount_packs` layout + symlink→copy fallback. Integration — replace `from_manifest_is_refused_this_phase` with `from_manifest_mounts_per_worker_packs` (temp HOME; fixture manifest written by the test: workers `parent = ["demo-review"]`, `reviewer = ["demo-scan"]`, adapter "none"; `start --from <path> --json`; assert: `workdir/packs/parent/demo-review` and `workdir/packs/reviewer/demo-scan` exist, no loose `workdir/<skill>` dirs; lock `workers` arrays exact; json `workers` array exact; `why` prints both `worker parent:` and `worker reviewer:` lines; `finish` → workdir and agents gone; `result.json` `unmounted: true`), `from_manifest_rejects_invalid` (each Step-1 error case: non-zero, exact stderr substring, zero run dirs under HOME), `from_manifest_spawn_uses_parent_pack` (mock-pi via existing helpers; `start --from <fixture> --adapter pi --no-wait -- -- pi -p hi`; audit spawn argv prefix exactly `["pi","--no-skills","--skill","<run>/workdir/packs/parent/demo-review","pi","-p","hello"]`-shaped i.e. one `--skill` for the parent's pack member then the user argv; abort cleans up). Fixture manifests are inline `fs::write` strings, not repo files.

### Step 5 — run-local agents (feature-012)

1. `src/adapter/mod.rs`:
   - `pub struct AgentSpec { pub name: String, pub description: String, pub pack_dir: PathBuf, pub skills: Vec<String> }` and `pub struct AgentFiles { pub loaded: bool, pub files: Vec<PathBuf>, pub include_hint: Option<String> }`.
   - Trait: replace the `#[allow(dead_code)]` `write_run_agents(&self, run_dir: &Path) -> Result<()>` with `fn write_run_agents(&self, run_dir: &Path, agents: &[AgentSpec]) -> Result<AgentFiles>;`.
   - TD-003: `fn selftest(&self, version: Option<&str>) -> Result<SelftestOutcome>;` — each impl: `let Some(version) = version else { return Ok(SelftestOutcome::Skipped) };` then the existing `help_flag_selftest`; `cmd_adapters` passes `version.as_deref()`. No other selftest callers exist (grep-verified).
   - Delete `path_b_is_refused`; add `none_write_run_agents_is_empty` (empty `AgentFiles`, no dir created) and `pi_write_run_agents_pins_frontmatter` (tempdir run_dir; one spec; assert the file content exactly — the template below).
2. `src/adapter/pi.rs::write_run_agents`: `create_dir_all(run_dir/agents)`; per spec write `agents/<name>.md` (error propagates with `with_context` like omp's overlay write):
   ```markdown
   ---
   name: {name}
   description: {description}
   inheritSkills: false
   skillPath: {pack_dir}
   skills: {skills joined ", "}
   tools: read, grep, find, bash
   ---
   Work only with the Skills in your skillPath. Do not search ~/.agents/skills or any global skill directory.
   ```
   Return `AgentFiles { loaded: false, files, include_hint: Some("pi-subagents discovers agents only from ~/.pi/agent/agents and the project's .pi/agents; lunchbox never writes those — copy runs/<run_id>/agents/*.md into one of them for this session; they must not outlive the run") }`. `explain()` gains: "Path B: writes runs/<id>/agents/<worker>.md with inheritSkills false, skillPath -> the worker's pack, explicit skills; pi-subagents (probed 2026-09-06 on 0.84.4) has no per-invocation agent-dir override, so the files are printed, not auto-loaded."
3. `src/adapter/none.rs::write_run_agents`: `Ok(AgentFiles { loaded: false, files: Vec::new(), include_hint: None })`.
4. `src/adapter/omp.rs::write_run_agents` — Step-3-gated:
   - Loaded: write `agents/<name>.md` in the probed omp format; `loaded: true`, hint None. `isolation_argv` overlay gains the probed agents key/dir entry (e.g. `agents.customDirectories`) **iff `run_dir/agents` is a directory** — self-contained, no signature growth; the overlay YAML template gains the block conditionally.
   - UNVERIFIED: pi-template files + `include_hint: Some("omp discovers task agents only from ~/.omp/agent/agents and ./.omp/agents; lunchbox never writes those — copy runs/<run_id>/agents/*.md into one of them for this session")`, `loaded: false`.
   - `explain()` states which mode and why (pinned probe result + date).
5. `src/main.rs::start_run` wiring (after the summary/json print, before `isolation_argv`): if `prepared.workers.len() > 1`, build specs from `workers[1..]` (`description` = worker description or `format!("Lunchbox run-local agent for worker {name}")`, `pack_dir` = `workdir/packs/<name>`, `skills` = pack) → `let agent_files = adapter.write_run_agents(run_dir, &specs)?;` → audit `{"event":"agents","adapter":adapter_name,"files":[file names],"loaded":agent_files.loaded}` → human: if `!files.is_empty()` print `agents         {n} run-local{loaded ? ", loaded by " + adapter : " (printed; not auto-loaded)"}`; print `include_hint` when `Some` (prefix `note           `); json: add `"agents": {"files": [...], "loaded": bool}` when workers > 1. A write failure propagates → `cmd_start`'s error path removes the run dir (DESIGN §20.3 holds).
6. Tests (feature-012): the Step-4 integration tests already assert agents dir contents + teardown (extend `from_manifest_mounts_per_worker_packs`: `agents/reviewer.md` exists with `skillPath: <…>/workdir/packs/reviewer`, `inheritSkills: false`, `skills: demo-scan`; audit `agents` event `loaded: false` for adapter none — wait, adapter none + workers>1: none returns empty files → no agents line/event; so assert agent files under a `--adapter pi --dry-run` variant instead: `from_manifest_pi_prints_agents` — mock-pi PATH, `start --from <fixture> --adapter pi --dry-run -- -- pi -p hi`; assert `agents/reviewer.md` content, audit `agents` event `loaded:false` + `files:["reviewer.md"]`, dry-run argv printed, then `finish`). omp mock test: `omp_manifest_overlay_and_agents` (mock-omp; VERIFIED branch → `run/omp-config.yml` contains both the skills `customDirectories` pointing at `packs/parent` and the agents dir entry, audit `agents` event `loaded:true`; UNVERIFIED branch → overlay skills-only + `loaded:false`) — the branch not taken is asserted by the same test reading the mode the adapter compiled with; write the test against the implemented mode and record the other in the plan's contingency. Update `adapters --explain` expectations if any test pins pi/omp explain text (none do today).

### Step 6 — Verification + bookkeeping

Run the Verification section end to end on a clean tree; flip `passes` for 011 then 012; DESIGN §14 omp-mechanism amendment + omp/pi `explain()` final text; update `PROGRESS.md` (state, verification table incl. both feature configs and the Step 3 probe outcome, TD-002/TD-003 recorded paid — remove their tracker rows), `AGENTS.md` phase line, `CHANGELOG.md` entry; move this plan to `docs/exec-plans/completed/`; commit `Path B run-local agents + --from manifests (features 011-012)`.

## Critical files & anchors

- `src/run/mod.rs` — Manifest/Worker/Lock builders (L136-182 hardcoded `default`), teardown already removes `agents/` (L365-369); receives `PreparedRun`/`prepare_run`/`mount_run` (TD-002) + `ManifestInput`.
- `src/main.rs` — `--from` bail to replace (L349-351), `start_run` agents wiring + `scan_root`/`parent_skills`, `cmd_adapters` selftest version pass-through.
- `src/mount/mod.rs` — new `mount_packs` beside `mount` (L8-32); reuse its per-package helpers; uniform fallback rule.
- `src/adapter/mod.rs` — trait: `write_run_agents` + `AgentSpec`/`AgentFiles`, `selftest(version)`; `path_b_is_refused` replaced.
- `tests/cli.rs` — `from_manifest_is_refused_this_phase` (L486-495) replaced; mock helpers L583-614 reused; run-id-from-json pattern (L419) for any multi-run test.

## Verification

Working dir: repo root; `. "$HOME/.cargo/env"`. Fake tree = temp HOME (no pantry needed — `--library testdata/skills` supplies pins). Fixture manifest `$TMP/m.toml`: `schema = 1`, `task = "path b check"`, `adapter = "none"`, `[[workers]] name = "parent" pack = ["demo-review"]`, `[[workers]] name = "reviewer" description = "Review specialist" pack = ["demo-scan"]`.

1. `cargo test` and `cargo test --no-default-features` → both green, zero warnings.
2. feature-011: `HOME=$T cargo run -- start --from $TMP/m.toml --library testdata/skills --adapter none --json` → exit 0; `workers` array `[{"name":"parent","menu_tokens":14},{"name":"reviewer","menu_tokens":13}]`; `packs/parent/demo-review` + `packs/reviewer/demo-scan` present; lock per-skill `workers` exact; `HOME=$T cargo run -- why` lists both workers; `HOME=$T cargo run -- finish` → workdir and agents gone; `result.json` `unmounted: true`. Error cases: schema 2 / unknown key / dup worker / empty pack / no workers / `--skill`+`--from` → each non-zero with the pinned stderr and `ls $T/.lunchbox/runs` empty (or unchanged). Budget: manifest with a 1500-token cap and `fail_on_budget` config (project `lunchbox.toml`) → `worker 'parent': menu_tokens …` naming the worker.
3. feature-011 spawn: mock-pi PATH, `start --from $TMP/m.toml --adapter pi --no-wait -- -- pi -p hi` → audit spawn argv begins `pi --no-skills --skill <run>/workdir/packs/parent/demo-review`; abort cleans.
4. feature-012 pi: `start --from $TMP/m.toml --adapter pi --dry-run -- -- pi -p hi` → `agents/reviewer.md` with `inheritSkills: false`, `skillPath: …/packs/reviewer`, `skills: demo-scan`; stdout prints the printed-mode agents line + include hint; audit `agents` event; `finish` removes `agents/`.
5. feature-012 omp (Step 3 VERIFIED only): scratch agent `marker-agent` + `HOME=<real> cargo run -- start --from <m-omp.toml> --adapter omp --wait -- -- omp -p --max-time 90 'List the names of your available task agents. Names only.'` (m-omp.toml: adapter "omp", workers parent/reviewer) → reply contains `reviewer` (the run-local agent) and the control set is otherwise unchanged; `run/omp-config.yml` holds both the skills block (→ `packs/parent`) and the agents block (→ `run/agents`); workdir gone after; standing dirs untouched: `ls ~/.omp/agent/agents` and `~/.pi/agent/agents` absent before and after; `pi list` output identical before/after. UNVERIFIED branch instead: assert the printed-mode line + hint and `omp-config.yml` skills-only.
6. No drift: `HOME=<fake> cargo run -- start --library testdata/skills --skill demo-review --skill demo-scan --adapter none` → identical output to the README snippet (`menu_tokens    this run: 27`, flat `workdir/<skill>` layout — `packs/` absent); `adapters` rows unchanged.

## Assumptions & contingencies

- **omp probe may fail** (no agents-dir config key): feature-012 step 2 then passes via the print/hint branch — both outcomes passing is by design (DESIGN §14 fallback); the implementer does not choose, Step 3's predicate decides, and `adapters --explain` says which.
- **omp agent-file format** comes from `omp agents unpack --dir` output in Step 3; if the format has no skills-directory field at all, the generated omp agent file points at the pack dir via whatever field exists (or the body text names the pack path) and the plan's pi template is not reused for omp — record the exact generated file in PROGRESS.
- **pi print mode is final for this milestone** (no arbitrary-dir loading exists in pi-subagents; writing `~/.pi/agent/agents` or project `.pi/agents` would violate DESIGN §14/§20.2 and is not an option). If a future pi-subagents adds a per-invocation dir flag, a later milestone can flip pi to Loaded.
- **`start --json` gains a `workers` key** (additive); the integration tests pinning `start --json` keys are updated in Step 4 — if a snapshot test elsewhere pins that JSON, update it the same way.
- **`enforce_worker_budget` relocation** changes the over-budget error text (now worker-named) for the CLI form too; unit tests pinning the old union message are updated to the new text — intentional uniformity, recorded in ADR-0003.
- **Concurrent-run ambiguity** (`latest_run` lexicographic) predates this milestone and is not expanded here; Path B tests capture run ids from `--json`.
- **TD-001** (flaky pi spawn test) is out of scope; its payoff condition (a reproducing failure) is unchanged by this milestone.
- **Pi Path B live check** needs no LLM (file contents + hint are deterministic); the omp live check reuses the sanctioned headless-observer pattern with `--no-session --no-title` and one bounded model call.
