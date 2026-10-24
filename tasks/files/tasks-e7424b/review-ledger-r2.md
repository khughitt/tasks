# Bounded session summaries: review ledger

Task: tasks-e7424b
Round 1: builder acceptance, no findings, codex/gpt-6.1-sol; report spec-review-r1-report.md.
Round 2: revise, Important 2 and Minor 3, claude-code/claude-opus-5-5; full report spec-review-r2-report.md.
Reviewed revision: 5d95ea1fe22f07439f38e5b82dbab1e256770ba8.
Corrected frozen revision: 76501bdd333e5c6b49758fdec1b6662b5c57fa6f.
Spec: docs/specs/2026-10-24-bounded-session-summary-design.md.
User review of the corrected revision is pending. No plan or implementation has started.

Ruling: round 2 Important 1 — accept and fix in 76501bd §8. Binary/README/tests may land independently; agent-loaded skill text, hooks and session consumers wait for the 2027-02-16 readout or a specific no-effect verdict from the user as flow-trial-1 owner. Why: the unused option preserves existing reads, while shared guidance can immediately change enrolled agents' behavior. Cost: if the separation is not maintained, early consumer exposure can change the trial's treatment or measurement.

Ruling: round 2 Important 2 — accept and fix in 76501bd §§2,4,5,7,8. Set the cap to 12,288 bytes against the observed 6,000-token tool budget, document the 3–4 bytes/token estimate and 1,000-token framing reserve, and require a consumer pilot. Why: the original 16 KiB cap left too little estimated margin. Cost: more orientation rows are omitted; if the sizing estimate is wrong, truncation can still occur, and the pilot must stop adoption.

Ruling: round 2 Minor 3 — accept and fix in 76501bd §§4,6. Explicitly recover omitted closeout invitations through summary.full's closeout section; also state that shallow roadmap roots preserve child_count and open_descendant_count. Why: list/tree do not supply prime's closeout invitation. Cost: a full read is needed to discover every closable goal.

Ruling: round 2 Minor 4 — accept and fix in 76501bd §§4,7. Use the running executable's absolute current_exe path for recovery, with typed failure if the path cannot be represented, and verify recovery against a different PATH binary. Why: worktree verification must continue through its own executable. Cost: recovery fails if that executable disappears; it must not silently switch binaries.

Ruling: round 2 Minor 5 — accept and fix in 76501bd §§5,8. Name summary_too_large as an opt-in exit-status change; deferred consumers capture a full read through the same executable/context and visibly report the error and saved path. Why: warning-heavy successful full reads can fail the bounded form, and a hook must not leave blank context. Cost: recovery needs another full scan and persistent file I/O; a failed capture remains an explicit consumer failure.

No minor (deferred) findings. No ruling rejects or re-grades a finding.

Verification: saved measurement sha256 matches the prior park note; design-only projection is 13,277 initial bytes and 11,770 after three roadmap-root evictions; five lanes are 6,564 bytes. Spec JSON example parses and its omission arithmetic is consistent. just test-fast: 1,665 passed, 0 failed, 4 ignored across 14 binaries. tasks check, git diff --check, and the pre-commit hygiene/fmt/clippy/task checks passed. No new product behavior or actual consumer tokenizer fit was tested; those remain future implementation/rollout checks.
