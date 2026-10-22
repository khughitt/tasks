---
id: tasks-ab586f
title: Document the safe write order for scope briefs and follow-up references
status: todo
priority: 2
size: xs
complexity: low
process: direct
created: 2026-10-20T16:06:05Z
updated: 2026-10-22T22:35:17Z
depends: []
parent: tasks-5408fe
tags: [feedback, friction, "from:hq"]
agent: claude-code
---

Why: scope needs a brief naming its follow-up task ids and follow-up tasks whose required input reference names that brief. The current guidance does not order those writes, while add/edit intentionally require a local input or context target to exist. [ref:scope-guide] is the workflow to clarify; [ref:reference-contract] section 6.1 defines the validation to preserve.

Established at 6698ca8: src/commands/add.rs::run applies reference validation before creating a task; src/refs.rs::require_targets rejects missing local input/context targets. The scope skill sections 4-6 require follow-up ids, brief references, dependency edges and final audit notes, but do not name the necessary write sequence.

Decision: document the existing two-stage workflow. Reject allowing missing input references or temporarily labeling the brief as an output: those weaken validation or misstate the consuming task's input. No parser, JSON, CLI option or reference-contract change is needed.

Done:
- The scope skill explicitly orders writing or reusing the brief with named follow-up roles and clearly pending ids, filing or reusing follow-ups with valid references to the existing brief, and replacing the pending ids with the returned real ids in the final decomposition.
- Complete the real-id references, Ideas it wakes instructions and dependency edges before the final scope audit notes. Preserve immediate pre-write member revalidation, the all-skipped/no-artifact rule, the three-document ceiling and each member's verdict rules.
- On rerun or partial failure, reuse the existing brief and follow-ups, and report exactly which writes succeeded. Do not leave fabricated task ids or invent another transaction mechanism.

Where: [ref:scope-guide] sections 4-6; src/refs.rs::require_targets; src/commands/add.rs::run; tests/refs.rs reference-target validation cases. This is a child of the existing review-pass follow-through goal tasks-5408fe.

Verification: walk the documented order against those implementations and existing tests for one new brief/follow-up, a rerun reusing both, and an all-skipped pass. Confirm every saved input reference names an existing document and the completed decomposition uses real ids; run the repository's documented checks. No new test harness is needed for this wording change.

Original report: The scope skill has a pass write a brief that lists its follow-up task ids and file follow-ups whose --ref points at that brief, but add refuses a ref to a file that does not exist yet, so the brief must be written with placeholder ids and patched after the adds; the skill could name that order or add could accept a ref created later in the pass

## References

- scope-guide (input, required, kind=guide): skills/scope/SKILL.md — workflow whose brief and follow-up write order must be explicit
- reference-contract (context, kind=spec): docs/specs/2026-10-19-task-references-design.md — local input and context targets must exist when written
