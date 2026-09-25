---
id: tasks-e7a870
title: Support screenshots and other attachments on tasks
status: idea
priority: 1
created: 2026-09-09T01:31:26Z
updated: 2026-09-25T15:54:08Z
depends: []
tags: [quick-add]
source: "mindful:thought:a1a5726e2ff44a0586a4e56dea0e5d86"
---

A task can point at a spec and a plan but not at an image. UI and design tasks want a screenshot as the 'before' reference. Decide the shape: a generic attachments list of repo-relative paths, a typed field per kind (screenshot, file), or reusing a related bin. Also decide where attachment files live in the repo and whether 'tasks check' verifies they exist, in the same way it checks spec and plan headings.

Precedent: tasks-061851 made specs and plans optional attachments.

## Notes

- 2026-09-25T15:54:08Z (main): Raised to P1 2026-09-25: asked for again. A phone screenshot was the evidence that settled a UI bug's cause (it ruled one hypothesis out on sight); with no place to attach it, the task holds a prose transcription and the image stays in a downloads folder. The attachment should travel with the record across checkouts and hosts.
