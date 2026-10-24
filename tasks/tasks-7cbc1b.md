---
id: tasks-7cbc1b
title: Warn at completion when per-task scratch still contains artifacts
status: todo
priority: 2
size: s
complexity: mid
process: direct
created: 2026-10-23T12:43:58Z
updated: 2026-10-24T11:32:17Z
depends: []
parent: tasks-b34076
tags: [feedback, idea, "from:hq"]
agent: claude-code/claude-opus-5-5
---

Why: a completed task can leave reproduction scripts and verification output in host-local scratch without the task naming them. [ref:brief] separates this close-time warning from the open park-resume design and records the related attachment-routing obstacle. Checked at 89e0371: src/commands/status.rs::close has no scratch check, and no src/ code reads WORK_ROOT.

Done: after a successful done, including an already-done cleanup retry, add a non-blocking warning when WORK_ROOT is nonempty and $WORK_ROOT/<canonical-project-prefix>/<task-id>/ contains any entry. Name the directory and an attach command run with -C at the resolved project checkout and an absolute source path, so an unrelated ancestor .git in scratch does not obstruct the suggested workaround. List only enough directory entries to establish nonemptiness; do not read payloads, recurse, attach files, or delete anything. Leave unset/empty WORK_ROOT and missing/empty scratch directories quiet. Diagnose an unreadable or invalid scratch path rather than silently treating it as empty. Keep existing JSON shapes and place diagnostics in the warnings array; preserve completion/output-reference checks, claims and failed-close behavior. No done/drop generalization, new option, artifact schema, or dependency on park-input design is needed.

Where to look: src/commands/status.rs::close, src/commands/mod.rs (done dispatch, resolved ctx.project.prefix/root and id_out warnings), tests/cli.rs and tests/common/mod.rs; document the warning and canonical-prefix directory convention in README.md and skills/tasks/SKILL.md. Use a key-by-key isolated subprocess environment with temporary HOME, XDG config/state and WORK_ROOT; isolate git config/selectors if a fixture invokes git. Check unset/empty WORK_ROOT, absent/empty/nonempty directories, an invalid or unreadable path, checkout/worktree routing, an already-done retry and an ordinary refused completion. Run focused tests through just test-one, then just test-fast.

Original report (preserved):
Asked by the user: avoid losing track of scratch made during a task. The global instructions place per-task scratch at $WORK_ROOT/<project>/<task-id>/. A task was closed with done while that directory still held a reproduction script and verification output that no record named. A warning on done (when WORK_ROOT is set and that directory exists and is non-empty), naming the directory and the attach command, would catch this mechanically; it could share a mechanism with declared resume inputs for parked tasks, which address park rather than close.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-24-scratch-evidence-brief.md — evidence, bounded warning contract and routing questions
