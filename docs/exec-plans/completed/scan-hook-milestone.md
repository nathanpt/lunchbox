# Lunchbox scan-hook milestone — feature-013

Spec source: `docs/design-docs/DESIGN.md` §2 ("failed scan → do not mount"), §7 (`scan_command` config), §9 (lock `scan` field), §10 rule 6 (scan hook; non-zero exit → fail unless `--override-scan`, explicit and audited), §16 (`start` synopsis), §17 (policy-gate order + untrusted-SKILL.md threat), §18 (audit). Repo state at start: `main` at `f1543f8` (Path B milestone + simplify pass), features 001–012 all `passes: true`, 100 unit + 26 integration tests green in the default configuration, 85 + 26 with `--no-default-features`, zero warnings.

User choices (2026-09-06): **shell-string invocation** — the configured command runs as `sh -c '<scan_command> "$1"' sh <package-source>` so args, quoting, and redirections in the config string work naturally (config is user-authored); **lock vocabulary `pass` / `none` / `overridden`** — unscanned runs stop writing `scan = "pass"`.

## Context

The scan hook is the only configured-but-unimplemented gate. `resolve::resolve` bails `scan_command is configured but not supported in this build` (src/resolve/mod.rs:54-56) — honest fail-closed placeholder, so nobody can believe a scan ran. `build_lock` hardcodes `scan: "pass".to_string()` (src/run/mod.rs:266) — the lock field is scaffolding. DESIGN §2 already promises the guarantee; §3 scopes it ("a scan hook is enough", full SBOM is a non-goal). `resolve::resolve` has exactly one production caller (`prepare_run`, src/run/mod.rs:319); `tokens::preview` and `doctor` are gate-free by design and stay so.

## Approach

Execute steps in order; flip `passes` only after the Verification loop passes.

### Step 1 — feature contract entry

Append to `docs/feature-list.json` (starts `passes: false`; key order `id, category, description, steps, passes, priority, dependencies`):

- `feature-013`, category `functional`, priority 11, dependencies `["feature-001"]`, description "scan_command policy gate: a configured scanner runs per locked skill before mount, non-zero exit fails closed, --override-scan is explicit and audited, and the lock records the real per-skill outcome (DESIGN §10/§17)", steps:
  1. "A configured scan_command runs once per locked skill as `sh -c '<scan_command> \"$1\"' sh <package-source>` before mount; non-zero exit fails the start with the skill, command, and exit code named and no run dir left behind"
  2. "--override-scan proceeds past a failed scan, records scan = \"overridden\" for the offending skills, prints a warning, and the audit records both the failure and the override; the TUI picker cannot override"
  3. "The lock records scan = \"pass\" only when a scanner exited zero, \"none\" when scan_command is empty, and \"overridden\" after an override; scanner output is captured so start --json stays a single JSON line; doctor and tui preview never scan"

### Step 2 — ADR-0004 + DESIGN amendments

