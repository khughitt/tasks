---
id: tasks-f1a008
title: Update README with agent-oriented install and quick-start guidance
status: done
priority: 1
size: s
owner: open-items
created: 2026-09-03T11:31:36Z
updated: 2026-09-03T11:54:03Z
depends: [tasks-74eef9]
tags: [docs]
---

An agent dropped into a project that uses tasks (or asked to adopt it) should be able to read README.md and, without other context, install the binary, install the skill, run tasks init, and follow the session protocol. Today the README assumes a human reader with the repo checked out; give it a short 'For agents' section with copy-pasteable steps, the install source (cargo install --git once the public repo exists), and a pointer to skills/tasks/SKILL.md.

## Notes

- 2026-09-03T11:54:03Z (open-items): README gains a 'For agents' section: clone, cargo install, skill symlink, init, AGENTS.md line, and the session commands; install section documents cargo install --git
