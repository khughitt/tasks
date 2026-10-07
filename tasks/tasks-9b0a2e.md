---
id: tasks-9b0a2e
title: "Linked projects: records outside the code checkout ([checkout] config, locate on miss bounded at the git top, where, init --checkout)"
status: doing
priority: 2
size: l
complexity: high
process: planned
owner: feat/linked-projects
created: 2026-10-01T16:01:43Z
updated: 2026-10-07T10:46:36Z
started: 2026-10-07T09:45:04Z
depends: []
parent: tasks-a7460c
tags: [cross-project]
agent: claude-code/claude-opus-5-5
plan: docs/plans/2026-10-07-linked-projects.md
---

Piece 1 of ops docs/specs/2026-10-01-linked-checkouts-design.md (§4.1, §4.1a, §8). A project's tasks/.config.toml may declare [checkout] path; when the walk up (stopped at the git top level) finds no config, locate matches the git main checkout of cwd against registered projects' [checkout]. New: tasks init --checkout <path> (validates a main checkout outside every registered tree; docs/ created in the project root), tasks where [PATH] --json with three outcomes ({project:{prefix,root,checkout,linked}}, {project:null}, error envelope incl. checkout_conflict) plus holds_records (§8.1), and checkout on linked rows of tasks projects --json (§8.2). Claims and parks keep the code worktree as resume location; parked::open_recorded/scan_recorded and their readers (quiet, next, prime parked, show fallback) resolve the project from that path via locate. No command writes into a linked checkout. CLI surface lands in ops cli.toml first (ops plan Task 1); vendored adopt cli.toml in the same commit. Tests: spec §6 tasks (1), nested foreign checkout, resolver outcomes.

## Notes

