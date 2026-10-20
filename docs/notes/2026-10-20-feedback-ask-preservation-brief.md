# Preserve feedback asks from filing through disposition

## Problem

A report must retain each requested outcome when it is filed, joined to existing
work, or proposed for drop. tasks-fa3525 concerns coverage decisions and carrying
asks; tasks-516ab4 concerns reporting context and provenance; tasks-fb861e concerns
finding existing work before filing. These are related safeguards, not substitutes
for one another. This effort belongs inside the work-discovery lane tasks-46d207.

## Current behaviour and evidence

Checked against checkout `88c242e` on 2026-10-20:

- `src/commands/feedback.rs` scans only open feedback tasks. `src/similarity.rs`
  compares title tokens, recurs on one exact match, and refuses ambiguous matches at
  Jaccard similarity >= 0.6. Ordinary tasks are excluded. A suggestion printed after
  creation cannot by itself prevent the duplicate just created.
- New feedback has no occurrence note; recurrence uses unstamped `append_note`.
  `src/commands/mod.rs::append_stamped_note` and `src/provenance.rs` already resolve
  native session provenance, with warnings for invalid input. A session pointer
  alone does not identify an older origin, a human request, or a particular call.
- `skills/scope/SKILL.md` requires evidence for drops but no explicit per-ask coverage
  map or verified carry. Scope proposes drops; it does not execute them.
- tasks-fa3525 attaches the audit, corrected labels and rerun scripts. The original
  71 judged reports had 11 labelled lost and 9 declined; its latest follow-up reports
  8 lost at drop time after correcting three judgments. Four unresolved losses were
  carried in a later follow-up; that is a different measure. The report text held
  all asks in 68/71 cases. These are audit findings, not a fresh replication.
  Notes disagree on one correction's delay (7.5 versus 13 minutes); a rerun must
  derive the timestamps and include the entire scope pass.
- tasks-fb861e reports 47 links to earlier feedback and 11 with only an earlier
  ordinary task. These counts motivate evaluation; they do not establish the
  precision of a looser or semantic matcher.
- Related work already exists: hq-c411df covers reader reachability and parent
  threads; lore-c01bc8 covers tool-level capture of distinct user asks;
  obs-c5c599 covers report-to-session/task/stage tracing; ops-f4950c verifies the
  remaining historical declined/pending/moot judgments. Their records were read;
  their code, original transcripts and private evidence packets were not re-audited.

## Constraints

Keep tool-level summaries useful when transcripts disappear; retain source pointers
without copying unrelated private context. Separate reporting session, origin session,
observed occurrence and later discussion. Preserve explicit, inferred, ambiguous and
unavailable evidence states. Native provenance must not become claim authority.

Keep the feedback matching and output contracts in
`docs/specs/2026-09-03-feedback-design.md` unless an approved design changes them;
its old no-auto-commit wording is superseded by
`docs/specs/2026-10-10-record-commit-design.md` and current code. No fuzzy match may
silently merge distinct asks. Ordinary tasks currently cannot accept `--recur`.
New command or option rows land in ops first. Keep claims, stale-copy checks,
fixed-root writes, review gates and drop approval intact.

Existing archive/trace/capture reports are coordination inputs, not completed
prerequisites or promises to deliver this work. Do not duplicate them or repeat the
whole historical audit as prerequisite research. No shared resource is declared in
this project's `[needs]`; a design needing one must propose its vocabulary.

## Alternatives

1. **Verified workflow plus existing provenance primitives — current lean.** Map every
   ask to a quoted commitment and distinguish delivered, retained, declined and still
   undecided. Write and verify a necessary carry before an approved drop; on failure
   keep the report open. Stamp each new occurrence and recurrence, retain a short
   summary and a visible user-relayed indication, and retrieve context where needed.
   This can deliver useful safeguards before changing CLI matching.
2. **Mechanical carry and richer occurrence metadata.** A drop-with-carry operation
   could enforce the write-before-close order. Decide partial failure, idempotent
   retries, cross-project ownership, and the user's ruling before introducing it.
   Hook-provided call anchors are an unverified option, not an assumed capability.
3. **Broader candidate suggestions at filing.** Compare local lexical ranking across
   ordinary and feedback tasks with the current matcher and agent-assisted search.
   Consider semantic infrastructure only if the bounded comparison justifies it.
   Decide when suggestions arrive, whether creation has already happened, and how a
   filer safely records an ask on ordinary work.

## Unanswered questions

- Which carry mechanism gives a checkable no-loss guarantee with the least machinery,
  and how is a user-relayed ask visibly marked and held for a ruling? The design
  follow-up must settle this, including a declined ask reaching the user as a
  question rather than being called covered.
- What occurrence record preserves new/recurrent provenance, an older origin and a
  useful summary? The design must show live and archived retrieval, parent-thread
  handling, and another-host retrieval or an explicit limitation, coordinating with
  hq-c411df. A timestamp/text match must remain labelled inferred.
- Which bounded candidate list finds paraphrases without drowning the filer in
  unrelated tasks? The research follow-up must measure retrieval and false matches
  on historical-at-filing records, with known related-but-distinct controls.
- How will improvement be measured? The design must retain a post-change blind
  rerun with before/now labels, full-pass snapshots and later reports that cite the
  original id. Agree ownership with obs; ops-f4950c's historical correction pass is
  different work. Do not present either raw or corrected baseline as settled ground
  truth.

## Proposed decomposition

- tasks-9b1c2b — P2/m/high/planned goal under tasks-46d207, retaining the three
  ideas as children.
- tasks-441019 — P2/m/high/planned: design feedback ask preservation and occurrence
  provenance. Wakes tasks-fa3525 and tasks-516ab4. It must produce reviewed design
  and plan artifacts before implementation; the brief is not design approval.
- tasks-a9744c — P2/s/mid/direct: evaluate candidate search on at most 12 historical
  cases, with known distinct-ask and no-match controls. Wakes tasks-fb861e.

Each idea receives a dependency edge and each follow-up must write its findings
back with `note --no-reviewed` before its result branch merges. The two follow-ups
need no dependency on one another; search quality does not gate ask preservation.

tasks-236f4c is a separate direct fix: recovery routes already exist under the tasks
skill's “Process and workspace” section (introduced in `f62e840`), while the warning
and Record commits paragraph point to unnamed “record-commit routes.” Give that
existing guidance a stable heading and link/name it precisely. Coordinate with
tasks-dd1225's project-root warning fix without absorbing it.
