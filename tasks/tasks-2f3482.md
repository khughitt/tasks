---
id: tasks-2f3482
title: sample --older-than 0 and 0w point to 0d in their rejection
status: doing
priority: 3
size: xs
complexity: low
process: direct
owner: main
created: 2026-09-24T13:35:30Z
updated: 2026-09-24T13:43:25Z
started: 2026-09-24T13:43:25Z
depends: []
tags: [feedback, friction, "from:tasks", cli]
agent: claude-code/claude-opus-5-5
---

tasks sample --older-than 0 fails with `bad interval "0": expected a count followed by d or w`; the reporter expected 0 to turn the age window off. By design it does not: every age shares the `<n>d`/`<n>w` grammar, and `0d` is its one spelling of zero (task-curation design §2, `parse_age` in src/defer.rs); tests/cli.rs rejects `7` and `0w` on purpose.

Done when `parse_age` rejects `0` and `0w` with a usage error that names `0d` as the way to skip the age check, the bad-age test in tests/cli.rs asserts that hint for both, and every other rejection is unchanged. Rejected: accepting a bare 0, which would special-case the shared grammar.

## Notes

- 2026-09-24T13:38:23Z (main): scope: scoped; kept the shared age grammar (0d is its only zero, by design) and turned the report into a rejection that names 0d; P3 xs low direct
- 2026-09-24T13:43:25Z (main): started
  provenance: {"harness_session":"claude-code:c3b66540-0acc-4861-917d-8c10dfaea3bd","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
