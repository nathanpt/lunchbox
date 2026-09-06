# lunchbox

A run-scoped skill runtime: hand an AI coding agent only the Skill packages
needed for this job, then take them back. Full design:
`docs/design-docs/DESIGN.md` (revised 2026-09-06 after design review).

Status: **pre-implementation** — design reviewed and amended; no source
code yet.

## Running

Not yet possible — no code exists. The stack is decided: Rust, single
binary (ADR-0001 in `docs/decisions/`). v1 install will be
`cargo install --locked --git`; commands land in `AGENTS.md` as the
skeleton appears.

## Repository map

| Path | Purpose |
|---|---|
| `docs/design-docs/DESIGN.md` | The design — source of intent (revised post-review) |
| `AGENTS.md` | Entry point for agents working in this repository |
| `ARCHITECTURE.md` | System shape, boundaries, invariants |
| `PROGRESS.md` | Current state, verification status, next move |
| `CHANGELOG.md` | Release-significant changes |
| `docs/feature-list.json` | Machine-readable feature contract |
| `docs/decisions/` | ADRs for decisions that constrain future work |
| `docs/design-docs/`, `docs/product-specs/`, `docs/exec-plans/`, `docs/references/`, `docs/generated/` | Knowledge store |
