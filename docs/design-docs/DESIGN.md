# Lunchbox

**Status:** draft, ready to implement — revised 2026-09-06 after design review  
**One-liner:** Hand an AI coding agent only the Skill packages needed for this job, then take them back.

Lunchbox is a run-scoped skill runtime. It is not a Skill library, marketplace, or symlink manager. Kitter, Skills Manager, qvr, and SkillKit already do those jobs. Lunchbox consumes a pantry (a library of Skill packages) and produces a sealed lunchbox for one run.

```
library (pantry)     →   lunchbox (this run's menu)     →   worker (pi / omp / subagent)
keep forever             mount, use, unmount                sees only the mount
```

---

## 1. Problem

Today “installed” means “the Skill folder sits on a path the agent scans.” Global and project installs leak into every session. Skill *descriptions* often enter context before the user types. Subagents often inherit the same folders as the parent. Pasting a Skill into a subagent prompt only works for small text-only cards and throws away version, hash, scripts, and audit.

Three knobs get mashed together:

1. Who is working (parent vs subagent)
2. What package is loaded (Skill body / scripts)
3. What is even visible this run (the menu)

Lunchbox is knob 3, with a clean handoff into knobs 1 and 2.

---

## 2. Goals

- One canonical mount per run: a sealed workdir that contains only the locked Skill packages for that job.
- Fail closed: unknown Skill, hash mismatch, or failed scan → do not mount.
- Teardown is a correctness property: success, failure, SIGINT, and crash all unmount.
- Measure `menu_tokens`: estimated Skill-description tokens without Lunchbox vs this run.
- First-class adapters for **Pi** and **Omp (oh-my-pi)**.
- Two call sites, one mount:
  - **Path A:** wrap the harness process (`--no-skills` + explicit skill path).
  - **Path B:** materialize run-local subagent defs so named workers get different packs.
- Compose with an existing library. Do not replace it.
- Local-first. No account, no required server, no background indexer.
- CLI and a feature-gated TUI are both v1 product surfaces (no web GUI). The
  TUI is a first-class motivation, not a wrapper: doctor, token-cost preview,
  skill picker, and policy review ship in v1.

---

## 3. Non-goals (v1)

- Replacing Kitter / Skills Manager / qvr / SkillKit
- skills.sh marketplace UI
- Translating `SKILL.md` into Cursor `.mdc` / Copilot formats
- Multi-device sync, SSO, hosted control plane
- Supporting 40 agents
- Auto-routing “pick the perfect Skills from vibes” (explicit pins + simple allowlists only)
- Permanently rewriting the user’s standing Omp/Pi agent files
- A full supply-chain SBOM product (a scan hook is enough)
- Prompt-inlining as the architecture (allowed only as a documented fallback for tiny text-only Skills)

---

## 4. Users and use cases

**Primary user — Maya.** Staff engineer, several repos, Pi or Omp daily, already has a Skill pantry. She wants the review helper to see `code-review` + `secrets-scan` and nothing else, and she wants the folder gone when the helper dies.

Use cases:

1. One-shot child harness: `lunchbox start --skill code-review -- -- pi --no-skills …`
2. Omp/Pi session with named specialists (scout / worker / reviewer), each with its own pack
3. CI job that must not see deploy or prod Skills
4. `lunchbox doctor` on an existing machine: show how fat the current menu is, without mounting anything
5. After a run: prove isolation (`menu_tokens`, unmounted=yes, hashes)

---

## 5. Vocabulary

| Term | Meaning |
|---|---|
| Library / pantry | Canonical Skill packages stored elsewhere. Lunchbox reads them. |
| Skill package | Directory with `SKILL.md` plus optional `scripts/`, `references/`, `assets/` |
| Run | One bounded job with an id and a lifecycle |
| Manifest | Intent for this run (which Skills, which worker, expiry) |
| Lock | Resolved, hashed, scanned set actually mounted |
| Sealed workdir | Temp view the agent is allowed to scan |
| Worker | Parent harness or isolated child/subagent that should see only the mount |
| Adapter | Per-harness glue for Path A flags and Path B agent files |
| Pack | Named bundle of Skills for one worker role (e.g. `review`) |
| Teardown | Always unmount, including crash and cancel |

