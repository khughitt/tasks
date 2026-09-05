---
id: tasks-d184e3
title: `doing` claims are invisible across worktrees and carry no agent identity or liveness
status: todo
priority: 2
size: l
created: 2026-09-04T14:15:45Z
updated: 2026-09-05T10:02:15Z
depends: []
tags: [feedback, friction, "from:beliefs"]
spec: docs/specs/2026-09-05-work-claims-design.md
---

Task files live on the feature branch, so `tasks prime` in the main checkout shows `todo` for a task another worktree holds as `doing`. `tasks start` records the branch as owner, so two agents on one branch are indistinguishable, and there is no heartbeat, so a `doing` task from a dead session looks the same as one under active work. Observed: two agent sessions began executing the same plan in the same worktree without either noticing the other. Suggestions: record agent identity and pid or host at `tasks start`; have `tasks prime` and `tasks ready` scan registered worktrees and warn "doing, last touched N minutes ago by X"; optionally a lease with a TTL that `tasks note` refreshes.

## Notes

- 2026-09-04T21:25:16Z (main): triage: redacted the reporter's project and worktree; kept as idea pending a design session on agent identity, liveness, and cross-worktree visibility of doing
- 2026-09-05T10:02:15Z (feat/doing-claims): spec revised after review: project-wide mutation lock spanning guard/file/claim, destination-based release, local status never prunes a shared claim
