---
id: tasks-efb6d8
title: "add refuses a body citing `[ref:x]` for an attachment, which can only be attached once the task exists: filing a task around an attached file takes add without the citation, attach, then edit --ref"
status: idea
priority: 2
reviewed: true
reviewed_at: 2026-10-20T01:41:21Z
created: 2026-10-19T21:32:49Z
updated: 2026-10-20T01:41:21Z
depends: [tasks-4216a2]
parent: tasks-7102c1
tags: [feedback, gap, "from:ops"]
agent: claude-code/claude-opus-5-5
---

Command: tasks add with a body holding the citation `[ref:repro]` failed with ref_citation_dangling; `--ref 'repro (...): files:repro.sh'` cannot be given either, since the attachment does not exist yet. Expected: add accepting --attach <path> (with its --ref) in the same call, or a citation to an attachment named in the same command.

Scope handoff: [ref:brief] records the verified implementation limits, alternatives, and related work. Wait for tasks-4216a2 to return its design or investigation finding, then re-scope this idea; no implementation approach is approved by this pass.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-19-reference-extensions-brief.md — evidence, alternatives, and the follow-up that wakes this idea

## Notes

- 2026-10-20T01:41:21Z (main): scope: briefed; verified reference limits in d242e98, 7e8f4f0, 3d35e51 and current attachment validation; preserved source and body; waits on tasks-4216a2; brief: docs/notes/2026-10-19-reference-extensions-brief.md