---

## 6. Architecture

```
                    config.toml
                         │
  user CLI ──► resolver ─┼─► lock ──► policy gate ──► mounter
                         │                               │
                    library path                    sealed workdir
                                                         │
                              ┌──────────────────────────┼──────────────────────────┐
                              │                          │                          │
                         Path A                     Path B                      doctor
                     spawn harness            write run-local              scan existing
                     with --no-skills         agent files that             agent skill
                     + --skill <mount>        point at the mount           dirs only
                              │                          │
                              └──────────► worker ◄──────┘
                                              │
                                         audit.jsonl
                                         result.json
                                         teardown
```

Core components (implement as a library + thin CLI, not a daemon):

1. **Library reader** — find a Skill by name in configured pantry paths.
2. **Hasher** — stable content hash of a Skill package tree.
3. **Resolver** — names/pins → lock entries.
4. **Policy gate** — scan hook + allow/deny + hash check.
5. **Mounter** — create sealed workdir, symlink or copy packages in.
6. **Token estimator** — approximate description-token cost of a menu.
7. **Lifecycle** — start / status / invoke / finish / abort / gc.
8. **Adapters** — `pi` and `omp` for Path A and Path B.
9. **Doctor** — read-only report of current agent menus.

No long-running service. Each command is a process. Use a per-run lockfile + `flock` so two commands do not tear down the same run.

---

## 7. On-disk layout

```
~/.lunchbox/
  config.toml
  runs/
    <run_id>/
      manifest.toml
      lunchbox.lock
      workdir/              # sealed scan root: workdir/<skill-name>/SKILL.md
      agents/               # Path B: run-local pi/omp agent files (optional)
      audit.jsonl
      result.json
      pid                  # worker pid if Path A spawned a process
```

Default library search order (first hit wins, later roots are still used for other names):

1. `config.library_paths` in order
2. `./.agents/skills`
3. `~/.agents/skills`

Config is two-layer (decision 2026-09-06):

- **Global:** `~/.lunchbox/config.toml`
- **Project:** `./lunchbox.toml`, meant to be checked into the repo — this is
  where a CI job's "must not see deploy or prod Skills" deny lives

Scalars: project wins. Lists: `deny` unions across layers. Merge semantics
for `allow` and `library_paths` are an open spec item (§24). The
policy-review TUI edits whichever layer it is viewing; the skill picker
writes run manifests under the run dir, never config.

```toml
# ~/.lunchbox/config.toml
library_paths = [
  "~/.agents/skills",
  "~/.claude/skills",
]
default_adapter = "pi"
mount_mode = "symlink"          # symlink | copy
scan_command = ""               # optional external scanner; empty = hash-only gate
allow = []                      # empty = allow any name in the library
deny = []
max_menu_tokens = 2000          # soft warn; hard fail if fail_on_budget = true
fail_on_budget = false
runs_dir = "~/.lunchbox/runs"
```

```toml
# ./lunchbox.toml — project layer (optional, checked in)
deny = ["deploy", "prod-*"]
```

Do not store secrets. Do not write into `~/.claude/skills` or the user’s standing `~/.pi` / `~/.omp` agent trees.

---

## 8. Identity and hashing

A Skill is identified by **frontmatter `name`** if present, else directory name.

Hash: SHA-256 of a canonical tree.

- Walk files in sorted relative-path order.
- Skip: `.git/`, `.DS_Store`, `*.pyc`, `__pycache__/`, `.skill_metadata.json` if it is manager bookkeeping.
- For each file: `path\0<size>\0<raw bytes>\n` then hash the concatenation, or hash each file and hash the sorted `path + digest` list. Pick one and test it. The digest string is `sha256:<hex>`.

Pin forms the resolver accepts:

