---
id: tasks-77dbc6
title: "Project groups: a named set of projects between one project and --all-projects"
status: idea
priority: 2
created: 2026-09-07T21:48:27Z
updated: 2026-09-07T21:48:27Z
depends: []
tags: [quick-add]
source: "mindful:thought:5ccf9506e36842cf8e635e55248040a9"
---

Scope today is one project (--project <prefix>, -C <dir>) or the whole registry
(--all-projects). Add a middle tier: a named group of projects, so "what should
we work on next from the verifiably projects?" is answerable.

Sketch:
- Declare groups in the registry (tasks config projects.toml), e.g. a [groups]
  table mapping a name to a list of prefixes.
- Accept a group wherever --all-projects is accepted today: list, ready, next,
  prime, tree, tags.
- Open: what to call the concept (groups / families / sets / collections /
  meta-projects); whether a project may belong to more than one; whether the
  group is declared in the registry or derived from something like the git
  remote org.

Motivating example: verifiably = nodes, atoms, beliefs, ... (the projects under
that GitHub org).

Related: tasks-3029be (done) added the registry-wide views this would narrow.
