---
id: tasks-4d7a32
title: "Scope skill: a defined re-scope path for scoped todos (re-check premises, narrow, re-rate)"
status: doing
priority: 2
size: s
complexity: low
process: direct
owner: tasks-4d7a32
created: 2026-10-13T09:04:51Z
updated: 2026-10-13T09:17:13Z
started: 2026-10-13T09:11:20Z
depends: []
tags: [feedback, gap, "from:hq"]
agent: claude-code/claude-opus-5-5
---

Why: a user asked to re-scope a scoped todo (hq-4ba161, a research task: re-check its premises against the current tool versions and narrow its question). The scope skill admits only `idea` records, even by explicit id, and curate's `refined` verdict forbids changing meaning, so the agent improvised a body edit plus a free-form `re-scope <date>:` note. Nothing defined which fields to re-rate, how to revalidate, what the audit note says, or what to do when a premise has failed.

Done: `skills/scope/SKILL.md` defines a re-scope path for explicit ids of scoped open records (`todo`, `blocked`); default selection still returns ideas only, and `doing` or live-claimed records are skipped as today. The same evidence and revalidation steps apply (revalidation compares against the record's original status, not `idea`). One verdict per re-scoped member:
- `rescoped`: premises hold; rewrite the body with what is now established (keep every fact, mark settled findings so they are not re-derived), re-rate size, complexity and process; status unchanged.
- `confirmed`: premises hold and nothing narrows; no edit.
- `briefed`/`question`: a premise failed and a decision remains; `edit --status idea` first, then the idea rules apply (`--reviewed` note, brief).
- `drop`: the premise is gone or the work landed; proposal only, as today.
Audit note is a plain `tasks note <id> "scope: <verdict>; <what changed>"` (`--reviewed` is idea-only). The tasks skill's `/scope` line names the re-scope use; the scope-pass design (docs/specs/2026-09-12-scope-pass-design.md, §explicit ids) records the amendment.

Where to look: skills/scope/SKILL.md §1 (select), §3 (verdicts), §6 (revalidate); skills/tasks/SKILL.md near the `/scope` line; curate's verdict list for wording parity. Related, not covered here: tasks-19d4fe (refile verdict), tasks-4e8cef (cross-project root).

## Notes

- 2026-10-13T09:11:14Z (main): scope: scoped; title and body rewritten from the hq-4ba161 hand re-scope (original title: scope skill processes only idea records, so re-scoping a scoped todo had no defined path); todo, P2, s, low, direct: a skill-text change whose verdicts and note form are settled in the body, checked by reading the skill against the hq-4ba161 case
- 2026-10-13T09:11:20Z (main): started
  provenance: {"harness_session":"claude-code:5c5a1fb6-0ada-43d8-817d-dc1f5b986503","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-13T09:11:32Z (tasks-4d7a32): resumed
  provenance: {"harness_session":"claude-code:5c5a1fb6-0ada-43d8-817d-dc1f5b986503","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-13T09:17:13Z (tasks-4d7a32): review: impl round 1 — verdict: revise; findings: Important 2, Minor 8; reviewer: claude-code
