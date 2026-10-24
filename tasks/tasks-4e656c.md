---
id: tasks-4e656c
title: "init --records writes the [checkout] table (and [git] commit = true) into the records directory's config but does not commit it, although that config sets [git] commit = true"
status: idea
priority: 2
created: 2026-10-23T12:39:33Z
updated: 2026-10-24T11:16:38Z
depends: [tasks-34424b]
parent: tasks-72e3d7
tags: [feedback, friction, "from:ops"]
agent: claude-code/claude-opus-5-5
---

Action: tasks init --prefix <p> --records <root> --force on a migrated project whose records directory was already committed in the records repository. Happened: the config was modified in place and left uncommitted; on a fresh project the new records directory was left untracked. Expected: under [git] commit the write commits like every other record write, or the JSON output says the commit was skipped. Constraint: a caller committing it afterwards must check the diff is exactly what init added, and on a host with a stale index must commit that one path with --only. Worked around in the caller.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-24-detached-migration-recovery-brief.md — incident evidence, contract tensions and investigation
