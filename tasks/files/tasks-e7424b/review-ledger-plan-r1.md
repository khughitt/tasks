# Bounded session summaries: review ledger

Task: tasks-e7424b
Round 1: builder acceptance, no findings, codex/gpt-6.1-sol; report spec-review-r1-report.md.
Round 2: revise, Important 2 and Minor 3, claude-code/claude-opus-5-5; full report spec-review-r2-report.md.
Reviewed revision: 5d95ea1fe22f07439f38e5b82dbab1e256770ba8.
Corrected frozen revision: 76501bdd333e5c6b49758fdec1b6662b5c57fa6f.
Spec: docs/specs/2026-10-24-bounded-session-summary-design.md.
Spec round 3: accept, Minor 2, claude-code/claude-opus-5-5; user accepted 76501bd for planning. Full report: spec-review-r3-report.md.
Plan frozen revision: fbb9e6aab5baad757a2f2742bb09a73ec1874155, docs/plans/2026-10-24-bounded-session-summary.md. Plan self-review round 1 accepted with no findings, codex/gpt-6.1-sol; user plan review is pending. No implementation has started.

Ruling: round 2 Important 1 — accept and fix in 76501bd §8. Binary/README/tests may land independently; agent-loaded skill text, hooks and session consumers wait for the 2027-02-16 readout or a specific no-effect verdict from the user as flow-trial-1 owner. Why: the unused option preserves existing reads, while shared guidance can immediately change enrolled agents' behavior. Cost: if the separation is not maintained, early consumer exposure can change the trial's treatment or measurement.

Ruling: round 2 Important 2 — accept and fix in 76501bd §§2,4,5,7,8. Set the cap to 12,288 bytes against the observed 6,000-token tool budget, document the 3–4 bytes/token estimate and 1,000-token framing reserve, and require a consumer pilot. Why: the original 16 KiB cap left too little estimated margin. Cost: more orientation rows are omitted; if the sizing estimate is wrong, truncation can still occur, and the pilot must stop adoption.

Ruling: round 2 Minor 3 — accept and fix in 76501bd §§4,6. Explicitly recover omitted closeout invitations through summary.full's closeout section; also state that shallow roadmap roots preserve child_count and open_descendant_count. Why: list/tree do not supply prime's closeout invitation. Cost: a full read is needed to discover every closable goal.

Ruling: round 2 Minor 4 — accept and fix in 76501bd §§4,7. Use the running executable's absolute current_exe path for recovery, with typed failure if the path cannot be represented, and verify recovery against a different PATH binary. Why: worktree verification must continue through its own executable. Cost: recovery fails if that executable disappears; it must not silently switch binaries.

Ruling: round 2 Minor 5 — accept and fix in 76501bd §§5,8. Name summary_too_large as an opt-in exit-status change; deferred consumers capture a full read through the same executable/context and visibly report the error and saved path. Why: warning-heavy successful full reads can fail the bounded form, and a hook must not leave blank context. Cost: recovery needs another full scan and persistent file I/O; a failed capture remains an explicit consumer failure.

No minor (deferred) findings. No ruling rejects or re-grades a finding.

Ruling: spec round 3 Minor 1 — accept and absorb into plan fbb9e6a Task 2 Step 5 and Task 3 Steps 1/3. Put all new README guidance in a separate top-level section before License, leave every existing skill-linked section unchanged, and add the skill link only after the trial gate. Why: the skill's existing section links could expose summary guidance to enrolled agents. Cost: adoption requires a later guidance/link change; reading the entire README can still discover the opt-in binary feature.

Ruling: spec round 3 Minor 2 — accept and absorb into plan fbb9e6a Task 2 full_argv interface and recovery_paths test. Validate the executable as an existing regular file and return typed io for a missing path, including a synthetic Linux path ending in ' (deleted)'; do not strip the suffix or substitute PATH. Why: current_exe can return an unavailable deleted-executable path after replacement. Cost: a summary can fail during that replacement window; a later replacement after validation can still make fresh recovery fail.

Both round 3 minors are assigned to the reviewed-plan candidate, not implemented or left as minor (deferred). Final implementation and user plan review remain outstanding.

External plan round 1: revise, Important 1 and Minor 2, claude-code/claude-opus-5-5; reviewed fbb9e6aab5baad757a2f2742bb09a73ec1874155. The supplied round label is preserved; the earlier controller plan round 1 was a self-review, not external approval. Revised frozen plan: 3a264b91111137b065393936080188f826b15ed1. User review remains pending; spec is still unchanged at 76501bd.

Ruling: external plan round 1 Important 1 — accept and fix in 3a264b9 Task 3 and Tracking. Inventory thirteen active project startup guides (the eleven top-level guides plus mind6 and nrp), the tasks skill and README startup prescriptions; after plan approval, file each other project's owner task with the same trial gate and keep Task 3 open until all owner dependencies are settled. This repository's AGENTS.md is explicitly included. Leave lore's descriptive halt wording unchanged and exclude retired mind3; re-inventory before adoption and file a lore task if that wording later needs a startup change. Why: changing the skill alone leaves project-directed sessions doing oversized full reads. Cost: twelve owner tasks add coordination, and missing a new surface would leave adoption partial; this does not delay Tasks 1–2.

Ruling: external plan round 1 Minor 2 — accept and fix in 3a264b9 Task 2 interface and scopes_and_recovery assertions. Use std::path::absolute(start_dir(cli.dir)?) rather than canonicalize, preserving the supplied symlink spelling for recovery -C. Why: the spec requires an absolute directory, not resolved storage paths. Cost: lexical absolutization does not guarantee later path availability; recovery can still fail if the directory changes.

Ruling: external plan round 1 Minor 3 — accept and fix in 3a264b9 Task 2 Step 7 and Tracking. File a named post-merge reinstall owner task independent of the trial gate; prepare the documented install from merged main after resolving the live binary/destination, request host-action approval, then verify the installed binary. The code record closes before merge while the reinstall and goal stay open until verified. Why: confined verification alone leaves README and the installed CLI out of step and does not satisfy the repository reinstall rule. Cost: approval can delay installed availability after the code merges; report that state explicitly instead of declaring delivery complete.

All three external plan findings are addressed in the revised plan; none is minor (deferred). Actual implementation and gated owner-task filing are still future work. The original review attachment was refused by the executing layout checker because it quotes a host layout value. Exact raw bytes remain in retained evidence; the attached portable derivative is labelled as such and its mapping is in the task notes. Feedback: ops-1b4842. No checker bypass or exemption is claimed.

Verification: saved measurement sha256 matches the prior park note; design-only projection is 13,277 initial bytes and 11,770 after three roadmap-root evictions; five lanes are 6,564 bytes. Spec JSON example parses and its omission arithmetic is consistent. just test-fast: 1,665 passed, 0 failed, 4 ignored across 14 binaries. tasks check, git diff --check, and the pre-commit hygiene/fmt/clippy/task checks passed. No new product behavior or actual consumer tokenizer fit was tested; those remain future implementation/rollout checks.
