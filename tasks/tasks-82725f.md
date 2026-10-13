---
id: tasks-82725f
title: "curate skill never says whether or how to commit the edits it makes in each sampled project, or to check that project's profile first"
status: todo
priority: 2
size: s
complexity: low
process: direct
created: 2026-10-08T20:35:39Z
updated: 2026-10-13T09:21:04Z
depends: []
parent: tasks-5408fe
tags: [feedback, gap, "from:ops"]
agent: claude-code/claude-opus-5-5
---

Why: a curate pass edits records in several projects and a scope pass edits records in its fixed root, but neither skill says whether or how to commit them, so the convention is recovered from git log, and a work-profile project's own commit rules go unmentioned.
Done: skills/curate/SKILL.md and skills/scope/SKILL.md each gain a short closing step: per project touched, under `[git] commit = true` the CLI commits each write, so read any commit_deferred/commit_failed warning and resolve it; otherwise commit only the records and documents the pass wrote, by pathspec (`git -C <root> commit -m "chore(tasks): <curate|scope> pass" -- <paths>`), unless that project's profile or own instructions decide commits differently, in which case leave them and list the uncommitted paths in the summary. Never push. `tasks commit` is named only as the recovery path, since it sweeps every uncommitted record under tasks/. The summary names each project's commit or why none was made.
Check: read both skills' new step against the tasks skill's "Record commits" section and the global profile rules; the next curate pass's summary names its commits.
Where to look: docs/notes/2026-10-13-review-pass-followthrough-brief.md; skills/tasks/SKILL.md "Record commits"; 1ef0ba5 (record commits landed after this report); prior pass commits 0fc5947, 9806c2e. Coordinate with tasks-4d7a32 and tasks-00723a, which also edit these skills.

Original report: A cross-project pass edits records in several repositories. The skill ends at the note, so the commit convention (per-project pathspec commit) had to be recovered from git log of the previous pass. In a work-profile project the repository's own rules decide commits, which the skill does not mention.

## Notes

- 2026-10-13T09:21:02Z (main): scope: scoped; todo P2/s/low/direct under goal tasks-5408fe; covers both curate and scope skills, accounting for record commits (1ef0ba5) that landed after the report; original report kept; brief: docs/notes/2026-10-13-review-pass-followthrough-brief.md
