# SDD ledger — plan: docs/plans/2026-10-15-claim-release.md
Spec: docs/specs/2026-10-15-claim-release-design.md (revision 5)
Pre-flight: Task 1 → Task 2: Task 2 replaces release.rs wholesale and changes run's signature (adds adopting); consumes Writer::Release, Ctx::session_label, require_reason, write_note/summary/landed — consistent with Task 1's code. Task 2 → Task 3: Task 3 consumes release::hint(id, session) defined in Task 2 — consistent. Task 1 → Task 3: Writer lifetime change does not touch Task 3 call sites (preflight passes Writer::Command). No conflicts.
Task 1: Ruling: the plan missed tools/cli.toml, the declared CLI surface src/surface.rs compares with the parser — added the release row after drop, and split the doc comment so its summary is one sentence like its neighbours — cost if wrong: one table row and a help-text wording
Task 1: Ruling: tools/cli.toml is vendored (ops-check 11 refuses an in-place edit) — with the user's approval landed the release row on ops main as 8618bc9 and adopted cli.toml from 8618bc9 here (brings other CLIs' landed rows too) — cost if wrong: one ops commit to revert
Task 1: complete (commits 38558e3..be273cd, tests: just test-fast → test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.63s)
Task 2: Ruling: --adopting needs its cli.toml option row — landed on ops main as 30b4448 (same approved path as 8618bc9) and adopted here — cost if wrong: one ops commit to revert
Task 2: complete (commits 68b513c..a32ddf8, tests: just test-fast → test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.63s)
Task 3: complete (commits 7ca00b9..440cfdf, tests: just test-fast → test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.65s)
Final review (impl round 1): fresh reviewer claude-code/claude-opus-5-5, verdict accept, Minor 9.
Final: Ruling: re-graded Minor 6 (release of a record that exists only in the gone holder's worktree ends in a bare task_not_found after rename/retire/ready advertised the release) to Important — a user following the advertised remedy dead-ends, the class the plan round treated as must-fix — cost if wrong: one error-suffix change
Final: minor (deferred): lock order can break if the routed prefix changes while old's lock is held (release.rs open, old < prefix branch)
Final: minor (deferred): blank-reason check runs after open's locks; spec says before anything is read
Final: minor (deferred): occupants skip matches the session string only; two sandboxes can share sid:1
Final: minor (deferred): move preflight's ownership()? can surface a relay config error instead of claimed
Final: minor (deferred): adopt hint's TaskId::parse(id)? turns a malformed old-store key into a parse error instead of claimed
Final: minor (deferred): test gaps — early-lock branch (old < prefix), second claim B release, live-claim no-hint for rename/retire
Final: minor (deferred): ready/retire/rename hints can point at release for the caller's own unverifiable claim
Final: minor (deferred): README command-list wording ("drop a claim", omits Stale)
Final: fixed record-only-in-holder's-worktree dead end — release_of_a_record_only_in_the_holders_worktree_names_the_rerun_there RED→GREEN, suite 1471/1471 (ef80e7b)
Re-review (impl round 2, scoped to ef80e7b): claude-code/claude-opus-5-5, verdict revise; Important 1 (suffix inside "task {0} not found"), Minor 4.
Final: Ruling: re-graded re-review Minor 2 (unquoted worktree path in the printed rerun) to Important — a printed command that fails when pasted is the defect this fix exists to remove; folded Minor 3 (test should run the printed command from main) into the same test — cost if wrong: none beyond the quoting call
Final: Ruling: new Error::TaskNotFoundNext { id, next } with kind task_not_found, displayed "task {id} not found; {next}" — TaskNotFound's format puts any suffix before "not found"; a dedicated variant keeps the JSON kind and leaves the 18 other constructors alone — cost if wrong: one variant to fold back
Final: fixed garbled not-found detail and unquoted path — release_of_a_record_only_in_the_holders_worktree_names_the_rerun_there RED→GREEN (now with a spaced path, run from main via shell split), suite 1471 passed, 0 failed (eca5ed6)
Final: minor (deferred): under --adopting the not-found guidance checks tasks/<new>-<hex>.md in a worktree that predates the rename, so it offers only "the claim stays"
Final: minor (deferred): elsewhere() compares the claim worktree with project.root rather than resume_root() (harmless for linked projects because the is_file check fails)
Re-review (impl round 3, scoped to eca5ed6): claude-code/claude-opus-5-5, verdict accept; Minor 1.
Final: minor (deferred): show.rs:71 suffixes Error::TaskNotFound the same garbled way ("task X (claimed in …) not found"); could use TaskNotFoundNext