| Input | Meaning |
|---|---|
| `code-review` | latest / only copy in library |
| `code-review@sha256:abc…` | exact tree |
| `code-review@v1.2.0` | only if the package or library metadata has that tag; otherwise error (do not guess) |

Missing name → error. Two packages with the same name in one library root → error unless pinned by hash.

---

## 9. Manifest and lock

`manifest.toml` is intent. `lunchbox.lock` is what was mounted.

```toml
# manifest.toml
schema = 1
run_id = "lbx_20260905_153012_a1b2"
task = "review PR 412"
adapter = "pi"                    # pi | omp | none
created_at = "2026-09-05T15:30:12Z"

[budget]
max_menu_tokens = 2000

[[workers]]
name = "parent"
pack = ["code-review", "secrets-scan"]

[[workers]]
name = "reviewer"
pack = ["code-review", "secrets-scan"]

[[workers]]
name = "scout"
pack = ["explore"]
```

Worker also takes an optional `description`, used as the run-local agent's
description (§14); omitted, it defaults to `Lunchbox run-local agent for
worker <name>`.

A manifest supplied to `start --from` is intent only: `run_id`,
`created_at`, and `harness_argv` are accepted and ignored — regenerated
per run. Only `task`, `adapter`, `[budget]`, and `workers` are read
(ADR-0003).

```toml
# lunchbox.lock
schema = 1
run_id = "lbx_20260905_153012_a1b2"
resolved_at = "2026-09-05T15:30:12Z"
mount_mode = "symlink"
workdir = "/home/maya/.lunchbox/runs/lbx_20260905_153012_a1b2/workdir"

[[skills]]
name = "code-review"
source = "/home/maya/.agents/skills/code-review"
hash = "sha256:6f2c…"
scan = "pass"
description_tokens = 180
workers = ["parent", "reviewer"]

[[skills]]
name = "secrets-scan"
source = "/home/maya/.agents/skills/secrets-scan"
hash = "sha256:91aa…"
scan = "pass"
description_tokens = 95
workers = ["parent", "reviewer"]

[[skills]]
name = "explore"
source = "/home/maya/.agents/skills/explore"
hash = "sha256:0bb1…"
scan = "pass"
description_tokens = 70
workers = ["scout"]
```

Lock is the only thing teardown and audit trust. Re-resolve on `start`, never mutate a lock after mount.

---

## 10. Resolver rules

1. Expand each pin against library paths.
2. Compute hash.
3. If pin includes a hash and it does not match → fail.
4. If name is on `deny` → fail.
5. If `allow` is non-empty and name is not on it → fail.
6. Run scan hook if configured; non-zero exit → fail (unless `--override-scan`, which must be explicit and audited).
7. Estimate description tokens for the union of each worker’s pack.
8. If `fail_on_budget` and any worker menu exceeds cap → fail.
9. Write lock, then mount.

No fuzzy “this task looks like a review so add code-review.” v1 is explicit.

---

## 11. Sealed workdir and mount

Default: symlink each package directory into `workdir/<name>`.

Windows / if symlink fails: copy the tree. Record `mount_mode` in the lock. Copies must still be deleted on teardown.

Layout the worker sees:

```
workdir/
  code-review/SKILL.md
  code-review/scripts/…
  secrets-scan/SKILL.md
```

Do not also link the rest of the pantry. Do not create a global “current” symlink that another run could trip over. Every path includes `run_id`.

If Path B needs per-worker views, use:

```
workdir/
  packs/
    default/     # or parent/
    reviewer/
    scout/
```

Resolved 2026-09-06 (ADR-0003): `start --from` runs always mount per-worker
packs at `workdir/packs/<worker>/` — single-worker manifests included.
`workers[0]` is the parent; its pack is the Path A scan root, and
`workers[1..]` become run-local agents (§14). The CLI `--skill` form keeps
the flat union at `workdir/` (the MVP layout above), so a child cannot
discover a sibling's Skills by walking `..`: each pack directory contains
exactly that worker's skills.

---