1. New `docs/decisions/0004-scan-hook-contract.md` (MADR format, status accepted, date 2026-09-06) recording: shell-string invocation with exactly one appended quoted path argument (user-authored config; script-free ergonomics beat argv-only purity); vocabulary `pass`/`none`/`overridden` and why the lock never contains `fail` (a failed scan without override aborts before the lock is written, DESIGN §20.3); scanning happens pre-mount on the pantry source, once per locked skill (union-deduped, so a skill shared by workers scans once); `--override-scan` is a CLI-only per-run flag — `prepare_run` callers other than `cmd_start` (the picker) pass `false`, so no surface can override except an explicit terminal invocation; scanner stdout/stderr is captured, not inherited (`start --json` must remain one parseable line; a bounded excerpt surfaces in the error text); no scanner timeout in v1 (the scanner is the user's own tool; a hang is observable and killable — recorded, revisit on a real report); fail-closed placement before run-dir creation so scan failures leave nothing behind.
2. DESIGN.md amendments (resolution-note style):
   - §7: `scan_command` comment notes the shell-string semantics (arguments and redirection allowed; the package source path is appended as the final quoted argument).
   - §9: lock `scan` vocabulary note — `pass` (scanner exited zero), `none` (no `scan_command` configured), `overridden` (scanner failed, `--override-scan` given); `fail` never appears because a failed scan without override aborts the run before the lock is written.
   - §10 rule 6: expand to the invocation contract (once per locked skill, pre-mount, `sh -c '<scan_command> \"$1\"' sh <source>`; non-zero → fail unless `--override-scan`, which is explicit and audited).
   - §16: `start` synopsis gains `[--override-scan]`.
   - §18: audit gains event `{"event":"scan","command":<cmd>,"skills":[{"name":<name>,"scan":<result>}],"override":<bool>}` written after the `resolved` event, only when `scan_command` is non-empty.

### Step 3 — implementation

1. `src/resolve/mod.rs`:
   - `Locked` gains `pub scan: String`.
   - New `fn scan_skill(command: &str, source: &Path) -> Result<bool>`: spawn `Command::new("sh").arg("-c").arg(format!("{command} \"$1\"")).arg("sh").arg(source).output()`; spawn/IO failure → bail `failed to run scan_command '<command>': <io error>`; `Ok(status.success())`.
   - `resolve(pins, cfg, roots_extra, override_scan: bool)` — drop the L54-56 bail. In the per-pin loop, after deny/allow pass and before pushing a new `Locked` (the existing duplicate-pin `continue` path already prevents re-scans), when `!cfg.scan_command.is_empty()`: `scan_skill` → success sets `scan: "pass"`; failure + `override_scan` sets `scan: "overridden"` and eprintlns `warning: skill '<name>' failed scan_command '<cmd>' (overridden by --override-scan)`; failure without override → bail `skill '<name>' failed scan_command '<cmd>' (exit <code>): <excerpt>` where excerpt is the captured stderr (first 500 chars, newlines collapsed to `; `), omitted entirely when stderr is empty. Empty `scan_command` → `scan: "none"`.
2. `src/run/mod.rs`:
   - `build_lock`: `scan: skill.scan.clone()`.
   - `prepare_run` gains `override_scan: bool`, threaded into `resolve`.
   - `mount_run`: after the `resolved` audit, when `!cfg.scan_command.is_empty()` append `{"event":"scan","command":cfg.scan_command,"skills":[{"name":l.name,"scan":l.scan}],"override":locked.iter().any(|l| l.scan == "overridden")}`.
3. `src/main.rs`: `StartArgs` gains `#[arg(long)] override_scan: bool`; `cmd_start` passes it to `prepare_run`. `src/tui/picker.rs` call site passes `false`.
4. Update `Locked` literal sites in unit tests (`locked_pair` in run tests, `locked_one` in mount tests, resolve tests) with `scan: "none"`.

### Step 4 — tests

- Unit (resolve; shell builtins as scanners — no script fixtures): `exit 0` → `locked[0].scan == "pass"`; `exit 3` without override → error contains the skill name, command, and `exit 3`; `exit 3` with override → Ok with `scan == "overridden"`; empty → `"none"`; `definitely-not-a-real-binary` → `failed to run scan_command`. Replace `scan_command_fails_closed` (L347-353).
- Integration (`tests/cli.rs`, temp HOME + project `lunchbox.toml`):
  - `scan_pass_invokes_scanner_and_locks`: `scan_command = "printf '%s' \"$1\" > <record-file>"` (redirection proves the shell-string choice); `start --library testdata/skills --skill demo-review --adapter none --json` → success; record file contains the pantry source path; lock `scan = "pass"`; audit `scan` event with command + per-skill results + `override: false`; stdout is one JSON line.
  - `scan_fail_fails_closed`: `scan_command = "echo findings >&2; exit 3"` → non-zero; stderr names skill + command + `exit 3` + the `findings` excerpt; `~/.lunchbox/runs` empty.
  - `scan_override_proceeds_and_audits`: same config + `--override-scan` → success; warning on stderr; lock `scan = "overridden"`; audit event `override: true`; `finish` cleans.
  - `scan_json_stays_pure`: `scan_command = "echo junk"` (scanner writes to stdout) → `start --json` stdout parses as exactly one JSON object (scanner output captured, not inherited).
  - Update lock-text pins: run-mod `lock_roundtrip_and_schema` asserts `scan = "none"` (was `pass`).
- Confirm `doctor`, `tui preview`, `adapters` paths never spawn a scanner (no code change needed — `tokens::preview` does not call `resolve`; note in PROGRESS).

### Step 5 — verification + bookkeeping

Run the Verification section end to end; flip `passes` for feature-013; update `PROGRESS.md` (state, verification table incl. both feature configs, ADR-0004 link), `AGENTS.md` phase line, `CHANGELOG.md` entry; sync `ARCHITECTURE.md` (resolve bullet: "scan hook refusal" → live scan gate; audit list gains the scan event); move this plan to `docs/exec-plans/completed/`; commit `Scan-command policy hook (feature-013)`.

## Critical files & anchors

- `src/resolve/mod.rs` — bail to delete (L54-56), `resolve()` signature, `Locked` (L8-14), `scan_command_fails_closed` (L347-353), budget tests' `Locked` literals via `pantry()`.
- `src/run/mod.rs` — `build_lock` hardcode (L266), `prepare_run` (L301-317), `mount_run` audit block, `locked_pair` test helper.
- `src/main.rs` — `StartArgs` (L101-125), `cmd_start` → `prepare_run` call.
- `src/tui/picker.rs` — `prepare_run` call site (L74-83) passes `false`.
- `src/mount/mod.rs` — `locked_one` test helper gains `scan`.
- `tests/cli.rs` — new scan tests; `mock_harness` pattern (L591+) for the recording scanner if needed.

## Verification

Working dir: repo root; `. "$HOME/.cargo/env"`; binary via `target/debug/lunchbox` (rustup needs the real HOME). Fake tree = temp HOME; project `lunchbox.toml` in cwd supplies `scan_command`.

1. `cargo test` and `cargo test --no-default-features` → both green, zero warnings.
2. Pass: `scan_command = "exit 0"` → `start --library testdata/skills --skill demo-review --adapter none --json` exit 0; lock `scan = "pass"`; audit `scan` event present with `override: false`; `finish` → `unmounted: true`.
3. Invocation shape: recording scanner (`printf '%s' \"$1\" > $TMP/record`) → record file holds the resolved pantry source path; a two-worker `--from` run with a shared skill records it once.
4. Fail closed: `exit 3` scanner → non-zero, stderr contains the skill name, command, `exit 3`, and any scanner stderr excerpt; `ls $T/.lunchbox/runs` empty. Spawn failure: nonexistent binary → `failed to run scan_command`.
5. Override: `--override-scan` → exit 0, stderr warning, lock `scan = "overridden"`, audit `override: true`.
6. Default: no `scan_command` anywhere → lock `scan = "none"`; `doctor`/`tui preview`/`adapters` outputs unchanged; README snippet byte-identical (README shows no scanner).
7. No drift: existing suites green with the vocabulary flip (`scan = "none"` in unscanned locks).

## Assumptions & contingencies

- `sh` exists on the host (already assumed by the symlink/mount code and mock harnesses).
- Unit tests use `exit N` shell builtins as scanners — no executable fixtures, no PATH dependence.
- Scanner stderr excerpt capped at 500 chars, newlines collapsed to `; ` — keeps error text single-line and bounded while surfacing the scanner's own finding text.
- No timeout: a hung scanner hangs `start` visibly; recorded in ADR-0004 as a deliberate v1 omission.
- `--override-scan` must appear before `--` (clap trailing-var-arg), same as every other start flag.
- If per-skill overrides are ever wanted (override one bad skill, keep the gate for the rest), that is a new, separate flag — out of scope; the ADR records the per-run-only decision.
- `--dry-run` scans (it mounts, so it gates); `--from` scans the union once regardless of worker sharing.
