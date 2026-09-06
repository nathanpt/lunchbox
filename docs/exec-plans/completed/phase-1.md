# Lunchbox Phase 1 — core MVP + Pi Path A (features 001, 002, 007)

Spec source: `docs/design-docs/DESIGN.md` (§7–§25, revised 2026-09-06). Acceptance: `docs/feature-list.json` entries feature-001, feature-002, feature-007. Language: Rust, unconditional (ADR-0001). Repo state at start: docs-only, 17 files, branch `main`, nothing committed, no Rust toolchain installed.

## Context

Build the first runnable increment of `lunchbox`, a run-scoped skill runtime: resolve explicit Skill pins from a pantry, mount a sealed per-run workdir, hand it to a worker, always unmount. Phase 1 = DESIGN §23 steps 1–9: skeleton + two-layer config, hasher, library reader, resolver, mount/teardown, lifecycle CLI (`start/status/finish/abort/why/gc`) with adapter `none`, token estimator + `doctor`, the **Pi Path A adapter** (spawn `pi` with isolation flags), and the README. TUI screens, Omp adapter, and Path B run-local agents are later phases (DESIGN §23 steps 10–11 stay out).

Verified environment facts (2026-09-06, this machine): `pi 0.84.4` at `~/.local/bin/pi`; `pi --help` contains `--no-skills, -ns` ("Disable skills discovery and loading") and `--skill <path>` ("Load a skill file or directory (can be used multiple times)"). `~/.agents`, `./.agents`, `~/.claude` do not exist — `doctor` must tolerate absent dirs. `cargo`/`rustc`/`rustup` are not installed.

## Approach

Execute steps in order; each step ends with `cargo test` green where tests exist.

### Step 0 — Prerequisites and commits

1. Install Rust: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal`, then source `~/.cargo/env` in every shell before cargo commands. Verify `cargo --version` ≥ 1.85 (edition 2024).
2. Commit the existing foundation (user-authorized): `git add -A && git commit -m "Foundation: design, ADR-0001, feature contract"`.
3. File this plan: `cp` its content to `docs/exec-plans/active/phase-1.md` (repo convention per AGENTS.md; move to `completed/` in Step 13).

### Step 1 — Cargo skeleton

`cargo init --name lunchbox` (git already initialized). `Cargo.toml`: `edition = "2024"`; dependencies (current stable at install time; `Cargo.lock` records exact versions): `clap` 4 with `derive` feature, `serde` 1 with `derive`, `toml`, `anyhow`, `serde_json`, `sha2`, `hex`, `fs4` (flock), `time` 0.3 with `formatting` + `macros` (ISO-8601 UTC timestamps), `signal-hook` 0.3 (SIGINT/SIGTERM during `--wait`). Dev-dependencies: `tempfile`, `assert_cmd`, `predicates`. No `config` crate: the two-layer merge needs custom list algebra (union/intersect/prepend) that `config`'s layering does not express; `toml`+`serde`+a hand-rolled merge is smaller — DESIGN §21's `config`-crate pin applied to TUI settings pages, not core.

Module layout (DESIGN §21): `src/main.rs`, `src/config/mod.rs`, `src/library/mod.rs`, `src/hash/mod.rs`, `src/resolve/mod.rs`, `src/mount/mod.rs`, `src/tokens/mod.rs`, `src/run/mod.rs`, `src/adapter/mod.rs`, `src/adapter/none.rs`, `src/adapter/pi.rs`.

No code comments anywhere (repo rule, AGENTS.md constraint 6).

### Step 2 — testdata pantry

Create exactly:

`testdata/skills/demo-review/SKILL.md`:
```markdown
---
name: demo-review
description: Review staged changes for defects and risks.
---

Demo review skill. Read the staged diff, list defects and risks found,
and suggest the smallest safe fix for each.
```

`testdata/skills/demo-scan/SKILL.md`:
```markdown
---
name: demo-scan
description: Scan for leaked secrets in the worktree.
---

