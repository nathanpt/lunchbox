# 0005. Distribution: git-install via GitHub, MIT, no crates.io in v1

- Status: accepted
- Date: 2026-09-06
- Supersedes: nothing (settles the placeholder DESIGN §25 left for the repository URL; records the license and registry decisions packaging required)

## Context and problem

v1 + Path B + the scan hook are complete (features 001–013) and the README
already promises `cargo install --locked --git <repository-url>` with a
placeholder URL. Packaging requires deciding: which registry (crates.io,
git-only, both), what license, and whether to ship prebuilt binaries. A
hard external fact forces part of this: `crates.io/crates/lunchbox` is
taken by an unrelated crate (v0.1.4, "an async virtual filesystem
interface", dormant ~3 years) and crates.io never frees names, so
publishing there would require renaming the crate.

## Decision drivers

- The README/DESIGN §25 posture already settled on git-only install; no
  evidence has arrived to overturn it.
- No name constraints exist on GitHub or in cargo's git-install path; the
  `lunchbox` binary name is unaffected everywhere.
- A v1.x CLI for terminal users has no audience on npm; wrapping native
  binaries for a JS registry is pure overhead.
- Licensing must be decided before any public distribution; MIT is the
  smallest permissive option consistent with the project's style.
- Binaries for non-cargo users are a want, not a need: the target user
  (DESIGN §4, Maya) has a Rust toolchain or can get one; Windows support
  is unresolved anyway (symlink/unix code paths, DESIGN §17 treats
  Windows copy-mode as future).

## Considered options

1. **GitHub repository + `cargo install --locked --git` + tags.** The
   recorded posture, made concrete: public repo, `v0.1.0` tag, committed
   `Cargo.lock` so installs are reproducible.
2. **crates.io under a renamed crate** (`lbx`, `lunchbox-cli`). Registry
   searchability and `cargo install <name>` ergonomics, at the cost of a
   crate-name/binary-name split or a full rename, plus registry rules
   (versions immutable, source public, permanent name commitment) for a
   project still pre-adoption.
3. **Prebuilt binaries via GitHub Releases (cargo-dist) now.** Toolchain-
   free installs; adds CI surface, per-platform build matrix, and an
   installer script to maintain before anyone has asked for them.
4. **npm wrapper distributing platform binaries** (esbuild-style). Wrong
   registry, wrong audience, double packaging.

## Decision outcome

Option 1, recorded as:

- Distribution is git-only: `https://github.com/nathanpt/lunchbox`,
  tagged releases starting `v0.1.0`; `cargo install --locked --git …
  --tag vN.N.N` is the blessed install command. The committed
  `Cargo.lock` is load-bearing — never merge a PR that drops it.
- License: **MIT** (`LICENSE`); `Cargo.toml` carries `license`,
  `repository`, `readme`, and `rust-version = "1.85"` (AGENTS.md MSRV).
- crates.io is explicitly **deferred**, not rejected: revisit only if
  registry searchability starts to matter, and only alongside a rename
  decision (the `lunchbox` name is permanently taken there).
- Prebuilt Release binaries (cargo-dist or a hand-rolled Actions matrix,
  Linux + macOS first) are deferred until a toolchain-free install is
  actually requested; Windows builds additionally wait on mount-mode
  work.
- npm is rejected for this project.

## Consequences

- The repo URL baked into README/`Cargo.toml` is the contract; moving the
  repo breaks documented installs (acceptable; note it in a future ADR if
  it ever happens).
- Tags are release-significant: cut them only after the CHANGELOG entry
  and a green two-config test run, per AGENTS.md verification rules.
- No registry squatting risk, no publish-irreversibility exposure, and
  installs require a Rust toolchain — the accepted trade.
- A later crates.io publish under a new name would need its own ADR and a
  README install-section update; nothing in the code names the registry.

## Confirmation evidence

- `gh repo view nathanpt/lunchbox` fails (name free at decision time);
  `gh auth status` shows the `repo` scope used to create and push it.
- Post-push: `cargo install --locked --git
  https://github.com/nathanpt/lunchbox --tag v0.1.0` succeeds from a
  clean environment and the installed binary reports v0.1.0 (recorded in
  PROGRESS.md).
