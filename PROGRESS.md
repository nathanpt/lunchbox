# Progress — lunchbox

Last updated: 2026-09-06 (design review + foundation sync)

## Current repository state

Pre-implementation. 17 tracked files: foundation + user-authored design
(`docs/design-docs/DESIGN.md`, revised 2026-09-06 after the design review) +
ADR-0001. No source code. Branch `main`; everything uncommitted (user has
not authorized a commit).

## Design review (2026-09-06) — settled

- Rust, unconditional — `docs/decisions/0001-rust-unconditional.md`.
- TUI in v1 as a first-class motivation: four Ratatui screens (doctor,
  token-cost preview, skill picker, policy review), feature-gated.
- Build order: core → TUI milestone (DESIGN §23 step 7) → Pi adapter →
  README → Omp → Path B.
- Two-layer config: `~/.lunchbox/config.toml` + `./lunchbox.toml`; project
  wins, `deny` unions.
- Install: `cargo install --locked --git` (crates.io `lunchbox` name is
  taken; publish deferred until a first external user).
- v1 exit bar: scripted MVP CLI loop + human walkthrough of all four
  screens (feature-008).

## Confirmed working surfaces

None — nothing runnable exists.

## Active work

None in flight. Next feature selection comes from `docs/feature-list.json`
(highest priority incomplete: feature-001).

## Blockers and unknowns

| Unknown | Blocks | Resolution path |
|---|---|---|
| Omp per-invocation discovery-off keys (`--skills` filter semantics, `OMP_PROFILE`) | Omp Path A adapter | Probe at adapter implementation (DESIGN §24) |
| Two-layer merge algebra for `allow` / `library_paths` | Config module | Specify when designing `src/config/` (DESIGN §24) |

## Verification status

| Check | Command | Result |
|---|---|---|
| Feature contract parses, 8 features, 0 passing | `jq` over `docs/feature-list.json` | ok |
| Stale pre-decision phrasing removed from docs | grep across root docs + indexes | clean (exit 1) |
| Design doc staleness sweep | grep `docs/design-docs/DESIGN.md` | clean (2026-09-06 session) |
| Tree shape | `find … -type f` | 17 files as designed |
| Working tree | `git status --short` | all untracked; no commit authorized |
| Build/test/lint | — | N/A — no code yet |

Verified environment facts: `pi --help` has `--no-skills, -ns` and
repeatable `--skill <path>` (file or directory); `omp --help` has
`--no-skills` and a `--skills` glob filter but no explicit-root flag.

## Next useful move

Start DESIGN §23 step 1: Rust repo skeleton (`cargo init`, two-layer config
defaults, `testdata/skills` demo pantry), building toward feature-001 and
feature-002.
