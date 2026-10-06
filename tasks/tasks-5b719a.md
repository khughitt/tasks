---
id: tasks-5b719a
title: "the documented post-change reinstall (cargo install --path .) is the one build step with no just recipe, so harnesses that only allow their test front door cannot run it"
status: idea
priority: 2
created: 2026-09-30T10:28:08Z
updated: 2026-10-06T16:52:05Z
depends: []
parent: tasks-6ed79b
tags: [feedback, friction, "from:tasks"]
---

Reported symptom: the documented post-change reinstall uses cargo install --path . directly, but agents restricted to the test/build front door have no equivalent Just recipe. The current justfile defines test/check and skill installation recipes, with no CLI reinstall recipe.

Next action: scope the smallest front-door addition around the existing documented install command and vendored timing wrapper. Verify release installation and executable resolution without repointing a shared launcher at a worktree or adding a build abstraction. Coordinate the resulting command pointer with the README and tasks skill; retain the report as an idea until its recipe, destination, and verification are settled.

## Notes

- 2026-10-06T16:52:04Z (main): organization: assigned to guidance/adoption; current justfile confirms the missing reinstall recipe, so compare one existing-command wrapper before proposing wider build tooling.
