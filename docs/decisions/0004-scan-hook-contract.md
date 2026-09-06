# 0004. Scan-hook invocation, lock vocabulary, and override semantics

- Status: accepted
- Date: 2026-09-06
- Supersedes: nothing (implements DESIGN §10 rule 6 and §17's "scan hook passed (if set)")

## Context and problem

DESIGN §7 has carried a `scan_command` config key since the design review,
§2 promises "failed scan → do not mount", and §10 rule 6 has been a one-line
placeholder. Until now `resolve::resolve` fail-closed with "scan_command is
configured but not supported in this build" and the lock hardcoded
`scan = "pass"`. Wiring the hook requires deciding: how the command string
is invoked (argv vs shell), what the lock's `scan` field may contain, where
in the lifecycle the scan runs, and who may bypass it.

## Decision drivers

- The config string is user-authored, like the rest of `lunchbox.toml`;
  users write scanners the same way they write shell commands.
- `start --json` must stay one parseable JSON line (DESIGN §18 operators).
- The lock is truth (§9): it must not claim a scan passed when one did not.
- Fail closed (§20.3): a failed scan must leave no run dir behind.
- v1 scope (§3): a scan hook is enough; SBOM/timeout machinery is a non-goal.

## Considered options

1. **Shell string.** Run `sh -c '<scan_command> "$1"' sh <package-source>`:
   the config string keeps its natural shell meaning (arguments, quoting,
   redirections), with exactly one appended quoted path argument.
2. **Argv split.** Tokenize the config and exec directly. Purer, but breaks
   every scanner invocation that uses a shell feature (`2>file`, pipes,
   `--flag value` grouping) and invites ad-hoc quoting rules of our own.
3. **Script-fixture scanners only** (command names a wrapper script the
   user maintains). Least code, but forces a file for `exit 0`-class
   scanners and hides the invocation shape.

## Decision outcome

Option 1, with these rules:

- Invocation: `sh -c '<scan_command> "$1"' sh <package-source>`. The
  package source path is appended as the final quoted argument; the config
  string is otherwise used verbatim and must not itself reference `$1` —
  a config that also embeds `"$1"` receives the path twice (observed:
  `printf '%s' "$1" > record` records a doubled path).
- Scanning happens pre-mount on the pantry source, once per locked skill,
  inside `resolve` after the deny/allow gates and after duplicate-pin
  dedup — a skill shared by several workers scans once.
- Lock vocabulary: `pass` (scanner exited zero), `none` (no `scan_command`
  configured), `overridden` (scanner failed, `--override-scan` given).
  The lock never contains `fail`: a failed scan without override aborts
  the run before the lock is written (DESIGN §20.3).
- `--override-scan` is a CLI-only per-run flag. `prepare_run` callers
  other than `cmd_start` (the TUI picker) pass `false`, so no surface can
  override except an explicit terminal invocation. Per-skill overrides
  (override one bad skill, keep the gate for the rest) would need a new,
  separate flag — out of scope.
- Scanner stdout/stderr is captured, not inherited: `start --json` remains
  one parseable line, and a bounded excerpt of scanner stderr (first 500
  chars, CR stripped, newlines collapsed to `; `) surfaces in the failure
  message. Capture is unbounded in v1 (both pipes read to EOF; TD-002).
  Failure without override bails
  `skill '<name>' failed scan_command '<cmd>' (exit <code>): <excerpt>`;
  spawn/IO failure bails `failed to run scan_command '<cmd>': <io error>`.
- No scanner timeout in v1: the scanner is the user's own tool; a hang is
  observable and killable. Revisit on a real report.
- Fail-closed placement: the scan runs before run-dir creation, so scan
  failures leave nothing behind (no cleanup path needed).
- Audit: when `scan_command` is non-empty, a `scan` event is appended
  after `resolved` with the command, per-skill results, and whether any
  skill was overridden.

## Consequences

- Unscanned runs stop writing `scan = "pass"`; their locks say `none`
  (existing lock-text pins updated).
- `sh` must exist on the host (already assumed by the symlink/mount code
  and mock harnesses).
- A scanner that writes to stdout no longer corrupts `--json` output.
- The scan verdict is point-in-time, like the hash gate (DESIGN §17:
  "new hash ≠ lock; user must re-pin"): symlink mode (the default)
  exposes the live pantry source for the run's lifetime, and copy mode
  has a scan→copy window. Scanners are expected to be read-only on the
  source — one that mutates it silently invalidates the hash the skill
  is locked under.
- `--dry-run` scans (it mounts, so it gates); `--from` scans the union
  once regardless of worker sharing. `doctor` and `tui preview` never
  scan — they are gate-free by design.
- Options 2 and 3 stay rejected: argv-split breaks natural scanner
  invocation; script-fixture-only pushes invocation knowledge out of the
  config where it belongs.

## Confirmation evidence

- Feature-013 in `docs/feature-list.json` and its verification record in
  PROGRESS.md: recording scanner (`printf '%s' > record`) proves the
  shell-string shape (redirection; the appended source path arrives as
  the scanner's argument); `exit 3` fails closed naming skill/command/code;
  `--override-scan` proceeds, warns, and audits `override: true`;
  `echo junk` on scanner stdout keeps `start --json` a single object.