## 12. Progressive disclosure and `menu_tokens`

Do **not** paste Skill bodies into prompts.

- **Stage A (menu):** name + description from frontmatter only. This is what should enter the worker’s system prompt via the harness’s normal skill discovery.
- **Stage B:** harness reads full `SKILL.md` on invoke.
- **Stage C:** scripts/references on demand.

`menu_tokens` is an estimate of Stage A for a given menu:

```
estimate = sum(over skills in menu) tokens(name + description)
```

Use a simple estimator in v1: `ceil(chars/4)` on `name + "\n" + description`. Document that it is approximate. Also compute `without_lunchbox` by scanning the adapter’s usual global+project skill dirs (same estimator). Print both on `start` and `doctor`.

Never dump pack text into a subagent system prompt. That double-pays and bypasses Stage A.

---

## 13. Path A — wrap the harness

Path A is how isolation is *true*. Lunchbox spawns (or prints) a child command whose skill discovery is only the mount.

### Pi

Pi supports disabling default discovery and adding explicit skill paths (`--no-skills`; extra skill dirs via settings or flags — adapter must probe the installed Pi and pin the exact flags in code comments + `lunchbox adapters pi --explain`).

Verified 2026-09-06 on the dev machine (`pi --help`): `--no-skills, -ns`
(disable discovery and loading) and `--skill <path>` (file **or directory**,
repeatable) both exist. The selftest still guards drift:

```text
pi --no-skills --skill <workdir>/<pack> [user args…]
```

or equivalent settings overlay that **does not** merge `~/.claude/skills` / `~/.agents/skills`.

If Pi cannot disable default scan, the adapter must error with a clear message. Do not start a worker and pretend isolation exists.

### Omp

Omp (oh-my-pi) is in the same family. Adapter must discover:

- how to disable `enableAgentsUser` / default `~/.agents/skills` discovery for one invocation
- how to pass an explicit skills root
- binary name (`omp` / `omp` CLI as installed)
- (probed 2026-09-06) `--no-skills` and `--skills=<glob>` exist in `omp --help`, but
  `--skills` only filters discovered skills and `--no-skills` also disables explicit
  custom directories; `OMP_PROFILE` ("isolated agent state") isolates auth/session
  state, not skill discovery. Resolved mechanism: a per-invocation `--config` overlay
  setting `skills.customDirectories` to the workdir with every `enable*` source toggle
  false — verified live on omp 18.1.11 (headless probe reply listed exactly the mounted
  skill; the foreign pantry appeared only without the overlay)

Same rule: if discovery cannot be disabled, refuse to claim Path A isolation.

### Spawn rules

```text
lunchbox start --adapter pi --skill code-review -- -- <harness args>
```

- Everything after `--` is passed to the harness.
- Lunchbox prepends isolation flags.
- Record pid, wait if `--wait` (default when `--` is present).
- On child exit, run finish/teardown unless `--keep` was set.

`--dry-run` prints the exact argv and does not mount-and-forget: still mount, print, then either wait or tell the user to `finish`.

`adapter = none` only mounts and prints `workdir`. Useful for tests.

---

## 14. Path B — run-local subagents

Path B is the product surface for harness-ops people. Do **not** edit the user’s standing agent files.

On `start` with a multi-worker manifest:

1. Write run-local agent definitions under `runs/<id>/agents/`.
2. Point each definition at its pack dir.
3. Tell Pi/Omp to load agents from that run-local directory for this invocation only.

Pi-subagents already has the right ideas: `inheritSkills: false`, explicit `skills` / `skillPath`, child skills not entering the parent catalog. Generate frontmatter that uses those fields. Verify names against the installed extension.

Sketch (Pi-shaped; adjust to actual frontmatter after probing):

```markdown
---
name: reviewer
description: Review specialist for this Lunchbox run only
inheritSkills: false
skillPath: /home/maya/.lunchbox/runs/lbx_…/workdir/packs/reviewer
skills: code-review, secrets-scan
tools: read, grep, find, bash
---
Review the change. Use only the Skills in your skillPath. Do not search ~/.agents/skills.
```

