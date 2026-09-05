---
id: tasks-7ba741
title: Test + CI iteration cost audit
status: doing
priority: 2
size: m
owner: main
created: 2026-09-04T21:44:54Z
updated: 2026-09-05T08:25:33Z
depends: [ops-31f038]
tags: [testing]
---

Piece of ops-65837b (the cross-project audit in the ops hub). 1. Measure: full-suite wall time, and roughly how often agent full-suite runs fail here. 2. Add a fast or affected-only test target for the inner loop and point AGENTS.md at it; keep the full suite for commit and CI. 3. Use a quiet reporter so test output does not flood agent context. 4. Fix suite hygiene: sleeps, real network, unshared fixtures. Record the before and after numbers in a note on this task.

## Notes

- 2026-09-05T02:38:40Z (main): design: ops docs/specs/2026-09-04-test-ci-audit-design.md; follow §5: (1) justfile + vendored tools/tt, route existing hooks, CI, and documented test commands through it, verify a line lands under each agent; (2) after a week of runs, add a note reading 'baseline <date>: <tt-report --project numbers>'; (3) gates to §4.6, AGENTS.md line, hygiene; (4) close with before/after numbers
- 2026-09-05T08:25:33Z (main): step 1 landed 2026-09-05: justfile (fast=test=cargo test, single crate), vendored tools/tt v2, .githooks + core.hooksPath, .tt/ gitignored, AGENTS.md gates rerouted to just gate/check/test. Verified: test-fast under claude and with agent vars unset both landed in the shared log (~/.local/share/ops/runs.jsonl) with tests=130; no fallback. Warm suite ~2s, check ~3.5s. Codex run and a true by-hand run still owed. Baseline note due ~2026-09-12 from tt-report --project tasks. Hook false positives filed as an ops idea.