Demo scan skill. Walk the worktree, flag likely secrets
(keys, tokens, passwords), and report file plus line.
```

Token constants these fixtures pin (estimator in Step 7): demo-review = 14, demo-scan = 13, union = 27. No `scripts/` dirs this phase.

### Step 3 — config module (`src/config/`)

```rust
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub library_paths: Vec<PathBuf>,   // default []
    pub default_adapter: String,       // default "pi"
    pub mount_mode: MountMode,         // Symlink | Copy; default Symlink
    pub scan_command: String,          // default ""
    pub allow: Vec<String>,            // default []
    pub deny: Vec<String>,             // default []
    pub max_menu_tokens: u64,          // default 2000
    pub fail_on_budget: bool,          // default false
    pub runs_dir: PathBuf,             // default "~/.lunchbox/runs"
}
```

Load: global `$HOME/.lunchbox/config.toml` (optional), then project `./lunchbox.toml` (optional). A present-but-unparseable file, or an unknown key in either layer (deny_unknown_fields), is a hard error — fail closed. All-absent → defaults.

Merge algebra (decides DESIGN §24's open item):
- Scalars (including `runs_dir`, `default_adapter`, `mount_mode`, budget fields, `scan_command`): project wins.
- `deny`: union, deduped, global order then project order.
- `allow`: both non-empty → intersection (global order); exactly one non-empty → that one; both empty → empty (= allow any).
- `library_paths`: project entries prepended to global (project shadows via first-hit resolution).
- CLI `--library` flags prepend ahead of the merged list.

`~` prefix expands to `$HOME`. Relative `library_paths` resolve against CWD. Tests set `HOME` to a `tempfile::tempdir` (guard with a `std::sync::Mutex` so env-mutating tests serialize).

### Step 4 — hash module (`src/hash/`)

`pub fn hash_tree(root: &Path) -> anyhow::Result<String>`:

Walk files recursively; sort by relative path (POSIX `/` separators). Skip any path containing `.git/`, `__pycache__/`, a `.DS_Store` or `*.pyc` component, or a `.skill_metadata.json` file. Per file: SHA-256 of raw bytes (`fs::read`, follows symlinks). Final digest: SHA-256 over the concatenation of `"{relpath}\0{hexdigest}\n"` lines in sorted order. Return `"sha256:<64 lowercase hex>"`.

Golden test (independent constant, computed from the spec): package with `SKILL.md` = `hello golden\n` and `refs/a.txt` = `abc` (no trailing newline) must yield exactly `sha256:6cffec6f66e39626ec7618746bc538791aa2f3e38e0df079b0f825d1f3c15b0c` (per-file digests `3585ff92a190a8dc23da1c2dde1cdd4662d0e0b15ae68ad04e635d9097425bd0`, `ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad`).

### Step 5 — skill identity + library reader (`src/library/`)

Frontmatter parse: if `SKILL.md` line 1 is `---`, collect to the next standalone `---`; extract only top-level (column 0) `name:` and `description:` single-line scalars, stripping surrounding quotes. `name` absent → directory name. No frontmatter → directory name, empty description. Missing `SKILL.md` → invalid package.

`pub fn scan_roots(roots: &[PathBuf]) -> anyhow::Result<Vec<FoundSkill>>` where `FoundSkill { name, source: PathBuf }`: list valid packages per root; **two packages with the same name inside one root → error** (resolver may still disambiguate when the pin carries a hash). Lookup honors ordered roots, first hit wins; missing name → error listing every root searched.

### Step 6 — resolver (`src/resolve/`)

Pin forms: `name`; `name@sha256:<64hex>`; anything else with an `@` (tags) → error `tag pins are not supported yet; pin by hash`.

`pub fn resolve(pins: &[String], cfg: &Config, roots_extra: &[PathBuf]) -> anyhow::Result<Vec<Locked>>` executes DESIGN §10 in order: expand pins against the merged root list (first hit; same-name-in-one-root duplicates allowed only when the pin has a hash — pick the entry whose hash matches, none match → error); compute hash; pinned-hash mismatch → error; name on `deny` → error; `allow` non-empty and name absent → error; missing `SKILL.md` → error; `scan_command` non-empty → error `scan_command is configured but not supported in this build` (hook arrives in a later phase; fail closed rather than silently skip). Budget: estimate tokens for the union menu; if `fail_on_budget` and union > `max_menu_tokens` → error, else warn on stderr. On any error after run-dir creation begins: clean up, leave nothing (DESIGN §20.3).

### Step 7 — token estimator (`src/tokens/`)

`pub fn estimate(name: &str, description: &str) -> u64 = ceil(chars(name + "\n" + description) / 4)`. `without` estimate = same estimator over the union of skills found in the adapter's discovery dirs (Step 10 `skill_dirs()`), tolerating absent dirs (→ 0 skills, printed with a note). `doctor` and `start` print `menu_tokens this run` and `without`.

### Step 8 — mount + run state (`src/mount/`, `src/run/`)

Run id: `lbx_<YYYYMMDD>_<HHMMSS>_<4 lowercase hex>` (UTC; hex from 2 bytes of `/dev/urandom`). Run dir: `<runs_dir>/<run_id>/` containing `manifest.toml`, `lunchbox.lock`, `workdir/`, `audit.jsonl`, `pid` (only when a child is spawned). Schemas exactly per DESIGN §9: manifest gets `schema = 1`, `run_id`, `task`, `adapter`, `created_at` (ISO-8601 UTC, e.g. `2026-09-06T15:30:12Z`), `[budget]`, `[[workers]]` with the single implicit worker `name = "default"`, plus `harness_argv` (array of strings; stored, spawned only by `adapter = pi`). Lock gets per-skill `name/source/hash/scan/description_tokens/workers = ["default"]`.

Mount: symlink each locked package dir to `workdir/<name>`. Any symlink failure → whole run switches to copy mode (uniform): walk the package, mkdir dirs, copy files; a symlink whose target resolves outside the package root → error `symlink escape`. `mount_mode` recorded in the lock reflects what ran. Never link anything beyond the locked set; no global "current" symlink.

Teardown (`finish`/`abort`): take exclusive `flock` on `runs/<id>/.lock` (fs4; contention → error `another lunchbox command is operating on this run`); remove `workdir/` (and `agents/` if present); append audit `unmounted`; write `result.json` per DESIGN §18 (`run_id`, `outcome` ok|aborted, `menu_tokens`, `without_menu_tokens`, `skills[{name,hash}]`, `unmounted: true`). Both are idempotent (second call exits 0, no-op). `finish` on a run whose pid is alive → error `run is still running; abort first`. `abort` SIGTERMs a live child, waits 5 s, SIGKILLs, then tears down with `outcome = aborted`.

Audit events (JSONL, ISO-8601 UTC `ts`): `resolved` (skills+hashes), `mounted` (mode, workdir), `spawn` (adapter, full argv), `unmounted` (reason: finish|abort|crash-cleanup).

### Step 9 — lifecycle CLI (`src/main.rs`)

clap derive subcommands: `doctor`, `start`, `status`, `finish`, `abort`, `gc`, `why`, `adapters`. `--json` on `doctor`, `start`, `status`, `why`.

`start` flags: `--task <str>`, `--skill <pin>` (repeatable), `--adapter <none|pi>` (default from config), `--library <path>` (repeatable, prepends), `--wait`/`--no-wait` (`--wait` default when a harness argv is present), `--keep`, `--dry-run`, then trailing `-- <argv…>` (collected with `allow_hyphen_values`; if the first collected token is `--`, drop it — this keeps DESIGN §13's documented `-- -- pi …` form working). `--from <manifest.toml>` → error `manifest-driven multi-worker runs arrive with Path B` (scope line: multi-worker is a later phase).

`start` flow: resolve → write manifest+lock → mount → print human summary (run id, workdir, `skill@sha256:…` list, `menu_tokens`/`without`, isolation line, finish hint). `adapter none`: never spawns; `--` argv is stored in the manifest only; summary notes `adapter none — mounted, not spawned`. `adapter pi`: build argv (Step 10), audit `spawn`, spawn child, write pid, `--wait` (default) → wait; on exit run `finish` unless `--keep`; exit with the child's exit code. SIGINT/SIGTERM while waiting (signal-hook): forward SIGTERM to child, wait, run `abort`, exit 130/143. `--dry-run`: mount, print the exact argv one token per line shell-quoted, never spawn, print `run lunchbox finish <id>`.

`status [run_id]` (default: most recent = lexicographically greatest run dir): `mounted` | `running` (pid file exists and pid alive) | `finished` | `aborted` | `leaked` (workdir present, pid dead/absent, no result.json). `gc`: delete run dirs older than 24 h not `running`, plus any `leaked`; print each removed path. `why [run_id]`: five lines — run id + task, worker packs, token delta (this run vs without), unmounted yes/no, outcome.

### Step 10 — adapters (`src/adapter/`)

Trait per DESIGN §19: `detect()`, `skill_dirs()`, `isolation_argv(workdir, skills, user_argv)`, `agent_dir_hint()`, `write_run_agents()`, `selftest()`, `explain()`.

`none`: `isolation_argv` → Err (never spawns); `skill_dirs` → configured library roots.

`pi`: binary `pi` resolved via `PATH`. `detect()` runs `pi --version`, parses the semver (verified shape: `0.84.4`). `skill_dirs()` → `["./.agents/skills", "~/.agents/skills"]` plus `~/.pi/agent/skills` only if it exists; all may be absent (this machine: all absent). `isolation_argv` → `["pi", "--no-skills", "--skill", "<workdir>/<skill1>", …, "--skill", "<workdir>/<skillN>", <user_argv…>]` — one `--skill` per package (help text reads per-skill "file or directory"); flags only, no settings overlay, never merges `~/.claude/skills`/`~/.agents/skills`. `agent_dir_hint()` → `~/.pi/agent` (documented never-written). `write_run_agents()` → Err `Path B not implemented in this build`. `selftest()`: pi absent → skip (exit 0, message `pi not found; selftest skipped`); pi present but `pi --help` lacks `--no-skills` or `--skill <path>` → fail loudly `pi isolation flags drifted`.

`adapters [--explain]`: lists `none`, `pi`; `--explain` prints each adapter's flag belief, noting `verified against pi --help 0.84.4 on 2026-09-06; selftest re-verifies`. `omp` anywhere → error `omp adapter arrives after Phase 1`.

### Step 11 — doctor

Read-only, never mounts. Detects pi (`--adapter` overrides config default). Prints: adapter + version; each `skill_dirs()` entry with exists/absent; skill count per dir; `menu_tokens` union estimate (the without-figure); three fattest descriptions (name + tokens); duplicates by name across dirs. `--json` mirrors the same fields.

### Step 12 — tests

`cargo test` must pass with no pi/omp installed (selftest-skip path also covered): hasher golden (Step 4 constant); config merge table (project-wins / deny-union / allow-intersect / library-prepend / unknown-key error); resolver gates (deny, allow-miss, hash mismatch, missing, tag error, scan_command error); mount symlink + forced-copy + symlink-escape error + teardown + idempotent finish + partial-failure cleanup; lifecycle under temp `HOME`; adapter argv construction exact-vector for two skills (mocked, no pi); doctor on a fake tree; `assert_cmd` integration for the full feature-001 loop and feature-002 `--json` numbers (14 / 13 / 27).

### Step 13 — README + bookkeeping + final commit

README per DESIGN §25: one-liner ("Your agent only sees these Skills for this job. Then they vanish."), install (`cargo install --locked --git <repo>`), `doctor` → `start`/`finish` quickstart using `--library testdata/skills`, `menu_tokens` before/after, "this is not a skill manager — point `--library` at Kitter / Skills Manager / `~/.agents/skills`". No architecture up front.

Bookkeeping (repo contract): flip `passes` to `true` in `docs/feature-list.json` for 001/002/007 only after their steps pass; update `PROGRESS.md` (state, verification results, next move); move `docs/exec-plans/active/phase-1.md` to `completed/`. Final commit: `git add -A && git commit -m "Phase 1: core MVP + Pi Path A adapter"`.

## Critical files & anchors

- `docs/design-docs/DESIGN.md` §7–§20 — the spec being implemented (config, hash, pins, lock, resolver order, mount, tokens, lifecycle, audit, adapter surface, guarantees).
- `docs/feature-list.json` — features 001/002/007: the acceptance steps; flip `passes` only on passing.
- `src/run/mod.rs` + `src/mount/mod.rs` — teardown correctness is DESIGN §20 guarantee 1; flock lives here.
- `src/adapter/pi.rs` — the exact isolation argv literal; fail-closed on flag drift.
- `AGENTS.md` — repo constraints: feature-at-a-time workflow, no code comments, clean cutover.

## Verification

Working dir: repo root; `~/.cargo/env` sourced. All commands run from a clean tree.

1. `cargo test` — green, no pi required (adapter tests are mocked).
2. Feature-001 loop (exact):
   - `cargo run -- start --library testdata/skills --skill demo-review --skill demo-scan --adapter none` → exit 0; summary prints run id, both `@sha256:` pins, `menu_tokens    this run: 27`; `ls ~/.lunchbox/runs/<id>/workdir` shows exactly `demo-review  demo-scan`.
   - `cargo run -- finish` → exit 0; workdir gone; `jq -r .unmounted ~/.lunchbox/runs/<id>/result.json` → `true`; `cargo run -- finish` again → exit 0 (idempotent).
3. Deny gate: in a temp dir with `lunchbox.toml` containing `deny = ["demo-scan"]`, `cargo run -- start --library <repo>/testdata/skills --skill demo-scan --adapter none` → non-zero, message names the denial, and no new run dir exists under `runs_dir` (test with `HOME` pointed at a tempdir).
4. Feature-002: `HOME=<fixture with .agents/skills containing the two demo packages> cargo run -- doctor --json` → skill count 2, union tokens 27, per-skill 14/13; no run dir created.
5. Feature-007 (pi present on this machine): `cargo run -- start --library testdata/skills --skill demo-review --adapter pi --wait -- -- pi --list-models` → exit 0; `jq -r '.event, .argv'` over the run's `audit.jsonl` shows a `spawn` event whose argv starts `pi --no-skills --skill <workdir>/demo-review`; after exit the workdir is gone and `result.json` has `outcome: "ok"`, `unmounted: true` (auto-finish).
6. `cargo run -- start --library testdata/skills --skill demo-review --adapter pi --dry-run -- -- pi --list-models` → prints the argv, mounts, spawns nothing; `finish` cleans up.
7. `cargo run -- adapters` → pi selftest ok; `adapters --explain` prints the flag belief.
8. Crash-teardown spot check: `start --adapter none`, then `gc` after `touch -d '2 days ago'` the run dir → dir removed, printed.

## Assumptions & contingencies

- **rustup via official script** (apt's rustc is too old for edition 2024). User may substitute a distro toolchain ≥ 1.85.
- **`--skill` per-package**: pi help reads "Load a skill file or directory … (can be used multiple times)" as one skill per flag. Contingency if the spawned pi errors or loads nothing: switch `isolation_argv` to a single `--skill <workdir>` (pantry-root reading), keep per-package as the documented default otherwise.
- **Merge algebra** (allow-intersect, library-prepend, deny-union) and the `-- --` strip rule are spec decisions made here per DESIGN §24's "specify at config design"; user may override before implementation starts.
- **Crate fallbacks**: `fs4` → `nix::fcntl::flock`; `time` → `jiff`. Only if the primary fails to build.
- **`default_adapter = "pi"` with only `none`/`pi` registered**: fresh-config `start` without `--adapter` uses pi; an `--adapter omp` errors `omp adapter arrives after Phase 1` — fail closed, never silently default-scan.
- **No default skill dirs exist on this machine** → `without` prints 0 with an explanatory note; that is correct behavior, not an error.
- **pi version drift** → selftest fails loudly; fix is updating the adapter's pinned flag set, never skipping the check.
- Commit policy is user-mandated: foundation commit first, phase commit last; no pushes.
