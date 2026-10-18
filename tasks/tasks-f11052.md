---
id: tasks-f11052
title: "Stop sid:0 claims from being held forever"
status: doing
priority: 3
size: s
complexity: mid
process: direct
owner: main
created: 2026-10-15T01:48:41Z
updated: 2026-10-18T09:57:15Z
started: 2026-10-18T09:55:13Z
depends: []
parent: tasks-2fa8c6
tags: []
source: tasks-860db2
agent: claude-code/claude-opus-5-5
---

From the tasks-860db2 final review (Minor 1). Inside a PID namespace whose session leader is outside it, /proc/self/stat's session field is 0, so the Unix session fallback resolves as sid:0 with pid 0. Under PID views that claim records no view and reads Unverifiable everywhere: pid 0 is never a process and no view can ever clear it, so it is held until reboot or --force. Before 0.3.0 it read stale at once. Decide between recording pid None (a TTL claim) for sid 0 and refusing sid:0 as an identity (every unrelated namespace shell shares it); test with unshare -r -p -f --mount-proc.

## Notes

- 2026-10-15T01:53:20Z (fix/claim-pid-view): widen per tasks-860db2 re-review: the same permanent hold applies to any dead pid recorded without a view (heartbeat rows 2-3, a fresh acquisition with a dead exported pid); decide one rule for all of them
- 2026-10-15T10:33:28Z (main): interim (0.4.0): a held sid:0 claim can now be cleared without a takeover by `tasks release <id> "<why>"` from another session; this task still owns the fix
- 2026-10-18T09:55:13Z (main): started
  provenance: {"harness_session":"claude-code:db33ae25-27fd-4537-883b-1b3aa17ecfdf","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-18T09:57:13Z (main): rule: a pid the writer's own /proc refutes (pid 0; a sid NotFound/zombie; an exported pid NotFound/zombie in the initial namespace) is recorded pid-less (TTL), first write and heartbeat alike, so it never becomes a permanent Unverifiable hold; a pid the writer cannot number (exported, non-initial view) stays Unverifiable per PID-view spec §3. sid:0 is also refused as an identity (every shell in that namespace shares it, so it cannot key ownership); set TASKS_SESSION. Amends spec §2 recording only; §3/§4 unchanged. Rated mid: interacts with the heartbeat table.