Omp task-agent / subagent config should get the same treatment: a run-local copy, not a patch to `~/.omp/agent/`.

When the run ends, those files die with the run dir.

If the harness cannot load agents from an arbitrary directory, Path B is “print the files and the one-line include snippet” rather than silent mutation. Never leave a standing agent pointing at a deleted mount.

Mechanism resolved 2026-09-06 (ADR-0003; probed against pi 0.84.4 and omp
18.1.11): neither harness can load agents from a run-local directory for one
invocation. pi-subagents discovers only from builtin/package/user
(`~/.pi/agent/agents`)/project (`.pi/agents`) scopes — no env var, flag, or
settings key adds a directory. omp's task-agent frontmatter is
name/description/tools(/spawns/model/thinkingLevel/output) with no skills
field, its settings schema has no `agents.*` keys, `--add-dir` adds a
*workspace* directory, and relocating `PI_CODING_AGENT_DIR` orphans
models/secrets so the child cannot run at all. Both adapters therefore ship
the print fallback: lunchbox writes `runs/<id>/agents/<worker>.md`
(pi-subagents frontmatter for pi; omp format with the body naming the pack
dir for omp), prints an include hint, records `loaded: false` in the audit,
and never writes a standing agent dir. `adapters --explain` pins the probe
results; if a future harness gains a per-invocation agent-dir override, a new
ADR can flip that adapter to Loaded mode.

---

## 15. Script execution

Skills may contain scripts. The worker runs them through the harness as usual, with cwd / relative paths inside the mounted package.

Lunchbox v1 does not sandbox scripts itself. Policy gate + “this run only has these packages” is the boundary. Document that a Skill script is untrusted code.

Optional later: wrap `bash` via a tiny helper that rejects paths outside workdir + repo.

---

## 16. Lifecycle CLI

Binary name: `lunchbox`. Short alias `lbx` if you add one; do not block on it.

```text
lunchbox doctor [--adapter pi|omp] [--json]
lunchbox start [options] [-- <harness argv>]
lunchbox status [run_id]
lunchbox finish [run_id]
lunchbox abort [run_id]
lunchbox gc
lunchbox adapters [--explain]
lunchbox why [run_id]
```

### `doctor`

Read-only. No mount.

Print:

- adapter detected
- global + project skill dirs found
- skill count
- estimated `menu_tokens` as the harness would see them today
- fattest descriptions
- duplicates by name across dirs

This is the education on-ramp. Ship it as soon as the estimator and dir map exist.

### `start`

```text
lunchbox start \
  --task "review PR 412" \
  --skill code-review \
  --skill secrets-scan \
  --adapter pi \
  --library ~/.agents/skills \
  --wait \
  -- -- pi -p "review the staged diff"
```

Also:

```text
lunchbox start --from manifest.toml
```

`--from` rules (ADR-0003): `--skill` and `--from` are mutually exclusive —
pins belong in the manifest workers; `--adapter`/`--task` flags win over the
manifest's fields; manifest `[budget] max_menu_tokens` wins over config; the
supplied manifest's `run_id`/`created_at`/`harness_argv` are regenerated.
Invalid manifests (unknown key, schema ≠ 1, no workers, duplicate worker
name, empty pack) fail closed with no run dir left behind.

Output (human):

```
run            lbx_20260905_153012_a1b2
workdir        ~/.lunchbox/runs/…/workdir
skills         code-review@sha256:6f2c…  secrets-scan@sha256:91aa…
menu_tokens    this run: 275
without        ~18400  (28 skills on pi global+project)
isolation      path A flags applied: --no-skills --skill <workdir>
unmount        run lunchbox finish lbx_…   (auto on --wait exit)
```

`--json` for operators.

### `status`

State: `mounted` | `running` | `finished` | `aborted` | `leaked`.

### `finish` / `abort`

