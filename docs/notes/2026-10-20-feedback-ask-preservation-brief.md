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
  creation cannot by itself prevent the duplicate just created. Separately, the tasks
  skill names `tasks search "<phrase>" --all-projects` (a literal, case-insensitive
  substring over open titles and bodies, ordinary tasks included) as the duplicate
  check before filing, while the global feedback rule asks for one command, then carry
  on; nothing records whether a filer ran the search.
- New feedback has no occurrence note; recurrence uses unstamped `append_note`.
  `src/commands/mod.rs::append_stamped_note` and `src/provenance.rs` already resolve
  native session provenance, with warnings for invalid input. A session pointer
  alone does not identify an older origin, a human request, or a particular call.
- `skills/scope/SKILL.md` requires evidence for drops but no explicit per-ask coverage
  map or verified carry. Scope proposes drops; it does not execute them.
- tasks-fa3525 attaches the audit, corrected labels and rerun scripts. The original
  71 judged reports had 11 labelled lost and 9 declined. The corrected labels overturn
  three lost judgments, leaving 8. One of the three (ops-11a4a6) left its carry as
  advice in the drop proposal (02:08:49Z on 2026-10-17); the drop executed at
  02:14:17Z, and a later scope note wrote the ask onto ns-80a711 at 02:21:50Z, 7.5
  minutes after the drop and 13 after the proposal. The CSV label and the 12:44 note
  say "13 min after the drop"; the timestamps settle it. The labels count that case
  not lost, yet count asks delivered by later work (ops-075ea7, ops-3b4a6e) as lost, so
  the baseline measures losses after a short window following the drop, not at the
  drop itself; the post-change rerun must fix that window explicitly (the follow-up
  proposes the whole scope pass), and a carry written after the close is still the
  order this work removes. Four losses still unresolved were carried in the earlier
  follow-up; that is a different measure. The report text held all asks in 68/71
  cases. Both lost asks among the 6 user-relayed or mixed reports were the user's own
  words (2/6, against 6/65 agent-observed reports); both samples are small. These are
  audit findings, not a fresh replication.
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

A carry onto a covering task in another project is a write outside the scope pass's
fixed root: ops-fec58a's carry went to tasks-1dd2a0, and hq-872750's covering tasks
are in relay. tasks-91c622 designs cross-project scope writes and already admits the
refile move; a carry's cross-project contract comes from there. In this lane,
tasks-5408fe owns scope and curate follow-through (its child tasks-19d4fe edits the
same scope verdicts), and tasks-22f407 the queue of pending proposals, where a user
ruling would naturally wait.

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
   This can deliver useful safeguards before changing CLI matching. The drop-step
   half and the provenance half are designed separately so the first need not wait
   on retrieval.
2. **Mechanical carry and richer occurrence metadata.** A drop-with-carry operation
   could enforce the write-before-close order. Decide partial failure, idempotent
   retries, cross-project ownership, and the user's ruling before introducing it.
   Hook-provided call anchors are an unverified option, not an assumed capability.
3. **Broader candidate suggestions at filing.** Compare the documented `tasks search`
   check, local lexical ranking across ordinary and feedback tasks, the current
   matcher and agent-assisted search. Consider semantic infrastructure only if the
   bounded comparison justifies it. Decide when suggestions arrive, whether creation
   has already happened, and how a filer safely records an ask on ordinary work.

## Unanswered questions

- Which carry mechanism gives a checkable no-loss guarantee with the least machinery,
  and how is a user-relayed ask held for a ruling? tasks-441019 must settle this,
  including a declined ask reaching the user as a question rather than being called
  covered, and the cross-project carry with tasks-91c622.
- What occurrence record preserves new/recurrent provenance, an older origin, a
  useful summary and a visible user-relayed marker? tasks-a58a90 must show live and
  archived retrieval, parent-thread handling, and another-host retrieval or an
  explicit limitation, coordinating with hq-c411df. A timestamp/text match must
  remain labelled inferred.
- Which bounded candidate list finds paraphrases without drowning the filer in
  unrelated tasks? tasks-a9744c must measure retrieval and false matches on
  historical-at-filing records, with known related-but-distinct controls and the
  documented `tasks search` check as a baseline.
- How will improvement be measured? tasks-441019 must retain a post-change blind
  rerun with before/now labels, full-pass snapshots, the loss window fixed above and
  later reports that cite the original id; tasks-a58a90 retains the blind at-filing
  pilot. Agree ownership with obs; ops-f4950c's historical correction pass is
  different work. Do not present either raw or corrected baseline as settled ground
  truth.

## Proposed decomposition

- tasks-9b1c2b — P2/m/high/planned goal under tasks-46d207, retaining the three
  ideas as children.
- tasks-441019 — P2/m/high/planned: design per-ask drop verification and carry.
  Wakes tasks-fa3525.
- tasks-a58a90 — P2/m/high/planned: design feedback occurrence provenance. Wakes
  tasks-516ab4. Split from tasks-441019 on 2026-10-20 so the drop-step fix does not
  wait on cross-host retrieval.
- tasks-a9744c — P2/s/mid/direct: evaluate candidate search on at most 12 historical
  cases, with known distinct-ask and no-match controls. Wakes tasks-fb861e.

Both designs must produce reviewed design and plan artifacts before implementation;
this brief is not design approval. Each idea receives a dependency edge and each
follow-up must write its findings back with `note --no-reviewed` before its result
branch merges. The three follow-ups need no dependency on one another: search quality
does not gate ask preservation, and tasks-441019 judges user relay from report text
until tasks-a58a90's marker exists.

tasks-236f4c was scoped in the same pass but lies outside this cluster: a direct fix
naming the existing record-commit recovery guidance; its body holds the evidence.
