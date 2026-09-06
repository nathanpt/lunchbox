# Lunchbox TUI milestone — features 003, 004, 005, 006

Spec source: `docs/design-docs/DESIGN.md` §21–§22 (TUI framework pins, four screens, `--json` twins, headless snapshot tests), §7 (config layers). Acceptance: `docs/feature-list.json` feature-003/004/005/006 steps. Repo state at start: Phase 1 + simplify pass complete, `main` at `d59bde9`; core CLI (`doctor/start/status/finish/abort/gc/why/adapters`) green (67 unit + 15 integration tests); Rust 1.98.1 via `~/.cargo/env`; no TUI code exists.

## Context

Build the four v1 TUI screens (doctor, token-cost preview, skill picker, policy review) as Ratatui surfaces behind cargo features, each with a `--json` twin and headless snapshot tests, completing features 003–006 so feature-008 (v1 exit bar) needs only the human walkthrough. Screens are read/act views over the existing core modules — no new core behavior except: format-preserving edits of config layer files (policy screen), and an in-process mount/unmount path reusable by the picker (refactor of `cmd_start`, no behavior change).

## Approach

Execute steps in order. Steps 3–6 complete one feature each, in priority order (AGENTS.md feature-at-a-time); flip each feature's `passes` only after its Verification section loop passes.

### Step 1 — ADR-0002 + Cargo features and dependencies

1. Write `docs/decisions/0002-tui-dependencies.md`: adopt `ratatui 0.30` + `crossterm 0.29` (DESIGN §21 pins), `color-eyre` for panic-time terminal restore in the TUI path, `toml_edit` for format-preserving edits of user-authored `lunchbox.toml` / `~/.lunchbox/config.toml` (policy screen); `insta` as dev-dependency for buffer snapshots. Record two deviations from DESIGN §21's pin list: the `config` crate stays excluded (Phase 1 already hand-rolled the two-layer merge algebra it cannot express; `toml_edit` covers the editing need without a second config representation), and the `cargo generate ratatui/templates` component template is not used (four small screens; template scaffolding outweighs it). Supersedes nothing (ADR-0001 untouched).
2. `Cargo.toml`:
```toml
[features]
default = ["tui-doctor", "tui-menu"]
tui-doctor = ["dep:ratatui", "dep:crossterm", "dep:color-eyre"]
tui-menu = ["dep:ratatui", "dep:crossterm", "dep:color-eyre", "dep:toml_edit"]

[dependencies]
color-eyre = { version = "0.6", optional = true }
crossterm = { version = "0.29", optional = true }
ratatui = { version = "0.30", optional = true }
toml_edit = { version = "0.23", optional = true }

[dev-dependencies]
insta = "1"
```
   Versions are "current stable at install time; `Cargo.lock` records exact" (same policy as Phase 1; contingency below covers drift).
3. Verify both build configurations compile and the suite passes in each: `cargo test` and `cargo test --no-default-features` (CLI-only: no `src/tui` code compiled, all 82 existing tests still pass).

### Step 2 — `tui` subcommand skeleton + data functions + JSON twins (no rendering yet)

1. `src/main.rs`: add clap subcommand (unconditionally, so flag parsing is identical in every build):
```rust
Tui {
    #[command(subcommand)]
    screen: TuiScreen,
}

enum TuiScreen {
    Doctor { #[arg(long)] adapter: Option<String>, #[arg(long)] json: bool },
    Preview {
        #[arg(long = "skill", value_name = "PIN")] skills: Vec<String>,
        #[arg(long = "library", value_name = "PATH")] libraries: Vec<String>,
        #[arg(long)] adapter: Option<String>,
        #[arg(long)] json: bool,
    },
    Picker { #[arg(long = "library", value_name = "PATH")] libraries: Vec<String>, #[arg(long)] json: bool },
    Policy { #[arg(long)] json: bool },
}
```
   Handler routing: when neither feature is compiled in, every `tui` invocation exits non-zero with `this build has no TUI screens; rebuild with default features or --features tui-doctor,tui-menu (plain CLI equivalents: lunchbox doctor, lunchbox start)` — fail closed, never a silent no-op. When a screen's feature is off but the other is on, same error naming the missing feature.
