---
id: tasks-82725f
title: "curate skill never says whether or how to commit the edits it makes in each sampled project, or to check that project's profile first"
status: idea
priority: 2
created: 2026-10-08T20:35:39Z
updated: 2026-10-08T20:35:39Z
depends: []
tags: [feedback, gap, "from:ops"]
agent: claude-code/claude-opus-5-5
---

A cross-project pass edits records in several repositories. The skill ends at the note, so the commit convention (per-project pathspec commit) had to be recovered from git log of the previous pass. In a work-profile project the repository's own rules decide commits, which the skill does not mention.
