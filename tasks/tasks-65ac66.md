---
id: tasks-65ac66
title: Acceptance tests and documentation
status: done
priority: 2
size: m
complexity: mid
process: direct
created: 2026-09-22T14:15:49Z
updated: 2026-09-22T16:29:11Z
completed: 2026-09-22T16:29:11Z
depends: [tasks-634c4a]
parent: tasks-8921f4
tags: []
model: "claude-opus-5[1m]"
agent: "claude-code/claude-opus-5[1m]"
plan: docs/plans/2026-09-22-relay-ancestry-identity.md
step: "Task 8: Acceptance tests and documentation"
---

## Notes

- 2026-09-22T16:29:11Z (design/relay-ancestry): done
  provenance: {"harness_session":"claude-code:3b265943-33e2-4466-947a-581d11a9cbb0","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-22T16:29:11Z (design/relay-ancestry): Fourteen acceptance tests over the real ancestry: a relay claim keyed by the agent id with its proof, park and close by the owner after the registry is removed, a resume refused because parking released the claim, repeated start keeping the held identity, a nested codex session refused against its outer claude owner, a contradicted hint refused at acquisition and again on a held claim, an empty registry refusing rather than falling to terminal identity, a world-readable registry refused, the explicit pair working with no registry, an explicit mismatch staying foreign under one harness, a mid-run mode change continuing a natively held claim without re-keying or taking over, --force refused without a resolved identity, and relay off keeping a harness session on the native ladder. Two harness defects fixed along the way: sh -c execs its script's last command in place, which was replacing the shim with tasks and erasing the ancestry under test (a builtin now closes every script), and a copied /bin/sh raced sibling test threads into ETXTBSY (a symlink supplies the comm instead). README gains a Relay identity section and the agent skill three sentences.
  provenance: {"harness_session":"claude-code:3b265943-33e2-4466-947a-581d11a9cbb0","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
