# Tech debt tracker

Known debt and deliberately parked "while I'm at it" work. A row leaves this
table only by being paid off or explicitly rejected (with a note saying why).

| Id | Added | Debt | Why it matters | Payoff condition |
|---|---|---|---|---|
| TD-001 | 2026-09-06 | `pi_spawn_records_audit_and_aborts` (tests/cli.rs) is timing-flaky under heavy parallel load: it failed 2 of ~10 full-suite runs during the TUI milestone while passing 5/5 standalone and 3/3 in immediate full-suite re-runs; the mock-pi spawn/exit/`kill -0` polling races when the test machine is saturated. Not caused by the TUI diff (spawn path untouched; suite pins its output). | Erodes trust in CI: a red suite that passes on re-run trains people to re-run instead of read. | When a failure reproduces, capture the mock's stderr and replace the exit-polling with a deterministic handshake (mock writes a done-file the test polls) or bump its wait budget. |

When recording debt, prefer an actionable payoff condition ("rewrite X when
feature Y lands") over vague urgency.
