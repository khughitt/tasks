# Follow-up discovery after completion

Scoping handoff, 2026-10-08. Idea: tasks-d40e8e. Existing lane: tasks-46d207.
This is not an approved design.

## Problem

After a task completes or work merges, ask whether follow-up tasks should be
created or updated. The captured idea proposes project-configured lifecycle hooks;
the outcome is useful follow-up discovery, while the mechanism remains undecided.

## Current behaviour and evidence

- `src/commands/status.rs::close` loads the task, applies `transition`, appends an
  optional message, saves and returns an ID. That path contains no configurable
  lifecycle callback. `edit` also reaches the transition path, so a wrapper around
  `done` alone would not cover every completion path.
- `src/commands/mod.rs::transition` distinguishes a fresh completion from an
  already-done retry and recurring-occurrence recovery. Follow-up delivery cannot
  infer a new completion merely from a successful command exit. The investigation
  must inspect save and failure boundaries before choosing a delivery contract.
- tasks-5caeae is done. `docs/specs/2026-09-09-periodic-design.md` section 2.3
  explicitly settled recurrence as durable model fields independent of hooks.
  The old note on tasks-d40e8e describes a historical fork, not an open prerequisite.
- The checked-in `.githooks/pre-commit` runs repository gates; it is not a task-event
  dispatcher. The current close path does not observe arbitrary Git merges.
- The source `mindful:thought:ec1726a8595c47f78ddaad57f65eb89e` was preserved.
  Both the typed reference and the documented thought-ID form returned no matching
  thought from the local Mindful CLI. The original task body is the available capture.
- Local open-task searches found no existing research task answering this question.
  Agent claim/closure-policy work in tasks-2fa8c6 concerns ownership; it does not
  establish a project-configured follow-up event contract.

## Constraints

Retain recurrence, JSON output, claims, retry behavior and atomic record-write
semantics. A prompt to consider follow-ups does not authorize arbitrary task writes
or external actions. A merge and a task completion have different producers and
may need different integration points. Any eventual native hook design must settle
configuration trust, payload, execution failure, retries and reentrant task writes.
This investigation needs no shared host resource or global hook changes.

## Alternatives

1. Make the follow-up question an explicit agent closeout step, with Git integration
   handled separately where needed. This is the current lean for the captured use
   case: investigate coverage before adding machinery.
2. Use an external command wrapper or Git hook. Compare coverage of direct `done`,
   edit-to-done and merges, and identify what bypasses the wrapper. No live install.
3. Add a native post-save task hook. Consider only if the evidence establishes an
   event/delivery requirement the simpler workflows cannot meet; it still cannot
   observe all Git merges by itself.

## Unanswered questions

- The investigator establishes which completion paths, retries and failures a prompt
  must cover, using the existing source and focused fixtures if needed.
- The investigator recommends the smallest sufficient home for the one captured
  workflow. Missing broader use cases remain unknown, not assumed requirements.
- If native execution is still needed, a later design author proposes event/payload,
  failure, duplicate-delivery, trust and reentrancy semantics for user review.

## Proposed decomposition

- **tasks-d4e73d**, P2/s/mid/direct: determine whether completion follow-ups require
  native task hooks; produce an event/coverage table and recommendation. Update this
  brief and add a finding note to **tasks-d40e8e** in the same commit as the result.
- **tasks-d40e8e** remains an idea with a briefed verdict, under the existing work
  discovery lane. Preserve its full body and original source. No speculative hook
  implementation or design child is filed until the investigation establishes need.
