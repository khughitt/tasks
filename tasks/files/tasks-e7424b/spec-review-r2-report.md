# Spec review, round 2

Frozen revision: 5d95ea1fe22f07439f38e5b82dbab1e256770ba8
Repository: tasks, branch design/tasks-e7424b
Path: docs/specs/2026-10-24-bounded-session-summary-design.md
Reviewer: claude-code/claude-opus-5-5 (identity supplied with the review)
Delivered by: user, to the original Codex session for recording and revision

Full returned review:

I reviewed frozen revision `5d95ea1` of the spec and recommend **revise**, with small changes. The overall approach holds up: an opt-in `--summary` flag, the default left unchanged, whole rows dropped rather than clipped, warnings never dropped, and a full-data route. The figures the spec cites match the code and the measurement file. Two points should be settled before the plan is written: what the 16,384-byte limit is sized against, and which part of the rollout the trial gate actually holds back. Nothing is recorded on the task yet. The Codex session that wrote the spec is the note writer for this round. Its note should be `review: spec round 2 — verdict: revise; findings: Important 2, Minor 3; reviewer: claude-code/claude-opus-5-5`, and it then revises the spec into a new frozen revision for your review.

**What I checked against the code (all correct)**
- The working file is identical to `5d95ea1`. The measurement file's sha256 matches the park note.
- The measurement figures are exact: 133,228 bytes in total; roadmap 81,549 bytes over 46 roots; ready 40,435 bytes over 84 rows; lanes 10,096 bytes over 7 rows.
- The orderings the spec relies on are what the code does. Parked rows are newest first (`parked.rs:72`), doing uses `sort_list`, and closeout uses `sort_ready` (`list.rs:808,853`).
- Errors: `render_error` gives the `{error:{kind,detail}}` envelope and errors exit 1. `halts` is left out when empty. `--closed` affects only the pretty output. `-C` and a repeatable `--without` exist as the spec assumes.
- Lanes are built before any sampling, so a summary that keeps the first few lanes also keeps the full view's cross-lane hold decisions.
- `project_color` never appears in JSON, so `paint_projects` does not change the byte count.
- Each roadmap root's `TaskSummary` already has `child_count` and `open_descendant_count`. So a root shown without its children still says it has descendants. §4 could state this.

**Important**

1. **The trial gate is drawn around the wrong part of the change (§8).** The new binary option changes nothing until something calls it. The `skills/tasks/SKILL.md` guidance, which §8 lists as part of the implementation, is different: that skill is loaded into other projects' sessions. As soon as it mentions `--summary`, enrolled agents can start using it, and that changes what they see. §8 currently treats "an unused opt-in implementation" as the uncertain case and asks the measurement owner to judge it.
   - Suggested fix: split the rollout. The binary, the README and tests can land without the gate. The skill text, hooks and session instructions wait for the readout on 2027-02-16, or for a no-effect verdict on that specific change.
   - Also name the measurement owner (the flow-trial-1 owner) instead of "the measurement owner".

2. **The 16,384-byte limit is not derived from any consumer's limit (§2, §5).** §1 cites a 6,000-token tool budget, but §5 never connects 16,384 bytes to it. Dense JSON with short keys, hex ids and timestamps runs at roughly 3–4 bytes per token, so the limit is about 4,100–5,500 tokens. That fits under 6,000 only narrowly, and only if the harness adds little around the output.
   - I estimated the summary for the measured repo from `prime-before.json` with the proposed caps: about 12.7 KB of rows plus about 0.7 KB of metadata, roughly 80% of the limit.
   - Five lanes alone take 6,564 bytes, at about 1.3 KB each with no `active` rows. One lane with a few active rows would start evicting rows here.
   - Suggested fix: name the consumer budget the limit is sized for and show the margin. If 6,000 tokens is the real constraint, consider 12 KB.

**Minor**

3. **No recovery route for closeout (§6).** The skill says "close goals only when `prime`'s closeout offers them". With a cap of 3, an omitted closeout goal is never offered, and the recovery table has no closeout row. The only recovery is `summary.full`. Add a row saying so, so agents don't search `list` or `tree` for it.

4. **The `full` argv hardcodes `"tasks"` (§4).** When a worktree binary is run by its explicit path, as the live-test rule requires, the recovery read goes through whatever `tasks` is on `PATH`, possibly a different version. Either use the running executable's path, or state that `"tasks"` is a command name, not a path.

5. **§5's "existing nonfatal … warnings remain nonfatal when they fit" hides a real behaviour change.** Under `--summary`, enough warnings turn a successful read into exit 1 with nothing on stdout. Accepting that cost is defensible, and §5 says so later. But the deferred consumer rollout should say what a session-start hook does on `summary_too_large`, for example capture the full read to a file and report its path, so the failure doesn't become a blank session start.

The working file and the worktree are unchanged. I ran no tests, pushed nothing, and left nothing running.
