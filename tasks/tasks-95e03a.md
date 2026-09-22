---
id: tasks-95e03a
title: Continuity in the guards
status: done
priority: 2
size: m
complexity: high
process: direct
created: 2026-09-22T14:15:49Z
updated: 2026-09-22T16:22:02Z
completed: 2026-09-22T16:22:02Z
depends: [tasks-3f2dcd]
parent: tasks-8921f4
tags: []
model: "claude-opus-5[1m]"
agent: "claude-code/claude-opus-5[1m]"
plan: docs/plans/2026-09-22-relay-ancestry-identity.md
step: "Task 6: Continuity in the guards"
---

## Notes

- 2026-09-22T16:22:02Z (design/relay-ancestry): done
  provenance: {"harness_session":"claude-code:3b265943-33e2-4466-947a-581d11a9cbb0","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-22T16:22:02Z (design/relay-ancestry): Continuity in the guards. claims::proves_ownership re-derives a claim's recorded host, boot, pid and pid_start against the caller's nearest harness ancestor with no registry read, so a session nested under the owner is refused and a contradicted scoped hint defeats the proof. Resolution carries an identity error rather than raising it, so the two steps that can establish a right to act without one run first and acquisition still requires a resolved identity — --force displaces an owner but never substitutes for one. The three-way Ownership verdict keeps the identity-match path on today's freshly resolved identity, so a native Codex park still records codex:<id>, and only proof-only ownership substitutes the claim's own identity, which is what stops relay being enabled mid-flight from re-keying a natively held claim. refuse_foreign_live_claim, claim_guard and park share the two helpers; a resolution failure on a claimed task now names the holding session.
  provenance: {"harness_session":"claude-code:3b265943-33e2-4466-947a-581d11a9cbb0","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
