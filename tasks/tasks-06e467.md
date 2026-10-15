---
id: tasks-06e467
title: "PID views: deferred review minors"
status: todo
priority: 3
size: s
complexity: low
process: direct
created: 2026-10-15T08:05:08Z
updated: 2026-10-15T08:05:08Z
depends: []
parent: tasks-2fa8c6
tags: []
source: tasks-860db2
agent: claude-code/claude-opus-5-5
---

Deferred Minor findings from the tasks-860db2 final review and re-review (spec docs/specs/2026-10-14-claim-pid-view-design.md):
- Identity carries pid: Option<u32> beside pid_source: Option<PidSource>, so a pid without a source (continuation) or a source without a pid can be built. Option<(u32, PidSource)> or an explicit Continued variant makes 'a pid this write did not supply' a type, not a doc-comment convention.
- A malformed recorded pid_view ("garbage") reads Unverifiable with the reason 'recorded in another PID view'; a distinct reason ('recorded PID view unreadable') is more truthful. Amend spec §3's reason list with it.
- Spec §2's amended heartbeat footnote spells out only the absent-pid outcome; add the row-2 result (same pid, unestablished, dead: that pid with no view, Unverifiable as in row 3).
- Tests: pid_view_heartbeat_after_the_recorded_pid_died_keeps_the_claim_held never asserts its first start recorded a view (outside the initial namespace it passes without the fix); add a unit case for heartbeat row 2 with a dead pid; assert describe_claim's 'unverifiable: <why>' text; add a CLI test for Review Focus item 4 (pid-less repeated start keeps a live recorded pid, start and view).
