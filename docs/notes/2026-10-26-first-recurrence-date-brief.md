# Choose the first date of a recurring task

## Problem

tasks-adf58d asks for a recurring task whose first occurrence falls on a chosen future date, followed by the existing completion-based cadence. Today a new recurrence is eligible immediately. The current workaround uses a plain deferred task and manually resets/re-defers it after each check.

## Current behaviour and evidence

Source inspection at 0845def (2026-10-26) establishes:

- `docs/specs/2026-09-15-defer-design.md` §2.4 deliberately forbids `every` with `defer`: two dates lacked a precedence rule. `src/format.rs::validate_task` enforces this for parsing and field-setting paths; `tests/cli.rs::defer_refuses_bad_values_and_incompatible_records` asserts refusal for add and both edit directions.
- `src/periodic.rs::due/is_due` derives later due times from `last_done + every`, and `src/query.rs` applies deferral to selection. `src/commands/edit.rs` and lifecycle transitions clear deferral on a status change. These are interacting contracts, so deleting a refusal is insufficient.
- The filing session, claude-code:b0f7fde8-95d2-4d12-a287-5daf820d7b6b, line 592 attempts `add --every 30d --defer 2026-11-26`; line 593 records the validation refusal. Lines 608 and 620-621 record the workaround and creation of this report. Binding is inferred: the unique filing call's output was transformed to the id and action rather than retained as full JSON.
- The user's own turn at line 509 asks for reassessment after one month, repeated if needed. The request for a reusable CLI capability was the agent's remedy, so the task's origin remains agent. No older origin session is named.
- Searches of open work and existing briefs/specs found no first-occurrence design. tasks-157d05 covers dated resumption of parked doing work; tasks-d4e73d covers completion hooks. Neither answers initial recurrence scheduling.

## Constraints

Keep completion stamps truthful: do not simulate a completed occurrence to seed a date. Preserve current default eligibility and completion-based cadence for existing recurrences, explicit early-run rules, status/claim semantics, and JSON contracts. Calendar time must not authorize user-held work. Reuse the work-discovery lane tasks-46d207; no new lane or generic scheduler is needed.

The existing periodic and defer specs are design history to check against code, not authority to silently change behavior. A new CLI option, if chosen, needs its surface rows landed in ops before parser changes and adoption here.

## Alternatives

1. **Lean: permit an initial deferral alongside recurrence.** Reuse the existing date field without adding an option, with explicit rules for never-completed versus previously completed records and for the first start, early start, completion, reopening and cadence changes. This minimizes surface area but changes a persisted invariant and needs a binary rollout assessment.
2. **Dedicated first-occurrence field/option.** Separate initial scheduling from later recurrence. Clearer separation, but adds schema, CLI and adoption work; choose it only if reuse cannot give coherent lifecycle rules.
3. **Keep manual deferred checks.** Preserves current behavior but retains repetitive reset/re-defer work and does not supply the capability the report requests.

The lean is a design recommendation, not an approved specification.

## Unanswered questions

The design author, followed by user review, must settle:

- Can initial deferral be restricted to never-completed recurrences, and what happens after early start, reopen, disabling/re-enabling cadence, or an editor save?
- Which date is displayed in `show`, `list --periodic/--deferred`, `prime` and selection before/at/after the first date? What remains in the record after it is spent?
- Can old readers consume the resulting record, and what upgrade ordering is necessary?
- Does a first run on a calendar date retain today's UTC date boundary while later cycles retain completion timestamps?

These are design questions; no separate measurement or research task is warranted from the current evidence.

## Proposed decomposition

- Reuse tasks-46d207 as the existing parent for tasks-adf58d and its design follow-up.
- tasks-ba4528 — P2/s/high/planned: Design first-occurrence scheduling from this brief. Produce a user-reviewed design and then a user-reviewed implementation plan; no runtime implementation during this handoff. Compare alternatives and specify lifecycle, rollout and test cases.
- tasks-adf58d remains an idea with a dependency on tasks-ba4528. On completion the design task must update this brief and run `tasks note tasks-adf58d --no-reviewed "<approved finding>"` before its branch merges, enabling a later scope pass.

