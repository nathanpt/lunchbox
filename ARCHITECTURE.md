# Architecture — lunchbox

This file answers the standing architecture questions at the summary level;
`docs/design-docs/DESIGN.md` is the detailed design. Update each section as
decisions land in `docs/decisions/`.

## What is this system?

A run-scoped skill runtime named `lunchbox`: it resolves explicit Skill pins
from a pantry (library), mounts a sealed per-run workdir, hands it to a
worker (parent harness via Path A isolation flags, or run-local subagent
definitions via Path B), and always unmounts. Users and use cases:
DESIGN §4.

## Where does work start?

- Entry point: a CLI invocation — argv in, output/exit code out.
- Command surface: DESIGN §16 (binary `lunchbox`, Rust/clap per ADR-0001).
- No code exists yet; there is no runtime start path to trace.

## How do the major components relate?

None exist yet. When the first implementation lands, record here:

- the module map and data flow from argv to output;
- the boundary between core decision logic and I/O — for a CLI this boundary
  is expected to matter, so keep pure logic testable without executing side
  effects (filesystem, network, process spawns).

## Boundaries and invariants

- Standing invariant: behave as a normal Unix CLI — exit 0 on success,
  non-zero on failure, usable in pipelines. Any deviation is a user-visible
  decision and requires an ADR.
- External dependencies: the first set is pinned by DESIGN §21 (clap;
  ratatui + crossterm behind a cargo feature; TOML/serde-level crates).
  Adding beyond that set is decision-worthy (`docs/decisions/`).
- Data storage: none planned yet; if the tool persists state, that choice
  requires an ADR (format, location, migration).

## Where to look next

1. `docs/design-docs/DESIGN.md` — intent and detailed design.
2. `docs/decisions/0001-rust-unconditional.md` — the language decision.
3. `docs/feature-list.json` — the contracted features.
