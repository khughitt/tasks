---
id: tasks-f9fc01
title: Clear a claim whose holder is gone without starting the task
status: doing
priority: 2
size: m
complexity: mid
process: planned
owner: feat/claim-release
created: 2026-10-15T08:05:09Z
updated: 2026-10-15T09:27:19Z
started: 2026-10-15T08:25:35Z
depends: []
parent: tasks-2fa8c6
tags: []
source: tasks-860db2
agent: claude-code/claude-opus-5-5
spec: docs/specs/2026-10-15-claim-release-design.md
plan: docs/plans/2026-10-15-claim-release.md
---

Why: since tasks 0.3.0 (tasks-860db2, spec docs/specs/2026-10-14-claim-pid-view-design.md §3–§4) a pid claim that no command can judge reads `Unverifiable`: recorded in another PID view, recorded before views existed, or recorded with no established view. Nothing prunes it, takes it over without --force, or releases its exclusive holds. It clears only through its own view, its owner's park or done, a reboot, or `start --force`. While held it hides the task from ready/next and blocks rename, rename --adopt, retire and move. When the holder is known gone (a finished sandbox, a dead container), the only manual path is `start --force` followed by park or drop. That writes a takeover note and a resumed/parked lifecycle the task never had. A person needs a direct, recorded release.

Established while scoping (main at 00cdbd8):
- No existing verb fits. `park` on a foreign claim refuses `Live` and `Unverifiable` (src/commands/park.rs, takeover block) and has no --force. `drop`/`done` go through the transition guard (src/commands/mod.rs, `Error::Claimed` in the status path), which lets --force through only for `to == Doing`. `tasks claims` is read-only and opens no checkout.
- Defect in a refusal hint: move's preflight (src/moving/preflight.rs) says "park it with `tasks park {id} ...` first". For a foreign claim that park refuses, so the hint leads nowhere.
- The refusals: rename (src/rename/mod.rs `authorize`), adopt (src/rename/adopt.rs), retire (src/commands/retire.rs `reject_live_claims`) all emit `Error::Claimed(id, describe_claim)` with no remedy. `ready`'s omission warning (src/commands/list.rs `claim_omission` + `force_hint`) names only `start --force`.
- Holds live on the claim (src/holds.rs), so removing the claim releases its exclusive holds. Parks are a separate store entry and are untouched.

Lean (for the spec to confirm): a new verb `tasks release <id> --reason "<why>"`. It removes another session's claim only when it reads `Unverifiable` (and `Stale`, harmlessly). It refuses `Live` with `claimed`, pointing to the owner or `start --force`. It leaves status, owner and park alone and appends one note naming the displaced session and owner, the verdict and its reason, and the given reason, in the `takeover_summary` style (no Unix session pid). Rejected: `park --force` (park records a next step and a park under the releaser: the same false lifecycle) and `claims --release` (a write on a command that opens no checkout, while the note needs the record).

Decisions for the spec:
- Whether the note is a generated lifecycle marker (`released`) with provenance, which obs consumers must learn, or a stamped plain note.
- Which record the note lands in when the claim's worktree copy is newer or gone (`refuse_stale_copy`, src/commands/mod.rs), and whether the release succeeds while the record write is refused.
- Whether the record's status stays `doing` (as after a stale prune) or the verb should offer to return it to `todo`.
- JSON output shape (the contract), and README/skill wording (skills/tasks/SKILL.md mentions the release paths in two places).

Done when: `tasks release` removes an unverifiable foreign claim and records why, refuses a live one, and leaves the lifecycle untouched. The rename, adopt, retire and move refusals and `ready`'s omission warning for an unverifiable claim name it, and move's park hint is corrected. CLI tests cover each refusal hint and the release/refuse pair (inject the view as the claim-pid-view tests do). Skill and README are updated, and `cargo install --path .` is rerun.

Related: tasks-ea23de (pretty output shows the unverifiable reason), tasks-f11052 (sid:0 claims held forever: a release target until fixed).

## Notes

- 2026-10-15T08:25:23Z (main): scope: scoped; rewrote body with evidence (main 00cdbd8), lean (new release verb), spec decisions and done criteria; found move's park hint dead-ends on a foreign unverifiable claim; set todo P2 m mid planned
- 2026-10-15T08:25:35Z (main): started
  provenance: {"harness_session":"claude-code:6724124e-7059-43a8-80ff-e57e2348fafc","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-15T08:25:42Z (feat/claim-release): resumed
  provenance: {"harness_session":"claude-code:6724124e-7059-43a8-80ff-e57e2348fafc","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-15T08:28:48Z (feat/claim-release): parked (waiting on user, review): user reviews docs/specs/2026-10-15-claim-release-design.md in .worktrees/claim-release; on approval the agent writes the implementation plan
  provenance: {"harness_session":"claude-code:6724124e-7059-43a8-80ff-e57e2348fafc","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-15T08:35:26Z (feat/claim-release): resumed
  provenance: {"harness_session":"claude-code:6724124e-7059-43a8-80ff-e57e2348fafc","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-15T08:35:27Z (feat/claim-release): review: spec round 1 — verdict: revise; findings: must-fix 1, should-fix 1; reviewer: codex/gpt-6.1-sol
- 2026-10-15T08:37:22Z (feat/claim-release): spec revision 2: release --adopting <old> reaches the old store's claim (gated on adoption's own classify_registry Fresh; both locks sorted) with an end-to-end hint test; occupancy skips every non-live claim of the released session, parks still count
- 2026-10-15T08:37:23Z (feat/claim-release): parked (waiting on user, review): user reviews spec revision 2 (docs/specs/2026-10-15-claim-release-design.md in .worktrees/claim-release); on approval the agent writes the implementation plan
  provenance: {"harness_session":"claude-code:6724124e-7059-43a8-80ff-e57e2348fafc","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-15T08:58:52Z (feat/claim-release): resumed
  provenance: {"harness_session":"claude-code:6724124e-7059-43a8-80ff-e57e2348fafc","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-15T08:58:53Z (feat/claim-release): review: spec round 2 — verdict: revise; findings: should-fix 1; reviewer: codex/gpt-6.1-sol
- 2026-10-15T08:59:10Z (feat/claim-release): spec revision 3: under --adopting, own and live-foreign refusals name the adoption path, not park/done/start --force; tests assert it. User approved the spec with this fix and asked for the implementation plan
- 2026-10-15T09:10:42Z (feat/claim-release): spec revision 4 (plan stage): stale note carries no reason (pids), prime not lanes, 'rerun the same command'; plan docs/plans/2026-10-15-claim-release.md with 3 steps
- 2026-10-15T09:10:50Z (feat/claim-release): parked (waiting on user, review): user reviews docs/plans/2026-10-15-claim-release.md in .worktrees/claim-release and picks the execution method; then the agent runs the Setup baseline and starts tasks-fada88 (Task 1)
  provenance: {"harness_session":"claude-code:6724124e-7059-43a8-80ff-e57e2348fafc","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-15T09:27:19Z (feat/claim-release): resumed
  provenance: {"harness_session":"claude-code:6724124e-7059-43a8-80ff-e57e2348fafc","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
