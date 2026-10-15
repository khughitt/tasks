---
id: tasks-f11052
title: "Stop sid:0 claims from being held forever"
status: todo
priority: 3
size: s
complexity: low
process: direct
created: 2026-10-15T01:48:41Z
updated: 2026-10-15T10:33:28Z
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
