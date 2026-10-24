# Spec review, round 3

Frozen revision: 76501bdd333e5c6b49758fdec1b6662b5c57fa6f
Repository: tasks, branch design/tasks-e7424b
Path: docs/specs/2026-10-24-bounded-session-summary-design.md
Reviewer: claude-code/claude-opus-5-5 (identity supplied with the review)
Delivered by: user, with explicit approval for planning

Full returned review and approval:

I'd accept revision 76501bd. All five round-2 findings are resolved and I found nothing blocking. The working file is identical to that commit. I have two minor points, which the implementation plan can absorb, so you can approve the design and let the Codex session write the plan. That session should record this round as review: spec round 3 — verdict: accept; findings: Minor 2; reviewer: claude-code/claude-opus-5-5 before it continues.

How each round-2 finding was resolved
- Rollout split (§8): binary delivery and consumer adoption are now separate, and the reason is stated: the skill is loaded into other projects' sessions. The owner citation checks out: flows' docs/notes/2026-10-22-records-only-direct-path-brief.md §5 names you as flow-trial-1's owner, and evals/trials/flow-trial-1.md has read_on: 2027-02-16.
- Budget (§2): the limit is now 12 KiB, and the sums are right. 12,288 bytes at 3–4 bytes per token is 3,072–4,096 tokens, and 6,000 − 4,096 − 1,000 = 904. The 13,277-byte projection agrees with my earlier estimate from prime-before.json (about 12.7 KB of rows plus metadata). The spec labels the token rate as an assumption and requires a pilot through the real consumer before adoption, which is the right way to handle what can't be measured yet.
- Closeout recovery (§6): the new row points to full prime's closeout and says list and tree can't stand in for it.
- Executable path (§4): recovery now uses current_exe(). If the path can't be read or isn't UTF-8, the command fails with a typed error instead of silently falling back to tasks. That matches your fail-early rule.
- Overflow recovery (§5, §8): the wording no longer hides the change in exit status. The consumer behaviour on summary_too_large is spelled out: keep the original invocation, since the failed output carries no summary.full; capture the full read to a file; and report the file's path visibly.

Minor, for the plan
1. The README can reach enrolled agents. The README now lands with the binary, outside the gate. But skills/tasks/SKILL.md links to README sections (lines 29, 88, 117, 179, 203, 239, 408), so an agent following one of those links into a section that mentions --summary would see it before the gate allows. The plan should put the --summary text in its own README section that no skill link points to.
2. A replaced binary can produce an unusable recovery path. On Linux, when the running binary has been replaced on disk (for example by cargo install --path . while it runs), current_exe() returns the path with  (deleted) appended. The recovery argv would then name a file that doesn't exist. The window is tiny because prime is short-lived, so it's enough to name the case in an acceptance test, or treat it as the typed I/O error the spec already defines.

Keith: accepted for planning.
