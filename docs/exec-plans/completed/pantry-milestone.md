# Lunchbox pantry milestone — feature-014

Spec source: ADR-0006 (`docs/decisions/0006-thin-git-installer.md`, accepted
2026-09-08). Repo state at start: `main` at `8900e7c`, features 001–013 all
`passes: true`, 105 unit + 30 integration tests green default, 90 + 30 with
`--no-default-features`, zero warnings.

User choices (2026-09-08 grill-me): thin git installer (whole-repo
granularity); implicit managed root `~/.lunchbox/pantry` (no config writes,
user paths win first-wins); default branch + `git pull --ff-only`, no ref
pinning; auto-detect pantry root with `--path` escape hatch; shell out to
git; README intro leads with context cost, supported by blast radius.

## Context

v0.1.0 resolves skills only from paths the user already configured. The
acquisition step (clone a skills repo, keep it current) is manual. The
resolver's root list is `Config::search_roots` (src/config/mod.rs:184):
CLI `--library` → merged `library_paths` → `./.agents/skills` →
`~/.agents/skills`. Doctor reports the adapter's standing dirs only
(`Adapter::skill_dirs`) and drives the without-Lunchbox estimate from them.
Tests use temp `HOME`s and a mock-binary `PATH` (`path_with_mock_bin`,
`mock_harness` in tests/cli.rs) — no network, no real harnesses (DESIGN §21).

## Approach

Execute in order; flip `passes` only after the Verification loop passes.

### Step 1 — docs (done before implementation)

ADR-0006 (above); feature-014 entry appended failing (done); DESIGN
amendments in Step 6 ride with the closeout but the load-bearing ones
(search order, CLI list) land with the implementation commit.

### Step 2 — `src/pantry.rs` (new module)

- `pub fn pantry_home() -> Result<PathBuf>` — `$HOME/.lunchbox/pantry`;
  bail without HOME (mirrors `config::global_path`).
- `pub fn name_from_url(url: &str) -> Result<String>` — final path
  component, `.git` stripped; validated as exactly one normal path
  component (reject `..`, dot-prefixed, separators, empty).
- `fn dir_has_skill_children(dir: &Path) -> Result<bool>` — any non-dot
  child directory containing `SKILL.md`; read errors propagate.
- `pub fn detect_pantry_root(repo: &Path, path_override: Option<&str>) -> Result<PathBuf>`:
  override → `repo.join(sub)`, verified; else candidates = repo root
  (if skill children) + each non-dot first-level child dir with skill
  children; exactly one wins, zero/multiple bail naming the pantry,
  candidates, and the `--path` fix.
- `pub fn managed_pantries() -> Result<Vec<ManagedPantry>>` — pantry_home's
  child directories (sorted), each with `override_file` =
  `pantry_home/<name>.path` (one line) honored by detection.
- `pub fn resolve_roots() -> Result<Vec<PathBuf>>` — roots only.
- `pub fn add(url: &str, path: Option<&str>) -> Result<Added>` — bail if
  `<pantry_home>/<name>` exists; `git clone <url> <dest>` captured
  (failure → remove the partial dir, bail with exit code + stderr
  excerpt); detect (failure → remove clone, bail); write `<name>.path`
  only when `--path` given; `Added { name, root, skills }` with skills
  counted via `library::scan_root`.
- `pub fn update(name: Option<&str>) -> Result<Vec<String>>` — selected
  pantry or all; unknown name bails listing none; `git -C <repo> pull
  --ff-only` captured (failure bails naming pantry + exit + excerpt);
  re-detect after pull (failure bails with fix hint); returns updated
  names.

### Step 3 — wiring

- `src/config/mod.rs`: `search_roots` returns `Result`, inserts
  `crate::pantry::resolve_rots()?` after `library_paths`, before the
  `.agents/skills` defaults. Callers add `?`: `resolve::resolve`
  (already `Result`), `tokens::preview`, `tui_preview`/`tui_picker`
  paths in main.rs, `tui::picker` test (`.unwrap()` stays).
- `src/adapter/mod.rs`: `DoctorReport` gains `git_version:
  Option<String>` and `pantries: Vec<PantryReport>`; `PantryReport {
  name, repo, root: Option<PathBuf>, error: Option<String>, skills }`;
  `doctor_report` fills them (`detect_version("git")`,
  `pantry::managed_pantries()` with per-pantry errors captured, never
  fatal); `to_json` gains `"git"` and `"pantries"` keys (additive).
- `src/main.rs`: `CliCommand::Add { url, path }`, `CliCommand::Update {
  name }`; `cmd_add`/`cmd_update` (human output only — gc/finish
  precedent). `cmd_doctor` human output: `git` row after `adapter`
  (`git <v>` / `git (not found)`); `pantries:` section after the skill
  dirs block, omitted when empty; error lines for broken pantries.
- `src/tui/doctor.rs`: render the git row and pantries block; snapshot
  updates.

### Step 4 — tests

Unit (`src/pantry.rs` `mod tests`, `config::with_home` for HOME
isolation, fixture repos built with `fs`):

- `name_from_url`: plain/`.git`/trailing-slash URLs; rejects empty,
  dot-prefixed, `..`, separator-bearing names.
- detection: root-shaped repo → root; `skills/`-nested → subdir; root +
  `skills/` both populated → ambiguous error naming both; empty repo →
  no-skills error; `.git`-like dot dir ignored as candidate.
- `--path`: honored; nonexistent subdir → error.
- `resolve_roots` ordering: `Config::search_roots` with user
  `library_paths` + a managed pantry puts user roots first (config test).

