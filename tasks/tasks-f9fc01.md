---
id: tasks-f9fc01
title: Clear a claim whose holder is gone without starting the task
status: idea
priority: 2
created: 2026-10-15T08:05:09Z
updated: 2026-10-15T08:05:09Z
depends: []
parent: tasks-2fa8c6
tags: []
source: tasks-860db2
agent: claude-code/claude-opus-5-5
---

Since tasks 0.3.0 a pid claim that no command can judge (recorded in another PID view, before views were recorded, or with a dead pid and no view) is held until reboot, its owner's release, or start --force. While held it hides the task from ready and blocks rename, rename --adopt, retire and move. The only manual path is start --force (taking ownership) and then park or drop, which writes a takeover note and a lifecycle the task never had. A person who knows the holder is gone (a finished sandbox, a dead container) needs a direct, recorded release. Scope: whether this is a new verb (e.g. tasks release <id> --reason) or a flag on an existing one, what it records on the task, and whether the refusals from rename/retire/move and ready's omission warning should name it.
