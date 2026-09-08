# 0006. Thin git installer: `add` / `update` and the managed pantry

- Status: accepted
- Date: 2026-09-08
- Supersedes: the "not a Skill library / not a skill manager" posture where it
  blocked *acquisition* (DESIGN §3 first bullet, §6 intro, §25 README note).
  The per-skill non-goals stand — see "What is still not lunchbox's job" below.

## Context and problem

v0.1.0 reads filesystem pantries only: a user points `--library` or
`library_paths` at directories that already exist. The clone and the
`git pull` that keep a skills repository current are manual, and nothing in
the product teaches them. Cross-project *visibility* already works — one
global `library_paths` entry serves every project — but the acquisition
step that gets the repo onto the machine in the first place is missing.

User pivot (2026-09-08, grill-me session): skills are written once and
reused across projects for similar run types; expecting per-project skill
authoring is untenable; lunchbox needs to answer "where do the skills come
from" with one command. The motivating pantry is
https://github.com/nathanpt/agent-skills — README and LICENSE at the repo
root, Skills as `skills/<name>/SKILL.md` directories one level down.

## Decision drivers

- One command from URL to resolvable skills; zero flags for canonical
  layouts (root pantry, `skills/` pantry).
- Git is already the version system users use for skill repos; lunchbox
  should not grow a second one.
- ADR-0002's minimal-dependency posture: no heavy vendored git crate.
- The ownership boundary that v1 established: lunchbox writes only under
  `~/.lunchbox`; user-authored config and standing skill/agent dirs are
  never mutated.
- Fail closed: broken acquisition state must name itself, not silently
  degrade resolution.

## Considered options

1. **Thin git installer** — `lunchbox add <git-url>` clones a whole skills
   repo into a lunchbox-managed pantry root; `lunchbox update` fast-forward
   pulls. Git remains the sync layer; no per-skill bookkeeping.
2. **Per-skill manager** — install individual skills out of repos, track
   per-skill versions, remove/update individually. This is Kitter / Skills
   Manager's feature set (the DESIGN §3 non-goal); it buries the sealed-run
   core under version bookkeeping and partial-repo extraction.
3. **Docs-only clone workflow** — keep v0.1.0 behavior, document `git
   clone` + `library_paths`. Rejected by the user: the pivot exists because
   the manual step is the gap.
4. **Vendored git (gix)** — no external `git` requirement. Rejected: a
   large dependency tree to avoid depending on a tool the audience
   installed lunchbox with (`cargo install --git`), against the shell-out
   precedent (`scan_command`, adapter spawns).

## Decision outcome

Option 1, with these rules:

- **Verbs.** `lunchbox add <git-url> [--path <subdir>]` and
  `lunchbox update [name]`. Both are action commands like `gc`/`finish`:
  human output, no `--json` twin.
- **Granularity.** The whole repository. No per-skill install, version,
  or remove operations, ever in this design.
- **Storage.** `~/.lunchbox/pantry/<name>/`. The name is the URL's final
  path component minus `.git`, validated as exactly one normal path
  component (same rule as `--from` worker names); anything else fails.
  Re-add over an existing name fails; removal is `rm -rf` of the directory
  (documented). No `--name` override in v1.
- **Clone semantics.** Plain `git clone <url> <dest>` (full clone —
  skills repos are small and shallow clones add edge cases), default
  branch, no ref pinning. Per-run stability stays where it already lives:
  resolve-time hash pins and the lock. A drifting pantry cannot silently
  change a pinned re-run; it fails with a hash mismatch. `--ref` is the
  recorded trigger-based follow-up (someone needs a frozen tag).
- **Update semantics.** `git -C <repo> pull --ff-only`, captured.
  Divergence or failure fails loudly, naming the pantry and the exit code;
  the user resolves it in the clone. No pantries and no argument → no
  output, exit 0 (gc precedent).
- **Implicit managed root.** `~/.lunchbox/pantry` is always searched —
  `add`/`update` never write config. Ordering inside the resolver's search
  roots: CLI `--library`, then project+global `library_paths`, then
  managed pantries, then the `.agents/skills` defaults. User config wins
  over managed on a name clash; an explicitly added pantry wins over
  ambient defaults.
- **Pantry-in-repo detection** (stateless; recomputed on every scan, so
  `doctor` always shows the truth and a restructured repo needs no
  migration): candidates are the repo root (if any first-level child
  directory holds `SKILL.md`) plus each first-level child directory
  (dot-directories like `.git` skipped) that holds `SKILL.md` children.
  Exactly one candidate → that is the pantry root. Zero → fail. More than
  one → fail naming the candidates and suggesting `--path`.
- **`--path` escape hatch.** `add --path <subdir>` pins the pantry root
  explicitly (relative to the repo root). The choice is recorded as one
  line in `~/.lunchbox/pantry/<name>.path` — lunchbox-owned state beside
  the clone, never inside it. A stale override (repo restructured away
  from it) fails closed with the fix named; lunchbox never silently
  re-detects past an explicit pin.
- **Fail-closed placement.** Detection failure at `add` removes the
  freshly cloned directory — no half state. Detection failure later
  (manual drop of a non-repo directory, restructured repo, stale
  override) fails `start`/`preview`/`picker` with the pantry named;
  `doctor` shows the error per pantry without failing.
- **git mechanism.** Shell out to `git`. Absent git → `add`/`update`
  fail closed; `doctor` reports a `git` row (`git <version>`, or
  `git (not found)`).
- **Doctor surface.** `doctor` gains the `git` row and a `pantries:`
  section (name, resolved root, skill count — or the detection error).
  The section is omitted entirely when no pantries exist, so clean
  machines see only the `git` row change. Managed pantries never enter
  the without-Lunchbox estimate: they are not directories the standing
  harness scans.

### What is still not lunchbox's job

Per-skill install/versioning/removal, marketplace search, syncing beyond
`git pull`, and editing skill contents. Kitter / Skills Manager / qvr
remain the tools for those. This ADR adds acquisition of whole repos
only.

## Consequences

- DESIGN §3, §6, §7 (layout + search order), §16 (CLI list), §25 (README
  posture), §26 (defaults table) gain resolution notes; the README's
  "This is not a skill manager" section is rewritten to state the new
  boundary.

## Confirmation evidence

- Feature-014 in `docs/feature-list.json` and its verification record in
  PROGRESS.md (2026-09-08): live `lunchbox add
  https://github.com/nathanpt/agent-skills` succeeded flagless
  (`skills/` auto-detected, 6 Skills); `start --skill grill-me --adapter
  none` mounted from the managed pantry with no `--library` and `finish`
  unmounted; `lunchbox update` fast-forwarded. Mock-git integration
  tests pin ambiguity/override/divergence/missing-git/broken-pantry
  fail-closed behavior; both feature configurations green (114 + 36 and
  99 + 36) with zero warnings; byte-compare against the pre-change binary
  shows `adapters` / `tui preview --json` / `start` outputs identical and
  `doctor` differing only by the new `git` row.
