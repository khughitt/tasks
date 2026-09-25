---
id: tasks-476c6b
title: prime --all-projects omits tasks that exist only in a worktree branch
status: idea
priority: 2
created: 2026-09-25T02:36:08Z
updated: 2026-09-25T02:36:08Z
depends: []
tags: []
source: ai-21ea5d
agent: claude-code/claude-opus-5-5
---

Found in the ai turn-boundary-gate spec review 2026-09-24 and reproduced live (ai-2f1271): prime joins claims to the registered checkout's task files, so a live claim on a worktree-only task is invisible there with no warning. tasks claims answers the claim question; this is about prime's other readers.