Integration (`tests/cli.rs`, mock `git` on `PATH` — `mock_git` helper
next to `mock_harness`, dispatching on subcommand: `clone` materializes
a fixture tree, `-C <repo> pull --ff-only` exits 0 or simulates
divergence, `--version` prints a fixed semver):

- `add_registers_pantry_and_start_resolves`: nested fixture →
  `add https://example.com/you/agent-skills` succeeds (output: name,
  root under `~/.lunchbox/pantry`, skill count); `doctor --json`
  `pantries[0]` root + skills; `start --skill <fixture-skill> --adapter
  none --json` with no `--library` resolves; `finish` cleans;
  `without_menu_tokens` stays 0 (adapter none, no library paths).
- `add_root_shaped_and_path_override`: root-shaped fixture → root =
  repo; ambiguous fixture + `--path skills` → succeeds, `<name>.path`
  written, `update` keeps honoring it.
- `add_failures`: ambiguous fixture without `--path` → non-zero naming
  candidates, pantry dir absent; existing name → non-zero, first intact;
  clone failure (mock exits non-zero) → non-zero with exit + excerpt,
  nothing left.
- `update_reports_and_fails`: happy → `updated` line; unknown name →
  non-zero; divergence (mock pull exit 128 + stderr) → non-zero naming
  pantry + code + excerpt.
- `missing_git_fails_closed`: `PATH` = empty dir → `add` non-zero
  (`failed to run git`), `doctor` prints `git (not found)` and exits 0.
- `broken_pantry_fails_start_closed`: junk dir dropped into
  `~/.lunchbox/pantry` → `start --skill x --adapter none` non-zero
  naming the pantry; `doctor` exit 0 with the error in the pantries
  section.

Update pinned doctor outputs if the git row shifts anything (existing
assertions are `contains`-based; README snippet updated in Step 5).

### Step 5 — README

- Intro: context-cost lead (every skill's name+description rides the
  agent's menu every turn; fat menus tax every call), blast-radius
  support (a worker sees exactly the pinned pack), audience line (you
  already run Pi/Omp and keep Skills).
- New "Adding skills" section: pantry shape (`<name>/SKILL.md`),
  `lunchbox add <git-url>` (auto-detect rule in one sentence, `--path`
  escape), `lunchbox update`, manual paths (`--library`,
  `library_paths` config, `.agents/skills` defaults), ordering note
  (your configured paths win over managed pantries).
- Rewrite "This is not a skill manager": lunchbox clones whole skill
  repos and mounts per-run subsets; git is the version system; per-skill
  install/version/edit stays with Kitter / Skills Manager /
  `~/.agents/skills` + `--library`.
- Doctor snippet gains the `git` row (captured from the real binary);
  add/update example output captured live against
  https://github.com/nathanpt/agent-skills and dated (external repo —
  counts can drift; feature-009's byte-exact bar applies to the
  clone-local testdata snippets only).
- gif-script: leave (explicitly abridged).

### Step 6 — closeout

DESIGN §3/§6 (resolution note: acquisition in scope, per-skill still
not), §7 (layout + search order), §16 (CLI list + `add`/`update`
sections), §25 (README posture note), §26 (defaults row);
ARCHITECTURE.md (command surface, `src/pantry.rs` bullet, search order,
doctor surface, pantry storage note); PROGRESS.md (state + verification
table); CHANGELOG.md; AGENTS.md phase line; flip `passes`; move this
plan to `docs/exec-plans/completed/`; commit.

## Critical files & anchors

- `src/config/mod.rs` — `search_roots` (L184-196) gains pantry roots +
  `Result`.
- `src/adapter/mod.rs` — `DoctorReport` (L144-163), `to_json`
  (L173-188), `detect_version` (L209).
- `src/main.rs` — `CliCommand` (L36-69), `main` dispatch (L130-140),
  `cmd_doctor` human block (L157-202).
- `src/tui/doctor.rs` — render (L33-87) + snapshots.
- `tests/cli.rs` — `mock_harness` (L1170), `path_with_mock_bin`
  (L1153), doctor tests (L345, L1482).
- New: `src/pantry.rs`, `docs/decisions/0006-thin-git-installer.md`.

## Verification

Binary `target/debug/lunchbox`; fake tree = temp HOME; network live
checks last.

1. `cargo test` and `cargo test --no-default-features` → green, zero
   warnings (`cargo build` clean both ways).
2. Mock-PATH checks per Step 4 pass (they are the suite).
3. Live acquisition: `HOME=<fake> lunchbox add
   https://github.com/nathanpt/agent-skills` → flagless success,
   `skills/` auto-detected; `doctor` lists the pantry + git row;
   `start --skill grill-me --adapter none` (no `--library`) mounts;
   `finish` cleans; `lunchbox update` fast-forwards; delete dir →
   doctor back to no-section output.
4. No drift: with an empty pantry dir, `doctor`, `adapters`,
   `tui preview --json`, and README-shape `start` outputs differ only by
   the `git` row (byte-compare against the pre-change binary where
   pinned).
5. README: every clone-local snippet byte-verified (temp HOME, real
   binary); the add/update example verified live and dated.

## Assumptions & contingencies

- `git` exists on dev/CI machines for the mock-git tests (mocked on
  PATH; the suite never touches the network).
- `git clone` into a fresh dir leaves nothing on failure — belt: `add`
  removes the dir on any failure path.
- Full clone (not `--depth 1`): skills repos are small; shallow adds
  ff-only edge cases. Recorded in ADR-0006.
- Ambiguity across dot-dirs: only `.git` realistically appears; dot-dirs
  are skipped as candidates (not as skill dirs inside a detected root).
- If `search_roots`' new `Result` ripples further than the four call
  sites listed, stop and re-read before widening.
