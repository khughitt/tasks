---
id: tasks-ffa10e
title: "just install-skills: link every skills/* into the harness skill dirs"
status: done
priority: 3
size: xs
complexity: low
process: direct
owner: main
created: 2026-09-13T18:25:02Z
updated: 2026-09-14T10:49:30Z
started: 2026-09-14T10:34:09Z
completed: 2026-09-14T10:49:30Z
depends: []
tags: [docs]
---

Why: skills/scope landed (ad12540) with two new ln lines in the README's install section, but the links were never created, so /scope was not available to the harness while tasks and curate were. The install is a hand-maintained per-skill list; every new skill needs a re-run nobody is prompted to do. Done: a justfile recipe that symlinks each directory under skills/ into ~/.claude/skills and ~/.agents/skills (idempotent, ln -sfn), and the README install section points at it instead of listing per-skill commands. Where: justfile, README.md 'Skills' section.

## Notes

- 2026-09-14T10:49:30Z (ffa10e-install-skills): just install-skills links every skills/* into ~/.claude/skills and ~/.agents/skills (ln -sfn, idempotent); the README agent-skill section points at it instead of the per-skill ln list
