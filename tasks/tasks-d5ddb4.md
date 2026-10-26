---
id: tasks-d5ddb4
title: Documentation review
status: todo
priority: 1
size: s
complexity: low
process: direct
created: 2026-10-24T12:49:55Z
updated: 2026-10-26T21:20:52Z
depends: []
tags: [docs]
source: curate-docs
agent: codex
---

The reusable record of curate-docs passes over this project: one note per pass naming the rubric version, commit, documents covered, exclusions and findings by verdict; resolution of findings is recorded apart from the review. Never closed by a pass; ops owns its lifecycle.

## Notes

- 2026-10-24T12:49:58Z (main): doc-review: eligible since 2026-10-24: baseline
- 2026-10-24T18:56:28Z (main): Raised to P1 and undeferred on 2026-10-24 at the owner's request: first-pass documentation reviews of active projects run now rather than three a week under the doc-review cap. Run /curate-docs in this project's checkout; the next doc-review run acknowledges the pass as volunteered.
- 2026-10-26T21:20:52Z (main): curate-docs: rubric v3; commit c1bbcae; mechanical: ops-docs 2, ops-check 0: tools/ops-docs absent (identity.toml and guide-contract adoption also absent); covered: README.md sections: introduction 1-16, Quick start 17-87, Concepts 88-256, Reference 257-1430, Bounded session summaries 1431-1472, License 1473-1475 (whole root guide, outside reference budget), AGENTS.md (whole, 1-123); excluded: identity.toml: missing (not read), docs/specs/: dated decision records (linked design consulted only for rule/intent evidence, not judged), docs/plans/: dated implementation plans, docs/notes/: dated briefs/evidence, tools/: vendored tree, tests/fixtures/relay/: vendored contract fixtures (README inspected to establish exclusion); keep: 3, 5, 7, 11; not judged: 1: no identity.toml purpose/abstract, 2: no identity.toml authority declaration (AGENTS design pointer is not a declaration), 6: retired in v2; budget checked mechanically (ops-docs unavailable), 10: no eligible docs reference documents; proposed: AGENTS.md (criterion 4): remove repeated Session protocol and Process and workspace policy, retaining the skill pointer/adoption and repository-specific instructions, and remove the repeated commit-conventions rule; the global instructions and tasks skill supply these (guide-adoption task owns deletion; no prose edits because isolated-worktree setup/test gates exceed this pass's execute-only bounds); stale: README.md Statuses/Agent skills (criterion 8): lacks a reader-facing capture/scope/brief/research/re-scope overview; skills/scope/SKILL.md sections 3-5 and 7 contain the linked procedure, tasks-ba8c21 records the reader need; README.md Record commits (criterion 9): claims automatic writes commit configuration, but src/cli.rs Command::commit_verb excludes init/rename/retire/unretire/unregister and src/written.rs journals records/attachments, confirmed by tasks-d6f0ed; README.md Relay identity lines 684-689 (criterion 9): says every error prescribes TASKS_SESSION plus TASKS_SESSION_PID and this recovers all errors, contradicting its own PID-namespace paragraph and src/relay/resolve.rs SET_SESSION_ONLY/no_match, which explicitly forbids the PID override there; unresolved: 5
