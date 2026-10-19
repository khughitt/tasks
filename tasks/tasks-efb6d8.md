---
id: tasks-efb6d8
title: "add refuses a body citing `[ref:x]` for an attachment, which can only be attached once the task exists: filing a task around an attached file takes add without the citation, attach, then edit --ref"
status: idea
priority: 2
created: 2026-10-19T21:32:49Z
updated: 2026-10-19T21:33:08Z
depends: []
tags: [feedback, gap, "from:ops"]
agent: claude-code/claude-opus-5-5
---

Command: tasks add with a body holding the citation `[ref:repro]` failed with ref_citation_dangling; `--ref 'repro (...): files:repro.sh'` cannot be given either, since the attachment does not exist yet. Expected: add accepting --attach <path> (with its --ref) in the same call, or a citation to an attachment named in the same command.
