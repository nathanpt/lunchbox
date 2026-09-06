# 0001. Implement Lunchbox in Rust, unconditionally

- Status: accepted
- Date: 2026-09-06 (language first recorded 2026-09-05 in DESIGN.md §21; made unconditional at the 2026-09-06 design review)

## Context and problem

Lunchbox is a run-scoped skill runtime shipped as one binary with a CLI and a
feature-gated TUI. At foundation time the language was deliberately left
open. DESIGN.md §21 recorded Rust but kept a live fallback — "Go is the
fallback if the author is faster there" — so the project's most
consequential choice was still conditional. The 2026-09-06 design review
promoted the TUI to a first-class v1 motivation, which made the live
fallback the expensive option: invoking it mid-project means rewriting the
hasher, mounter, lifecycle, and TUI stack.

## Decision drivers

- Single static binary, millisecond cold start per command.
- Primitives must be boring and solid: `flock`, symlinks, SHA-256 tree
  hashing, TOML.
- The TUI is a primary project motivation; DESIGN.md §21 pins Ratatui
  (0.30) + crossterm (0.29) with snapshot-test machinery.
- No mid-project language switch; the plan must be committal.

## Considered options

1. **Rust, unconditional.** Ratatui is the TUI stack the author wants to
   experiment with; std + small crates cover everything; static binary.
   Cost: Rust iteration speed and borrow-checker tax on the author.
2. **Rust, keep the Go fallback live.** Buys optionality; costs a standing
   rewrite risk and leaves the language decision unanswerable.
3. **Go + Bubble Tea.** Faster to move for many authors; static binary too.
   Cost: abandons the Ratatui motivation; DESIGN §21 pins and the crate
   layout would need rework.

## Decision outcome

Option 1. Lunchbox is implemented in Rust. No fallback is kept live.

## Consequences

- Crate layout per DESIGN.md §21 (`src/config`, `src/library`, `src/hash`,
  `src/resolve`, `src/mount`, `src/tokens`, `src/run`, `src/adapter`,
  `src/tui` behind a cargo feature).
- Go + Bubble Tea is rejected-for-now, not forever. If author velocity or
  ecosystem needs change materially, write a new ADR that supersedes this
  one; do not edit this record.
- A mid-project language switch requires superseding this ADR — it is not
  available as a quiet fallback.

## Confirmation evidence

- `docs/design-docs/DESIGN.md` §21/§26 amended 2026-09-06 to "Rust,
  unconditional"; grep finds no "Go is the fallback" occurrence.
- Design review 2026-09-06: user selected "Rust, unconditional" (choice
  mode) and confirmed the final synthesis.
