---
id: tasks-76671b
title: "note/start write the record in whichever checkout they run from, so a worktree and its main checkout diverge silently"
status: idea
priority: 2
created: 2026-09-07T12:51:11Z
updated: 2026-09-07T12:51:11Z
depends: []
tags: [feedback, gap, "from:forge"]
---

Commands run: 'tasks start <id>' and 'tasks note <id>' in a repository's main checkout, then a git worktree was created from that checkout's HEAD, then 'tasks done <id>' inside the worktree.

What happened: start and note wrote tasks/<id>.md in the main checkout, leaving it dirty and uncommitted. The worktree was branched from HEAD, so its copy of the record predated both writes. 'done' in the worktree updated that stale copy, which was then committed and merged. The note written before the worktree existed was never in the branch, and the merge required discarding the main checkout's dirty copy to proceed. One note was lost; nothing warned at any step.

What I expected: some signal that the record I was writing had a divergent copy in another checkout of the same project -- either at start (this project has a worktree whose copy differs) or at note/done. Not a refusal; a warning would be enough.

Why it looks addressable: the claim store already lives outside git and is visible from every worktree, with a session identity and a liveness handle, so the tool already knows other checkouts of a project exist and can be consulted. The markdown record does not get the same treatment. There is also already an escape hatch in the read path (--project resolves the registered root, which from a worktree is how you ask for the main checkout), so the concept of 'this checkout vs the registered one' is present in the CLI's vocabulary.

Not asking for writes to be routed to the registered root -- landing the record change in the same commit as the code is the point of working in a worktree. Just for the divergence not to be silent.
