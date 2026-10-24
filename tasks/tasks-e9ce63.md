---
id: tasks-e9ce63
title: init --force --records cannot re-point a detached project's checkout after the checkout directory moves
status: idea
priority: 2
reviewed: true
reviewed_at: 2026-10-24T11:16:24Z
created: 2026-10-23T11:56:47Z
updated: 2026-10-24T11:16:24Z
depends: [tasks-34424b]
parent: tasks-72e3d7
tags: [feedback, gap, "from:hq"]
agent: claude-code/claude-opus-5-5
---

Moving a detached project's code checkout to a new directory leaves the records config's [checkout] path naming the old directory. Running init --prefix <p> --force --records <records-root> in the moved checkout refuses with a config error: the records config already declares a detached checkout at the old path, and init does not relink an existing project. Expected: a forced re-point that rewrites [checkout] path and the registry row, as init --force does for an in-tree project, so a tool that renames and moves a checkout can finish for a detached project.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-24-detached-migration-recovery-brief.md — incident evidence, contract tensions and investigation

## Notes

- 2026-10-24T11:16:24Z (main): scope: briefed; forced relocation is blocked by init_detached's existing-checkout guard; safe identity and synchronized-config changes remain to establish; evidence: eedf9a9; waits on tasks-34424b; brief: docs/notes/2026-10-24-detached-migration-recovery-brief.md