2. Extract the doctor data builder out of `cmd_doctor` so CLI and TUI share one source: new `pub struct DoctorReport` in `src/adapter/mod.rs` holding `{ adapter: String, adapter_version: Option<String>, dirs: Vec<DirReport>, menu_tokens: u64, union: Vec<FoundDirSkill> }` plus `pub fn doctor_report(adapter: &dyn Adapter, cfg: &Config) -> Result<DoctorReport>` (wraps existing `scan_dirs` + `union_from` + `detect`; no equivalent exists). `cmd_doctor` serializes/render from this struct — its stdout (human and `--json`) must be byte-identical to today (guarded by a test comparing against golden output captured before the refactor).
3. Token preview estimation, new in `src/tokens/mod.rs` (no equivalent exists):
```rust
pub struct Preview {
    pub skills: Vec<PreviewSkill>,        // PreviewSkill { name: String, tokens: u64 }
    pub menu_tokens: u64,                 // sum of skills[].tokens
    pub without_menu_tokens: u64,         // adapter::union_menu over cfg-default or --adapter
    pub max_menu_tokens: u64,             // cfg value
    pub over_budget: bool,                // menu_tokens > max_menu_tokens
}
pub fn preview(cfg: &Config, roots_extra: &[PathBuf], pins: &[String]) -> Result<Preview>
```
   `preview` resolves pins by name only (first-hit over `cfg.search_roots(roots_extra)` using `library::find_in_root`; a pin with `@` is rejected with the existing tag-pin error; unknown name → the existing not-found error listing roots), estimates via `estimate(name, description)` from the `FoundSkill.description` the scan already carries. It applies no deny/allow/budget gates (estimates only — gates remain `start`'s job).
4. Config layer exposure in `src/config/mod.rs`: make `Layer` and its fields `pub`, add `Debug, PartialEq` to its derives, make `read_layer` `pub`, add `pub fn global_path() -> PathBuf` (`$HOME/.lunchbox/config.toml`) and `pub fn project_path() -> PathBuf` (`./lunchbox.toml`).
5. `tui <screen> --json` twins (implemented in `src/main.rs`, feature-gated identically to the screens; they run the data functions and print one JSON object):
   - `tui doctor --json` → exactly the `doctor --json` object (serialize `DoctorReport` the same way `cmd_doctor` does).
   - `tui preview --json --skill a --skill b [--library …] [--adapter …]` → `Preview` serialized: `{"skills":[{"name","tokens"}],"menu_tokens","without_menu_tokens","max_menu_tokens","over_budget"}`.
   - `tui picker --json [--library …]` → `{"library":[{"name","tokens","source"}]}` (all valid packages across `cfg.search_roots(libraries)`, deduped by name, first root wins — reuse `library::scan_root` per root + `tokens::estimate`).
   - `tui policy --json` → `{"global":{"path","exists","allow","deny"},"project":{"path","exists","allow","deny"},"effective":{"allow","deny"}}` where global/project come from `read_layer` (missing file → `exists:false`, empty lists) and `effective` from `Config::load()` merged values.
6. Tests (this step, `src/` unit + `tests/cli.rs`): golden JSON for each twin against the fake tree (both demo packages under `$HOME/.agents/skills` → doctor/picker counts 2, preview `menu_tokens 27` for both skills, `without_menu_tokens 27` when the adapter dirs are that same tree); feature-off fail-closed message asserted with `cargo run --no-default-features` equivalent via a second `assert_cmd` invocation using `Command::cargo_bin` built from the no-default-features profile — if two profiles in one test run prove impractical, assert the error string via a `#[cfg(not(any(feature = ...)))]` unit test on the router function instead (pick the unit-test route as default).

### Step 3 — Terminal scaffolding + doctor screen (feature-003)

1. `src/tui/mod.rs`, gated in `src/main.rs` by `#[cfg(any(feature = "tui-doctor", feature = "tui-menu"))] mod tui;`. Internally: `pub mod terminal;` (shared) + `#[cfg(feature = "tui-doctor")] pub mod doctor; pub mod preview;` + `#[cfg(feature = "tui-menu")] pub mod picker; pub mod policy;`.
2. `src/tui/terminal.rs`: `pub fn install() -> Result<()>` (idempotent `color_eyre::install()`, `crossterm::terminal::enable_raw_mode`, `EnterAlternateScreen`), `pub struct Restore;` implementing `Drop` (`DisableMouseCapture`-free: just `LeaveAlternateScreen` + `disable_raw_mode` + `ratatui::restore()`), `pub fn events() -> crossterm::event::Event` blocking read wrapper. If `ratatui::init()`/`ratatui::restore()` do not exist in the resolved ratatui version, use `Terminal::new(CrosstermBackend::new(io::stdout()))` and drop the `ratatui::restore()` call — the `Restore` guard already leaves the screen correctly.
3. Screen pattern (every screen file, so tests never touch a terminal): a `pub struct <Name>State` (data + cursor/scroll/selection fields, `Default`-able), `pub fn render(state: &mut State, frame: &mut ratatui::Frame, area: ratatui::layout::Rect)` (pure rendering, 16-color `ratatui::Style` only — no truecolor, so no `COLORTERM` branching is needed), and `pub fn handle_event(state: &mut State, event: &crossterm::event::Event) -> Action` where `enum Action { Continue, Quit, StartRun, FinishRun, … }` per screen. A per-screen `pub fn run(state) -> Result<()>` loop owns the terminal: install → `Restore` guard → loop { draw; `events()`; `handle_event` } — `run` is the only non-test caller of the terminal helpers.
4. `src/tui/snap.rs` (`#[cfg(test)]`): `pub fn frame_to_string(buffer: &ratatui::buffer::Buffer, area: Rect) -> String` — one output line per buffer row, cells joined, trailing spaces trimmed per line. All snapshot tests: build state → render onto `ratatui::backend::TestBackend::new(w, h)` → `insta::assert_snapshot!(frame_to_string(...))`. Commit generated `.snap` files under `src/tui/snapshots/` (insta default location for module tests).
5. Doctor screen (`src/tui/doctor.rs`): state carries `DoctorReport` + `scroll: usize`. Layout: title line `lunchbox doctor — <adapter> [<version>]`, then one line per `DirReport` (`<dir>  <N skills|absent>`), a blank line, `menu_tokens  <N>  (<K> skills union)`, then up to 10 lines of the union list (name + tokens) with the 3 fattest marked `*`. Keymap: `Up`/`Down` (and `k`/`j`) scroll the union list, `Home`/`End` jump, `q`/`Esc` quit. Data parity is structural: the state is built from the same `doctor_report` call the `--json` twin uses, so the snapshot plus a unit assertion `state.report.menu_tokens == twin_json["menu_tokens"]` proves step "displayed counts/tokens equal the doctor --json output".
6. Feature-003 verification loop (below) → flip `feature-003` `passes` to `true`.

### Step 4 — Token-cost preview screen (feature-004)

`src/tui/preview.rs`: state = library listing (from the same scan as the picker twin: name, tokens, selected flag), cursor index, `without_menu_tokens`, `max_menu_tokens`. Layout: left column = library list (`▸` cursor, `[x]`/`[ ]` selection), right column = `this run  <sum of selected>`, `without   <N>`, `delta     <without − this run>`, `budget    max <C>  <over_budget? "OVER" : "ok">`. Keymap: `Up`/`Down`/`k`/`j` move cursor, `Space` toggles selection (menu sum recomputed from state), `a` select-all, `n` select-none, `q`/`Esc` quit. Snapshot: two demo skills both selected → `this run 27`. Unit parity test: render → parse nothing; instead assert `state.selected_tokens() == tokens::preview(cfg, &[], &["demo-review","demo-scan"]).unwrap().menu_tokens`. Flip `feature-004` after its loop.

### Step 5 — `prepare_run` refactor + picker screen (feature-005)

1. Refactor `src/main.rs` (no behavior change; full suite must stay green before proceeding): extract from `cmd_start`/`start_run` the mount core:
```rust
pub struct PreparedRun {
    pub run_id: String,
    pub run_dir: PathBuf,
    pub workdir: PathBuf,
    pub locked: Vec<resolve::Locked>,
    pub mount_mode: config::MountMode,
    pub menu_tokens: u64,
    pub without_tokens: u64,
}
pub fn prepare_run(cfg: &Config, adapter_name: &str, task: &str, pins: &[String],
                   libraries: &[PathBuf], harness_argv: &[String]) -> Result<PreparedRun>
```
   doing: `resolve::resolve` → `run::new_run_id` → create run dir → `run::build_manifest` + write → `mount::mount` → `run::build_lock` + write → `adapter::union_menu` → audit `resolved` + `mounted`. `cmd_start` becomes: `prepare_run` → print summary → spawn/wait path unchanged (still registering `Signals` before `prepare_run` so the post-mount checkpoint keeps working). `start --json` output and audit contents must be byte-identical (assert via the existing integration tests, which already pin these).
2. `src/tui/picker.rs`: state = library listing (name, tokens, selected, source), cursor, `Option<PreparedRun>` (current run), status line. Keymap: `Up`/`Down`/`k`/`j` move, `Space` toggle, `s` start (calls `prepare_run(cfg, "none", "tui picker run", &selected_names, libraries, &[])`; on success the status line shows `run <id> mounted (<mode>)` and the union list is replaced by the run view: `skill@hash` per line), `f` finish (calls `run::teardown(&run.run_dir, run::Outcome::Ok, cfg)`; status `unmounted — result.json written`; `f` with no live run is a no-op status hint), `q`/`Esc` quit (with a mounted run: quit implies `f` — never leak a mount). The picker always mounts with `adapter none`; spawning a harness stays CLI `start` territory (printed in the footer: `spawn: use lunchbox start --adapter pi …`). Reuses `prepare_run`/`teardown`; no equivalent existed.
3. Tests: state-machine test drives `handle_event` with synthesized events (select both demos → `s` → assert exactly one run dir exists under a temp `runs_dir` (config `runs_dir` override) with `workdir` containing exactly `demo-review`/`demo-scan` → `f` → workdir gone, `result.json` `unmounted: true` → `q`). Snapshot: library list with both selected + footer hints. Flip `feature-005` after its loop.

### Step 6 — Policy review screen (feature-006)

1. Persistence helper in `src/config/mod.rs` (no equivalent exists):
```rust
pub fn append_layer_entry(path: &Path, list: &str, entry: &str) -> Result<()>
```
   `list` ∈ `"allow" | "deny"` (anything else → error). Reads the file via `toml_edit::DocumentMut` (missing file → empty document), gets-or-inserts `doc[list]` as an array of strings, appends `entry` if absent (present → no-op success), writes the file back (creating parent directories — `~/.lunchbox/` may not exist yet). Unparseable file → hard error (fail closed, same as `read_layer`). Format, comments, and key order of the rest of the file are preserved by construction.
2. `src/tui/policy.rs`: state = both `LayerReport`s (`{path, exists, allow, deny}` — reuse the twin's loader), active layer (`Tab`/`Backtab` switches global↔project), active list (`l`/`r` or `Left`/`Right` switches allow↔deny), cursor, `input: String` + `input_mode: bool`, status line. Layout: two columns (global / project) each showing `path`, `exists`, `allow: [a, b]`, `deny: [c]`; the active layer/list highlighted; bottom line = input (`add entry: <text>`) when in input mode. Keymap: `Tab`/`Backtab` layer, `Left`/`Right` list, `Up`/`Down` cursor over entries, `a` enter input mode, `Enter` in input mode → `append_layer_entry(active_layer_path, active_list, text)` → reload state + status `wrote <path>`; `d` deletes nothing in this milestone (deletion is not in the feature steps — one line: adding only), `q`/`Esc` quit (Esc first exits input mode).
3. Tests: unit — temp `HOME` + temp cwd: create both layer files with unrelated keys and comments; drive the state machine to add `deny: demo-scan` on the project layer; assert `./lunchbox.toml` gained the entry with its comment and other keys intact, `~/.lunchbox/config.toml` byte-identical, and `resolve::resolve(&["demo-scan"], &Config::load(), …)` now fails with the deny message. Integration (`tests/cli.rs`): same scenario via `tui policy --json` before/after (effective deny list grows). Snapshot: both layers populated, project layer active. Flip `feature-006` after its loop.

### Step 7 — Full verification + bookkeeping

Run the Verification section end to end on a clean tree, then: flip `passes` for 003–006 in `docs/feature-list.json` (only those), update `PROGRESS.md` (state, verification table incl. both feature-config test runs, next move = feature-008 human walkthrough + Omp), update `AGENTS.md` current-phase line and `CHANGELOG.md` (repo contract; AGENTS.md dictates the rest), file this plan at `docs/exec-plans/active/tui-milestone.md` at start and move to `completed/` at end, commit `TUI milestone: doctor, preview, picker, policy (features 003-006)`.

## Critical files & anchors

- `src/main.rs` — `cmd_doctor` (extract `DoctorReport`), `cmd_start`/`start_run` (extract `prepare_run`; keep `Signals` registration before it), new `Tui`/`TuiScreen` clap arms + twin handlers.
- `src/adapter/mod.rs` — `scan_dirs`/`union_from`/`DirReport` (reuse for `DoctorReport`); `src/tokens/mod.rs` — `estimate` + new `preview`.
- `src/config/mod.rs` — `Layer` (make pub + derives), `read_layer` (pub), new `append_layer_entry`, `global_path`/`project_path`.
- `src/tui/` (new) — `terminal.rs`, `snap.rs`, `doctor.rs`, `preview.rs`, `picker.rs`, `policy.rs`; state/render/handle_event split is what makes the screens headless-testable.
- `docs/feature-list.json` — steps for 003–006 are the acceptance loops; flip `passes` only on passing.

## Verification

Working dir: repo root; `. "$HOME/.cargo/env"`. Fake tree = temp `HOME` containing `.agents/skills/{demo-review,demo-scan}` copied from `testdata/skills` (pattern used by `tests/cli.rs::feature_002_doctor_reports_tree_without_mounting`).

1. `cargo test` → all green including new snapshot/twin/state-machine tests (default features).
2. `cargo test --no-default-features` → green, no `src/tui` compiled; `cargo run --no-default-features -- tui doctor` → non-zero, message `this build has no TUI screens`.
3. Feature-003: `HOME=<fake> cargo run -- tui doctor --json > a.json; HOME=<fake> cargo run -- doctor --json > b.json; diff a.json b.json` → identical. Snapshot test `doctor_screen_two_skills` passes headless (CI needs no terminal; it is part of `cargo test`).
4. Feature-004: `HOME=<fake> cargo run -- tui preview --json --skill demo-review --skill demo-scan` → `"menu_tokens": 27`, `"without_menu_tokens": 27` (same tree), `"over_budget": false`; with `--skill demo-review` alone → 14. Snapshot `preview_screen_selected_pack` passes.
5. Feature-005: picker state-machine test (select both → `s` → exactly one run dir, workdir = exactly the two packages → `f` → workdir gone, `result.json.unmounted == true` → `q`) passes; snapshot `picker_screen_two_selected` passes.
6. Feature-006: policy unit/integration tests pass — project layer gains `deny = [… "demo-scan"]` with formatting preserved, global layer byte-identical, and `HOME=<fake> cargo run -- start --library testdata/skills --skill demo-scan --adapter none` (cwd = the policy test project) now fails with `denied by policy`.
7. Human smoke (not CI): `cargo run -- tui picker` against `--library testdata/skills` in a real terminal — select, start, finish, quit — matches the state-machine behavior.

## Assumptions & contingencies

- **Default features include both TUI features** (DESIGN §21's "TUI is a first-class v1 surface"); CLI-only builds opt out via `--no-default-features`. User may flip defaults before execution.
- **Picker mounts with `adapter none`** — satisfies every feature-005 step; spawning a harness inside the TUI stays out (footer points at `lunchbox start`). User may require in-TUI `pi` spawn instead (would need a suspend/pty design — flag before execution, not during).
- **Keymaps as specified** per screen (vi-style arrows both ways, `q` quit); trivially adjustable later without architectural impact.
- **`config` crate stays excluded; `toml_edit` adopted** — recorded in ADR-0002 with reasoning; user may veto `toml_edit` (fallback: full-file rewrite via the existing `Layer` serializer, losing comments in user-authored layer files — strictly worse, chosen only on veto).
- **Crate version drift**: if `ratatui 0.30`/`crossterm 0.29` are not co-installable as pinned, fall back to the newest co-installable ratatui/crossterm pair, using ratatui's `crossterm_0_28` compat feature if required (DESIGN §21 anticipates exactly this); if `ratatui::init`/`restore` helpers are absent, use the manual setup described in Step 3.2. If `toml_edit 0.23` is not current, any stable `toml_edit` ≥ 0.22 works (API used — `DocumentMut`, array-of-strings get-or-insert — is stable across that range).
- **Snapshot churn**: if insta snapshots differ only by trailing whitespace across environments, set `insta::Settings::new` with `filters` trimming trailing spaces per line in `snap.rs` — decide at first flake, not preemptively.
