---
id: tasks-7ba741
title: Test + CI iteration cost audit
status: done
priority: 2
size: m
complexity: mid
process: direct
owner: main
created: 2026-09-04T21:44:54Z
updated: 2026-09-15T12:31:06Z
started: 2026-09-15T12:25:53Z
completed: 2026-09-15T12:31:06Z
depends: [ops-31f038]
tags: [testing]
model: "claude-opus-5[1m]"
---

Piece of ops-65837b (the cross-project audit in the ops hub). 1. Measure: full-suite wall time, and roughly how often agent full-suite runs fail here. 2. Add a fast or affected-only test target for the inner loop and point AGENTS.md at it; keep the full suite for commit and CI. 3. Use a quiet reporter so test output does not flood agent context. 4. Fix suite hygiene: sleeps, real network, unshared fixtures. Record the before and after numbers in a note on this task.

## Notes

- 2026-09-05T02:38:40Z (main): design: ops docs/specs/2026-09-04-test-ci-audit-design.md; follow §5: (1) justfile + vendored tools/tt, route existing hooks, CI, and documented test commands through it, verify a line lands under each agent; (2) after a week of runs, add a note reading 'baseline <date>: <tt-report --project numbers>'; (3) gates to §4.6, AGENTS.md line, hygiene; (4) close with before/after numbers
- 2026-09-05T08:25:33Z (main): step 1 landed 2026-09-05: justfile (fast=test=cargo test, single crate), vendored tools/tt v2, .githooks + core.hooksPath, .tt/ gitignored, AGENTS.md gates rerouted to just gate/check/test. Verified: test-fast under claude and with agent vars unset both landed in the shared log (~/.local/share/ops/runs.jsonl) with tests=130; no fallback. Warm suite ~2s, check ~3.5s. Codex run and a true by-hand run still owed. Baseline note due ~2026-09-12 from tt-report --project tasks. Hook false positives filed as an ops idea.
- 2026-09-05T08:30:54Z (main): verification complete 2026-09-05: by-hand run (tests null, tty) and Codex run (tests 130, piped) both in the shared log, no fallback. Codex line has agent null: CODEX_CI not exported; filed as an ops idea.
- 2026-09-05T08:53:43Z (main): Codex verified 2026-09-05: run issued by the Codex agent recorded agent codex + session, wrote to the repo fallback .tt/runs.jsonl (sandbox cannot reach the shared log), and tt-report harvested it. The earlier unattributed line was a shell run via the harness ! prefix, not Codex; ops idea withdrawn.
- 2026-09-13T16:47:43Z (main): Complexity mid: justfile, timing wrapper, and hooks exist in the current tree; remaining work is interpreting baseline measurements, checking gate/guidance alignment, and choosing evidence-backed hygiene fixes. The suite contains sleeps whose purpose must be checked before changing them.
- 2026-09-15T12:29:33Z (7ba741-ci-audit): baseline 2026-09-15 (tt-report --project tasks, 09-05..09-15): test 156 runs median 4.4s p90 16.0s fail 0.1; check 314 runs median 1.0s p90 4.0s fail 0.2; hook-pre-commit 299 median 0.6s; hook-pre-push 19 median 1.7s p90 11.7s; test-fast 8 runs, fast/full by agents 0.03; bypasses 106, all 'cargo test [--test cli] <name>' (one test by name). Split at 09-10: test min 0.9s median 2.9s before, min 9.9s median 13.6s after: rename::classify's exhaustive 1.57M-snapshot unit test (fff0f78, 09-08) costs 7.1s in debug and floors every cargo test. Warm today: unit bin 7.3s, cli 3.4s, 10.7s wall. Six 1.1s sleeps in tests/cli.rs order second-resolution timestamps; they run in parallel and are not the long pole.
- 2026-09-15T12:31:06Z (7ba741-ci-audit): step 3 landed 2026-09-15: rename::classify exhaustive enumeration is #[ignore]d; just test-fast [<name>] runs the non-ignored suite or one test by name; just test and hook-pre-push run cargo test -- --include-ignored; AGENTS.md carries the §4.7 line. Before (log 09-10..09-15, warm): test min 9.9s median 13.6s p90 17.8s, no way to run one test through the front door, 106 bypasses. After (warm, this host): test-fast 4.9s (465 tests), test-fast <name> 0.2s, test 10.7s unchanged and still exhaustive. tt-report --since will show the fast/full ratio and bypass count move as agents pick up the line.
