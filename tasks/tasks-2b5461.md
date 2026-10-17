---
id: tasks-2b5461
title: "A parked task cannot declare the files it needs to resume, so the quiet queue offers work whose inputs are already gone"
status: idea
priority: 2
reviewed: true
reviewed_at: 2026-10-17T16:47:39Z
created: 2026-10-17T00:51:23Z
updated: 2026-10-17T16:47:39Z
depends: [tasks-57425c]
parent: tasks-037538
tags: [feedback, idea, "from:material"]
agent: claude-code/claude-opus-5-5
---

Seen: a task parked for an idle host named its frozen input (a tmpfs path) only in the next-step text; a reboot between park and resume deleted it, and nothing noticed until a resuming agent read the scripts. Idea: an optional artifacts list on a record, each entry a path, a role (resume-input | evidence | scratch), an optional sha256, and the host it lives on (records sync; paths do not). park warns when a resume-input is missing or sits on a volatile mount (tmpfs, /run, /tmp); quiet/next/prime flag entries whose resume-inputs are missing (stat only, hashing on request); done/drop list scratch artifacts still present with the command to remove them, and never delete on their own. Keep actions declarative: arbitrary cleanup commands stored in synced records would run on whichever host closes the task, which is a hazard. Free-text cleanup steps can stay prose in the body.

## Notes

- 2026-10-17T02:57:34Z (main): scope: briefed; no record or park field names files today (park store holds next_step, reason, quiet recipe, host, worktree; store is host-local, records sync); lean park-scoped --input paths checked by park and the quiet/next/prime readers on the recorded host, plus a guidance line; record-wide artifacts and done/drop scratch listing left as the user's scope call; no follow-up until then; brief: docs/notes/2026-10-16-park-resume-inputs-brief.md
- 2026-10-17T16:47:39Z (main): scope: briefed; rerun of a stranded brief (premises rechecked at 496a967: ParkInfo still has no file field). Took the brief's lean on Q1 (park-scoped inputs, no scratch listing); filed goal tasks-037538 (parent) and planned design tasks-57425c, which this idea now depends on; brief: docs/notes/2026-10-16-park-resume-inputs-brief.md
