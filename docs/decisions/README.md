# Decisions (ADRs) — lunchbox

MADR-style records for decisions that constrain future work: architecture,
interfaces, data storage, security, deployment, dependencies, and major
user-visible behavior.

## Rules

- File names: `NNNN-short-title-with-dashes.md`, numbered from `0001`;
  numbers are never reused.
- Status of a record: `proposed`, `accepted`, `deprecated`, or
  `superseded by NNNN`.
- When a decision changes, write a new ADR that supersedes the old one;
  never rewrite decision history.
- One decision per ADR.

## Queue

- `0001` — Rust, unconditional: [0001-rust-unconditional.md](0001-rust-unconditional.md) (accepted 2026-09-06).
- `0002` — TUI dependencies: [0002-tui-dependencies.md](0002-tui-dependencies.md) (accepted 2026-09-06).
- `0003` — Path B pack layout and `--from` manifest semantics: [0003-path-b-packs-and-from-semantics.md](0003-path-b-packs-and-from-semantics.md) (accepted 2026-09-06).
- `0004` — Scan-hook invocation, lock vocabulary, and override semantics: [0004-scan-hook-contract.md](0004-scan-hook-contract.md) (accepted 2026-09-06).
- `0005` — Distribution: git-install via GitHub, MIT, no crates.io in v1: [0005-distribution-git-install-mit.md](0005-distribution-git-install-mit.md) (accepted 2026-09-06).
- `0006` — Thin git installer (`add`/`update` into `~/.lunchbox/pantry`): [0006-thin-git-installer.md](0006-thin-git-installer.md) (accepted 2026-09-08).
- `0007` — Menu TUI surface replaces the four single-purpose screens: [0007-menu-tui-surface.md](0007-menu-tui-surface.md) (accepted 2026-09-08).
- `0008` — Calibrated Stage A token budget: [0008-calibrated-token-budget.md](0008-calibrated-token-budget.md) (accepted 2026-09-08).
- `0009` — Run-scoped tool selection and `tool_tokens`: [0009-run-scoped-tool-selection.md](0009-run-scoped-tool-selection.md) (accepted 2026-09-09).

## Template

Copy into a new numbered file:

```markdown
# NNNN. <title>

- Status: proposed | accepted | deprecated | superseded by NNNN
- Date: YYYY-MM-DD

## Context and problem

What forces are at play; what needs deciding.

## Decision drivers

Constraints and goals that rank the options.

## Considered options

Each with honest tradeoffs.

## Decision outcome

What was chosen, stated plainly.

## Consequences

Including the rejected alternatives and what they cost.

## Confirmation evidence

The exact check that proves the decision holds in the repository.
```