Both teardown the workdir (and `agents/`). `finish` writes `result.json` outcome=`ok`. `abort` writes outcome=`aborted`. Both append audit events. Safe to call twice.

### `gc`

Delete run dirs older than 24h that are not `running`, and any workdir whose lock says mounted but pid is dead. Print what was removed.

### `why`

Five-line plain-language recap of the last run: who worked, which packs, token delta, unmounted yes/no.

### Defaults for `run_id`

`lbx_<utc compact>_<4 rand hex>`. If omitted, `status` / `finish` / `why` use the most recent run.

---

## 17. Policy gate and threat model

Before mount:

- hash matches pin if pinned
- name not denied
- scan hook passed (if set)
- package contains `SKILL.md`
- no symlink escape: if copying, do not follow links out of the package root

Threats and v1 handling:

| Threat | Handling |
|---|---|
| Untrusted `SKILL.md` (injection) | Isolation + small menu; scan hook optional; do not inline body |
| Unexpected new scripts vs last hash | New hash ≠ lock; user must re-pin |
| Fat descriptions bloating parent | `menu_tokens` + tiny packs |
| Helper summary leaking Skill body to parent | Adapter docs: ask helpers for structured results; cannot fully prevent |
| Leftover mounts | `finish`/`abort`/`gc`; pid tracking |
| Later run seeing previous workdir | paths always include `run_id`; no global “current” |
| Symlink blocked on Windows | copy mode, still teardown |
| Adapter lies about isolation | selftest: child `ls` of usual global skill dir must not be how skills are loaded; probe flags |

Fail closed. No silent fallback to “just start Pi with defaults.”

---

## 18. Audit and result

`audit.jsonl` — one JSON object per line.

```json
{"ts":"2026-09-05T15:30:12Z","run_id":"lbx_…","event":"resolved","skills":[{"name":"code-review","hash":"sha256:6f2c…"}]}
{"ts":"…","event":"mounted","mode":"symlink","workdir":"…"}
{"ts":"…","event":"agents","adapter":"pi","files":["reviewer.md"],"loaded":false}
{"ts":"…","event":"spawn","adapter":"pi","argv":["pi","--no-skills","--skill","…"]}
{"ts":"…","event":"unmounted","reason":"finish"}
```

`result.json`:

```json
{
  "run_id": "lbx_…",
  "outcome": "ok",
  "menu_tokens": 275,
  "without_menu_tokens": 18400,
  "skills": [
    {"name": "code-review", "hash": "sha256:6f2c…"}
  ],
  "unmounted": true
}
```

---

## 19. Adapters

v1 ships `pi` and `omp`. Each adapter implements:

```text
detect() -> installed version / binary path
skill_dirs() -> global + project paths for doctor
isolation_argv(run_dir, workdir, pack, user_argv) -> full argv | error (omp writes its per-run --config overlay into run_dir here)
isolation_summary() -> one-line isolation label for start's summary
agent_dir_hint() -> where standing agents live (never write here)
write_run_agents(run_dir, agents) -> AgentFiles   # Path B: &[AgentSpec{name, description, pack_dir, skills}] -> {loaded: bool, files: Vec<PathBuf>, include_hint: Option<String>}
selftest(version) -> isolation flags still exist
explain() -> human text of what we believe about this harness
```

`lunchbox adapters --explain` prints that text. If detection fails, say so.

Claude Code / Codex / Cursor are out of v1. Comments in code may note them as future adapters that likely **cannot** disable global scan.

---

## 20. Failure modes and guarantees

Guarantees:

1. After `finish` or `abort`, `workdir/` does not exist (or is empty and marked torn down).
2. Lunchbox never writes into the user’s standing skill or agent directories.
3. A failed `start` leaves no orphan mount (clean up on any error after mount begins).
4. Two runs never share a workdir.
5. If Path A isolation flags cannot be applied, `start` exits non-zero.

`SIGINT` / `SIGTERM` during `--wait` → abort path.

---

## 21. Implementation sketch