- 2026-10-01T16:23:47Z (main): spec amended in ops plan review round 1: holds_records counts a top-level tasks/.config.toml only when tracked in HEAD (§8.1, decision 5); the CLI's error JSON goes to stderr as today and consumers read it there
- 2026-10-06T16:52:01Z (main): organization: share registry identity/path classification with tasks-7580d2; before implementation compare locate/where and former-root resolver contracts, preserving the linked-checkout spec and reviewing any required amendment rather than inventing a second resolver.
- 2026-10-06T20:21:49Z (design/tasks-7580d2): tasks-7580d2 implementation reviewed: src/locations.rs is the shared history/normalization index; where's link step adds checkout kind there and never consults former roots (spec 2026-10-06-project-resolution section 5); main integration/install follows Task 6
- 2026-10-07T09:45:04Z (main): started
  provenance: {"harness_session":"claude-code:619d972e-0d37-44a1-840e-6572e2e906ac","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-07T09:45:16Z (feat/linked-projects): resumed
  provenance: {"harness_session":"claude-code:619d972e-0d37-44a1-840e-6572e2e906ac","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-07T09:59:57Z (feat/linked-projects): plan drafted: docs/plans/2026-10-07-linked-projects.md (5 tasks, children tasks-6211fb..tasks-1f5664); spec is ops 2026-10-01-linked-checkouts-design rev 5 (approved). Decisions flagged for review: locate unbounded vs where bounded; open_recorded link-only; init does not relink; unreachable projects skipped; resolve lenient. Task 1 commits to ops main.
- 2026-10-07T10:00:31Z (feat/linked-projects): parked (waiting on user, review): User reviews docs/plans/2026-10-07-linked-projects.md (in .worktrees/linked-projects) and picks an execution method; then the agent executes Task 1 (ops main vocabulary) and Tasks 2-5 here.
  provenance: {"harness_session":"claude-code:619d972e-0d37-44a1-840e-6572e2e906ac","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-07T10:12:33Z (feat/linked-projects): review: plan round 1 — verdict: revise; findings: P1 1, P2 5, P3 1; reviewer: codex
- 2026-10-07T10:15:50Z (feat/linked-projects): resumed
  provenance: {"harness_session":"claude-code:619d972e-0d37-44a1-840e-6572e2e906ac","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-07T10:23:06Z (feat/linked-projects): plan revised for round 1: init refuses every destination in a linked checkout's repository and checks --checkout by repository identity; holds_records uses ls-tree (regular-file blob only); ops Task 1 via an ops worktree with patch-only apply and local merge; methods land with first callers (GitTree.top/worktree/resume_root in Task 3, git_dir/is_main/worktrees in Task 4); real quiet park + claim-move tests; CheckoutScan keeps per-project results for resolve; one filter per test-one. Cross-project: ops check 5 fast path and ops-projects link use the same cat-file -e test (ops-b9b733's record is held in another session's worktree, so not noted there).
- 2026-10-07T10:23:08Z (feat/linked-projects): parked (waiting on user, review): User re-reviews docs/plans/2026-10-07-linked-projects.md (round 2) and picks an execution method; then the agent executes Task 1 in an ops worktree and Tasks 2-5 here.
  provenance: {"harness_session":"claude-code:619d972e-0d37-44a1-840e-6572e2e906ac","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-07T10:28:06Z (feat/linked-projects): review: plan round 2 — verdict: revise; findings: P2 2; reviewer: codex
- 2026-10-07T10:28:07Z (feat/linked-projects): Round 2: all seven original findings addressed. Remaining: tracked_config treats every failed HEAD verification as unborn, hiding an existing malformed branch ref; validate_checkout checks registered roots only by path containment, allowing a repository already holding an in-tree project in an externally stored worktree to gain a second linked project. Both reproduced with scratch Git repositories. Confirmed the two reported ops cat-file fast paths; their correction remains a prerequisite before the guard goes live.
- 2026-10-07T10:37:12Z (feat/linked-projects): resumed
  provenance: {"harness_session":"claude-code:619d972e-0d37-44a1-840e-6572e2e906ac","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-07T10:38:04Z (feat/linked-projects): plan revised for round 2: unborn HEAD recognized explicitly (symbolic-ref resolves, for-each-ref silent and empty), any other HEAD failure is io, with loose-ref and packed-refs corruption tests; --checkout refuses a repository whose common git dir any registered root shares, tested with a project registered from an externally stored worktree.
- 2026-10-07T10:38:06Z (feat/linked-projects): parked (waiting on user, review): User re-reviews docs/plans/2026-10-07-linked-projects.md (round 3) and picks an execution method; then the agent executes Task 1 in an ops worktree and Tasks 2-5 here.
  provenance: {"harness_session":"claude-code:619d972e-0d37-44a1-840e-6572e2e906ac","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-07T10:41:59Z (feat/linked-projects): review: plan round 3 — verdict: accept; findings: none; reviewer: codex
- 2026-10-07T10:42:00Z (feat/linked-projects): Round 3: both remaining findings addressed. Reproduced the new unborn rule in scratch repositories: unborn=true; malformed loose ref and corrupt packed-refs both unborn=false with symbolic-ref exit 128. Registered-root common-directory validation and its regression layout cover the second finding. Plan accepted for subagent-driven execution by the implementing session; implementation tests remain to be run. The two ops cat-file call sites remain a deployment prerequisite owned by the shared-repos session.
- 2026-10-07T10:44:33Z (feat/linked-projects): resumed
  provenance: {"harness_session":"codex:01a115f5-c858-7612-946e-9f9ae2f506d5","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-07T10:45:13Z (feat/linked-projects): execution: user approved plan and selected subagent-driven; reuse linked-projects, direct step tasks under reviewed planned parent; current ops setup recipe supersedes plan assertion that it is absent
- 2026-10-07T10:46:36Z (feat/linked-projects): ruling: Task 1 git apply conflicts only on adjacent newer resolve/obs entries; implement identical additions manually, preserve all newer vocabulary and tests, then review exact diff
