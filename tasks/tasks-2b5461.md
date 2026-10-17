---
id: tasks-2b5461
title: "A parked task cannot declare the files it needs to resume, so the quiet queue offers work whose inputs are already gone"
status: idea
priority: 2
created: 2026-10-17T00:51:23Z
updated: 2026-10-17T00:51:23Z
depends: []
tags: [feedback, idea, "from:material"]
agent: claude-code/claude-opus-5-5
---

Seen: a task parked for an idle host named its frozen input (a tmpfs path) only in the next-step text; a reboot between park and resume deleted it, and nothing noticed until a resuming agent read the scripts. Idea: an optional artifacts list on a record, each entry a path, a role (resume-input | evidence | scratch), an optional sha256, and the host it lives on (records sync; paths do not). park warns when a resume-input is missing or sits on a volatile mount (tmpfs, /run, /tmp); quiet/next/prime flag entries whose resume-inputs are missing (stat only, hashing on request); done/drop list scratch artifacts still present with the command to remove them, and never delete on their own. Keep actions declarative: arbitrary cleanup commands stored in synced records would run on whichever host closes the task, which is a hazard. Free-text cleanup steps can stay prose in the body.