- Language: **Rust**, unconditional (chosen 2026-09-05; confirmed unconditional at the 2026-09-06 design review — record as ADR-0001 in `docs/decisions/`). Single static binary, ms cold start per command, `flock` + symlink/hash/TOML all in std + small crates. Go rejected-for-now: with the TUI a primary motivation, Ratatui outweighs velocity considerations; no mid-project language switch.
- TUI framework: **Ratatui** (Rust, immediate-mode). The TUI is a first-class v1 product surface, not a wrapper: doctor, token-cost preview, skill picker, and policy allow/deny review all ship in v1 (§22 TUI milestone). Core stays framework-free (`clap` + library); TUI behind a cargo feature (`tui-menu`, `tui-doctor`) so `--no-default-features` still ships a working CLI. Rejected OpenTUI (Bun-locked, ~71MB bundle, per-process startup cost — right for agent CLIs, wrong for a flock-and-symlink systems tool). Evidence: vault note `40_Knowledge/46_UI Design/Pretty TUI Settings Pages Design Language.md` (§ Framework Landscape, Top 3 Pros vs Cons).
  - Pins: `ratatui 0.30` + `crossterm 0.29`; start from the component template (`cargo generate ratatui/templates`); `color-eyre` (auto terminal restore) + `config` crate (not stagnating `figment`); `insta` for buffer snapshots; truecolor-assumed with `COLORTERM` fallback; vet any widget crate's crossterm pin before adopting (third-party crates may still pin 0.28 — use the `crossterm_0_28` feature flag then).
- No required runtime (no Node to run `lunchbox` itself).
- Install (v1): `cargo install --locked --git <repo>`. No crates.io publish: the `lunchbox` crate name is taken (dormant async-VFS crate, checked 2026-09-06), and publishing waits for a first external user. Binary name stays `lunchbox`.
- Tests:
  - hasher golden fixtures
  - mounter symlink + copy + teardown
  - resolver pin / deny / missing
  - lifecycle temp `HOME`
  - adapter argv construction (mocked binary)
  - doctor on a fake tree
- `cargo test` must pass without Pi/Omp installed.
- Every interactive screen ships a `--json` mode with golden tests (machine-drivable TUIs: agents assert on JSON, humans get the Ratatui surface).
- TUI frames snapshot-tested headless (Ratatui `TestBackend` or kitty-vt harness, OMP `tui.ts` pattern) — no manual eyeballing in CI.
- Adapter selftest is skipped if the binary is absent.

Suggested crate layout (Rust):

```
src/main.rs               # CLI (clap)
src/config/
src/library/
src/hash/
src/resolve/
src/mount/
src/tokens/
src/run/                  # lifecycle + audit
src/adapter/pi.rs
src/adapter/omp.rs
src/tui/                  # Ratatui surfaces, behind `tui` cargo feature
testdata/skills/          # sample pantry for tests and first-run demo
skills/lunchbox/          # optional SKILL.md that teaches an agent to drive the CLI
```

Sample pantry for first-run / README demo: two tiny Skills (`demo-review`, `demo-scan`) under `testdata/skills/` so `doctor` and `start --library testdata/skills` work on a fresh clone.

---

## 22. MVP vs later

### MVP (build this first; stop and demo)

1. hasher + library reader + resolver (explicit names only)
2. mount symlink/copy + teardown + gc
3. `doctor` against configured dirs
4. `start` / `status` / `finish` / `abort` / `why`
5. `menu_tokens` + `without` estimate
6. `testdata/skills` demo

Done when this loop works:

```text
lunchbox start --library testdata/skills --skill demo-review --skill demo-scan --adapter none
# workdir contains exactly those two packages
lunchbox finish
# workdir gone, result.json unmounted=true
```

### v1 TUI milestone (immediately after MVP; §23 step 7)

Four screens, feature-gated, each with a `--json` twin and snapshot tests:

1. `doctor` — the doctor report, navigable
2. token-cost preview — `menu_tokens` for a candidate pack vs without-Lunchbox
3. skill picker — browse the library, compose a pack, start a real run, finish it
4. policy review — view and edit `allow` / `deny` in the layer being viewed (global or project)

