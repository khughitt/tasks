---
id: tasks-fcf6f7
title: Explain the single-line --body requirement when feedback recurs
status: done
priority: 2
size: xs
complexity: low
process: direct
owner: main
created: 2026-09-29T10:44:11Z
updated: 2026-09-30T10:27:21Z
started: 2026-09-30T10:23:58Z
completed: 2026-09-30T10:27:21Z
depends: []
parent: tasks-ea2a79
tags: [feedback, friction, "from:tack"]
agent: claude-code/claude-fable-5-1
---

Why: creating feedback accepts a multiline body, but recurrence stores detail as a single-line note. recur_into currently emits the generic 'note text must be a single line', which does not identify --body or explain the difference. Recurrence does accept a single-line body.

Done: retain the existing recurrence contract and validation error kind, but name --body and explain that recurring feedback appends a single-line detail note; tell the caller to provide one line. Apply this to explicit --recur and automatic exact-title recurrence. Clarify feedback --help and skills/tasks/SKILL.md. Keep multiline creation supported and make no record changes on rejected recurrence.

Where to look: src/commands/feedback.rs::recur_into, src/cli.rs::Command::Feedback body help, tests/cli.rs::feedback_recurs_on_exact_titles_and_refuses_to_guess_on_similar_ones, docs/specs/2026-09-03-feedback-design.md section 3 (which already requires rejection).

Verification: focused integration coverage for explicit and automatic multiline recurrence checks the actionable message, validation kind, and unchanged target bytes; single-line recurrence and multiline creation still succeed. Run just test-one --test cli feedback_recurs, then just test-fast before the implementation commit.

Original report: tasks feedback --recur refuses a multi-line --body with 'note text must be a single line', while a new entry accepts one; the message does not say the body is the cause or that --recur takes no body.

## Notes

- 2026-09-30T09:54:00Z (main): scope: scoped; retained the documented single-line recurrence contract; clarified that --body is supported; xs/low/direct; brief: docs/notes/2026-09-30-note-integrity-brief.md
- 2026-09-30T10:23:58Z (main): started
- 2026-09-30T10:27:21Z (tasks-fcf6f7): done
- 2026-09-30T10:27:21Z (tasks-fcf6f7): Recurring feedback with a multiline --body now fails with a message that names --body and explains that recurrence appends a single-line detail note; validation kind and no-write behavior unchanged. feedback --help and skills/tasks/SKILL.md say the same. Multiline creation still kept.
