---
id: tasks-4216a2
title: Establish a safe filing operation for tasks with input attachments
status: todo
priority: 2
size: s
complexity: mid
process: direct
created: 2026-10-20T01:40:49Z
updated: 2026-10-20T01:40:49Z
depends: []
parent: tasks-7102c1
tags: []
source: tasks-efb6d8
agent: codex
---

Question: What is the smallest safe operation for creating a task together with an input attachment, its reference, and the citing body?

Where to start: [ref:brief]; src/commands/add.rs::run and duplicates_of, src/commands/attach.rs::attach, src/refs.rs::require_targets, shared apply_fields/create/save, tests/refs.rs, and tests/attachments.rs. tasks-b9b2e2 already owns multi-file attach; reuse its constraints without extending that implementation task incidentally.

Bound: trace task creation versus attachment validation/write/cleanup, then reproduce one small attachment-backed input in a disposable project with isolated XDG_CONFIG_HOME and XDG_STATE_HOME. Compare existing add/attach/edit with add accepting an attachment and its explicit ref, or attach adding the reference to an existing task. Address input validation, duplicate/source-title reuse, name collisions, cap/storage safety, save failure, retries, and one-commit expectations. No production change or generic transaction framework. Do not turn inputs into pending outputs or weaken citation checks.

Expected result: record the observed sequence and recommendation on this task and in the brief, with failure/acceptance cases and whether a reviewed design is necessary. Name any CLI vocabulary work that must land in ops first.

Ideas it wakes: On completion, run `tasks note tasks-efb6d8 --no-reviewed "<finding>"` before this result's branch merges.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-19-reference-extensions-brief.md — question, evidence, and scope boundaries
