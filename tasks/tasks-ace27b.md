---
id: tasks-ace27b
title: "show, add, and edit find git-excluded plans and specs in the main checkout from a worktree"
status: done
priority: 2
size: s
complexity: mid
process: direct
owner: main
created: 2026-09-30T09:44:00Z
updated: 2026-09-30T09:52:23Z
started: 2026-09-30T09:48:51Z
completed: 2026-09-30T09:52:23Z
depends: []
parent: tasks-c4ad8e
tags: [worktree]
agent: claude-code/claude-opus-5-5
---

Why: tasks-2325a1 taught check to look in the main checkout for a spec/plan absent from a linked worktree, but the other readers still use the local root. Reproduced 2026-09-30 on a scratch repo (docs/ in .git/info/exclude, task with --plan/--step, git worktree add): 'tasks show <id>' in the worktree fails with kind io, 'No such file or directory', because Resolver::step_exists reads project.root (src/resolve.rs; called from src/commands/show.rs). The same read backs step validation in apply_fields (src/commands/mod.rs, so edit --step/--plan and add) and the editor path (src/commands/edit.rs), and Resolver::resolve_doc requires the file under the local root.

Done: move the Docs/Located locator from src/commands/check.rs into src/resolve.rs (Resolver gains the lazily asked main_checkout_root) and route step_exists and resolve_doc's existence test through it, so show reports step_found from main's copy and add/edit accept a doc and step that exist only in the main checkout, each with a warning naming it, as check does. A doc absent from both still fails as today.

Check: extend the check_in_a_worktree_reads_git_excluded_docs_from_the_main_checkout setup in tests/cli.rs: show succeeds with step_found true in the worktree; edit --priority and edit --step on the linked task succeed there; a missing-everywhere plan still fails. Update design §7's worktree paragraph.

## Notes

- 2026-09-30T09:48:51Z (main): started
  provenance: {"harness_session":"claude-code:2387b84d-f415-4946-9038-1bb62ec95101","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-30T09:52:23Z (ace27b-worktree-docs): done
  provenance: {"harness_session":"claude-code:2387b84d-f415-4946-9038-1bb62ec95101","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-30T09:52:23Z (ace27b-worktree-docs): show, add, and edit read a spec/plan/step absent from a worktree from the main checkout, warning once per doc; the locator moved from check into Resolver
  provenance: {"harness_session":"claude-code:2387b84d-f415-4946-9038-1bb62ec95101","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
