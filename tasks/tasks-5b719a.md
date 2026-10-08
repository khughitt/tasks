---
id: tasks-5b719a
title: Add a timed Just recipe for installing the tasks CLI
status: todo
priority: 2
size: s
complexity: low
process: direct
created: 2026-09-30T10:28:08Z
updated: 2026-10-08T09:50:38Z
depends: []
parent: tasks-6ed79b
tags: [feedback, friction, "from:tasks"]
---

Why: this repository requires a test/build front door, but its documented CLI reinstall has no Just recipe. justfile already exposes the timing wrapper and install-skills; README uses cargo install --locked --path ., while AGENTS.md omits --locked.

Done: Add one install recipe through the existing tools/tt wrapper, running cargo install --locked --path . from the Justfile's checkout. Use Cargo's existing installation destination and environment controls; add no custom installer, recipe arguments, config, launcher edits or automatic skill installation. Preserve failure exit codes. Point the maintained README checkout-install/adoption instructions and AGENTS.md reinstall instruction at just install, aligning the locked dependency policy with README; keep binary-only Git installation and install-skills separate. Inspect skills/tasks/SKILL.md for stale local-install advice without adding project-specific build commands to the generic skill.

Verification: Inspect just --dry-run install and use a minimal stubbed cargo invocation to confirm --locked --path ., correct working directory, timing-wrapper target and nonzero failure propagation without touching the live binary. Then use a disposable CARGO_INSTALL_ROOT for one real release install, invoke its bin/tasks explicitly and check version/basic help; inspect normal PATH resolution separately without repointing anything. The recipe is a supported front door, not a promise that any harness allowlist or host-install approval is bypassed. A live installation remains subject to the normal integration authorization.

Where to look: justfile tt binding and install-skills pattern; tools/tt; README.md For agents and Install; AGENTS.md Gates. Existing tests/front_door.rs covers Git-hook dispatch, so do not widen those tests just to mirror a one-line recipe. Record the smoke evidence; use the required test/check recipes before committing.

Process: direct; a single existing-command wrapper, established destination and concrete isolated smoke. P2/s/low.

Original report and prior context (preserved):
Reported symptom: the documented post-change reinstall uses cargo install --path . directly, but agents restricted to the test/build front door have no equivalent Just recipe. The current justfile defines test/check and skill installation recipes, with no CLI reinstall recipe.

Next action: scope the smallest front-door addition around the existing documented install command and vendored timing wrapper. Verify release installation and executable resolution without repointing a shared launcher at a worktree or adding a build abstraction. Coordinate the resulting command pointer with the README and tasks skill; retain the report as an idea until its recipe, destination, and verification are settled.

## Notes

- 2026-10-06T16:52:04Z (main): organization: assigned to guidance/adoption; current justfile confirms the missing reinstall recipe, so compare one existing-command wrapper before proposing wider build tooling.
- 2026-10-08T09:50:37Z (main): scope: scoped; P2/s/low/direct; one timed just install recipe using the README's locked Cargo command, aligned command pointers and isolated installation verification; no new build abstraction or host pointer changes