### Finish v1

Path A adapter for **Pi**, with selftest — done when, on a machine with Pi:

```text
lunchbox start --library testdata/skills --skill demo-review --adapter pi --wait -- -- pi --list-models
# or whatever harmless argv; isolation flags must be present in the spawned argv
```

Then the README: one-liner, install (`cargo install --locked --git`), gif-script (even if text-only), compose-with-pantry note.

**v1 exit bar** (design review 2026-09-06): the MVP CLI loop passes scripted, and a human walkthrough of all four TUI screens succeeds against real state — doctor on real dirs, picker → start → finish, policy edit persisting to the right layer.

### Next (v1.x — after the v1 exit bar)

- Omp Path A
- Path B run-local agents for Pi-subagents / Omp task agents
- `--from manifest.toml` multi-worker packs
- optional scan command hook
- `skills/lunchbox/SKILL.md` driver

### Later

- Claude/Codex adapters with honest “cannot isolate” mode
- `qvr.lock` ingest
- Kitter library path autodetect
- Windows junctions
- real sandbox for scripts

---

## 23. Implementation order for a coding agent

Do not start with adapters. Do not start with Path B. Core before TUI; TUI before adapters.

1. Repo skeleton, two-layer config defaults, testdata Skills
2. Hash + library list/lookup
3. Mount + teardown unit tests
4. Lock/manifest structs + start-without-spawn (`--adapter none`)
5. Token estimator + doctor
6. finish/abort/gc/why
7. TUI milestone: the four §22 screens (feature-gated), picker drives a real run
8. Pi Path A argv + `--` passthrough + `--wait`
9. README
10. Omp Path A
11. Path B run-local agents

---

## 24. Open questions (do not block MVP)

- Resolved 2026-09-06: Omp per-invocation discovery-off via a `--config` overlay (`skills.customDirectories` + source toggles off) — recorded in §13 Omp and PROGRESS.md.
- Two-layer merge algebra beyond "project wins / `deny` unions": `allow` and `library_paths` precedence. Specify when the config module is designed.
- Token estimator: chars/4 vs a small BPE later.
- Should `without_menu_tokens` include built-in harness skills we cannot see? v1: only filesystem dirs we scan.
- Resolved 2026-09-06: multi-pack workdir — `--from` runs always mount `workdir/packs/<worker>/`, the CLI `--skill` form keeps the flat union (ADR-0003; recorded in §11).

---

## 25. README posture (write during Finish v1)

First screen:

- One sentence: “Your agent only sees these Skills for this job. Then they vanish.”
- Install
- `doctor` then `start` / `finish`
- `menu_tokens` before/after
- “This is not a skill manager. Point `--library` at Kitter / Skills Manager / `~/.agents/skills`.”

Do not lead with architecture.

---

## 26. Defaults, summarized

| Decision | Default | Why |
|---|---|---|
| First harness | Pi, then Omp | They can disable discovery |
| Isolation | Path A required for `adapter=pi\|omp` | Otherwise we lie |
| UX for specialists | Path B run-local agents | Matches how Omp/Pi users work |
| Standing files | never mutate | Trust |
| Mount | symlink, copy fallback | Simple teardown |
| Resolver | explicit pins only | Fail closed |
| Estimator | chars/4 | Good enough to teach |
| Process model | no daemon | Less to leak |
| Scope | one binary; CLI + feature-gated TUI | Shipable |
| TUI | Ratatui, feature-gated; four screens in v1 | TUI is a v1 product surface; core testable with no terminal |
| Language | Rust, unconditional | Static binary, Ratatui; ADR-0001 |
| Config | two-layer: global + `./lunchbox.toml`, project wins, `deny` unions | Per-repo policy; CI use case |
| Install | `cargo install --locked --git` (v1) | crates.io name taken; no external users yet |
| v1 exit | scripted CLI loop + human TUI walkthrough | Both surfaces are the product |
