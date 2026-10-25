# SDD ledger — plan: docs/plans/2026-10-25-creation-occurrence-provenance.md

Spec: docs/specs/2026-10-25-creation-occurrence-provenance-design.md (revision 4, 00f58ea), read.
Worktree: .worktrees/tasks-053e89, branch tasks-053e89, from main d5b5472. No setup recipe; cargo builds in-tree.
Step children: T1 tasks-2170ff, T2 tasks-48cd3b, T3 tasks-3f2fe2, T4 tasks-58d20b, T5 tasks-b11cfd, T6 tasks-4d39db, T7 tasks-f42413.

## Pre-flight scan

| Pair / task | Produces vs consumes | Found |
|---|---|---|
| T1 ↔ T4 | T1 rows: add/feedback --origin, --origin-session; edit --origin, --no-origin; list/search/ready --origin (+none, repeatable). T4 parser: FieldArgs.origin (flattened in add at cli.rs:528 and edit at cli.rs:400), Add.origin_session, Feedback.origin/origin_session, FilterArgs.origins (flattened in ListArgs used by list+search, and Ready) | consistent: FieldArgs/FilterArgs flatten sets match the row sets, same as --process rows (cli.toml 372/419/450/479/545/559) |
| T2 ↔ T3 | T2 Origin, NoteOccurrence, Note.occurrence, Task.origin; T3 append_occurrence_note consumes them | consistent |
| T2 ↔ T4 | T2 Origin::combine, parse, validate_session_key; T4 recur_origin, --origin-session validation | consistent |
| T3 ↔ T4 | T3 append_occurrence_note(origin, origin_session) passes None, recur_into stamp; T4 fills origin args, recur_origin in recur_into | consistent; T4 must keep T3's stamp-before-guarded_update |
| T3 ↔ T4 README | T3 lifecycle + creation provenance paragraph; T4 origin sections | distinct sections; no conflict |
| T5 ↔ T6 | T5 skill text; T6 exercises it | T6 failure loops back to T5 text (plan says so) |
| T6 ↔ T7 | T7 wake notes after T6 | consistent |
| T1 self | rows + ops gate + commit + integrate + goal note | consistent |
| T2 self | tests in format.rs vs Produces | consistent; test 5 is a prose stub, implementer writes it |
| T3 self | Files list tests/move.rs, review.rs, attachments.rs for positional asserts; tests named in Step 1 vs Step 2 filters | Step 2 filter `creation_note` matches add_writes_one_stamped_creation_note, feedback_outside_projects_writes_creation_note, add_into_other_project_writes_creation_note; fine |
| T4 self | Step 1 adopt makes parser_surface_equals_table red until Step 4 | intended, consistent |
| T5 self | docs only, just check | consistent |
| T6 self | "from a real Claude Code session (the implementing session)" | see ruling R1 |
| T7 self | filings + wake notes + tasks done | consistent |

Ruling: R1 Task 6 runs as dispatched subagents in this Claude Code session, not inline in the controller — filing calls by one subagent, retrieval by a fresh one that did not make them; Claude Code subagents run under the parent's CLAUDE_CODE_SESSION_ID (spec §4.4, §7), so stamps still name this session and the retriever must find calls in the session's subagent transcripts, which is the realistic case — cost if wrong: the demonstrations would have to be rerun inline by the controller.

Baseline: `just test` at d5b5472 in the worktree: 1695 passed, 0 failed, 14 binaries, rc=0.

Task 2: dispatched (BASE c15592f), parallel with Task 1 (different repo)
Task 1: implementer DONE — ops-a82d6e, ops worktree .worktrees/tasks-origin-rows, branch feat/tasks-origin-cli-rows, c00aef4..acd8bb8; gate 1910 ok; feedback rows placed after --new (no --process row there). Review dispatched (sonnet).
Task 1: review round 1 accept, no findings (claude-sonnet-5-5). Resuming implementer to integrate.
Task 1: complete (ops commits c00aef4..acd8bb8, merged ops main c3f2294, review clean); tasks-2170ff done on main; ops worktree removed
Note: controller goal note 8842976 landed in the worktree mid-Task-2 (record file only); avoid further worktree commits while an implementer runs.
Task 2: implementer DONE_WITH_CONCERNS b61d1cf (Origin::combine #[allow(dead_code)] until Task 4 caller; empty provenance {} rejected per existing test; tests use notes.last(); extra test origin_combine_follows_the_rule). Review dispatched (opus).
Task 2: review round 1 revise (claude-opus-5-5): Important 1 — edit guard notes_unchanged_or_trimmed (src/commands/edit.rs:84-92) ignores note.occurrence.
Task 2: minor (deferred): two error texts for a bad origin (serde variant message in continuation vs Origin::parse in frontmatter)
Task 2: minor (deferred): occurrence_keys_reject_bad_forms asserts is_err() only, not which rule refused
Task 2: minor (deferred): no test of validate_task rejecting an in-memory bad occurrence, nor of the JSON shape (origin after agent, flattened note keys omitted when absent)
Task 2: minor (deferred): #[allow(dead_code)] on Origin::combine until Task 4's recur_origin caller
Task 2: fix round 1/5 (1 addressed, 0 open — edit guard ignores occurrence; commits b61d1cf..d61191f); re-review claude-sonnet-5-5 accept
Task 2: complete (commits c15592f..d61191f, review clean); tasks-48cd3b done
Task 3: dispatched (BASE 3b9877b, sonnet); tasks-3f2fe2
Task 3: implementer DONE 7ea7dcd (also touched tests/refs.rs, tests/external_docs.rs editor scripts; concerns: no equal-ids root test, no feedback --pretty test, 2.2s sleep). Review dispatched (opus).
Task 3: review round 1 revise (claude-opus-5-5): Important 1 — tests/cli.rs:805 notes[1] unshifted in lifecycle_provenance_conflicts_warn_without_becoming_notes, park-under-TASKS_SESSION check lost.
Task 3: minor (deferred): tests/refs.rs editor_saves_validate_added_entries notes=false branch no longer covers a notes-less record (both insert before ## Notes)
Task 3: minor (deferred): feedback_outside_projects_writes_creation_note does not assert from:unregistered tag
Task 3: minor (deferred): resolve_root_from equal-ids case untested
Task 3: minor (deferred): detail note stamp set by hand at feedback.rs:279 duplicates append_stamped_note
Task 3: minor (deferred): resolve_root_from re-runs resolve_from and relies on .ok() (signature plan-mandated)
Task 3: minor (deferred): recurrence test sleeps 2.2s
Task 3: fix round 1/5 (1 addressed, 0 open — unshifted notes[1] park assertion; commits 7ea7dcd..2fe1855); re-review claude-sonnet-5-5 accept; controller confirmed the len()==2 block above is the pre-park [created, started] state
Task 3: complete (commits 3b9877b..2fe1855, review clean); tasks-3f2fe2 done
Task 4: dispatched (BASE d889a41, sonnet); tasks-58d20b
Task 4: implementer DONE_WITH_CONCERNS c5b46a9 + merge 7bc9d02 (main into branch) + d8caa6f (enum baselines for new --origin rows, demanded by a test from main). Concerns: no separate RED for correction_supersedes_a_mistaken_label / editor_preserves_occurrence_keys; pretty note labels `origin X` / `origin session K`.
Ruling: R2 accept the implementer's merge of main (73d3131) into the branch at 7bc9d02 — ops's cli.toml already declares the `docs` command that landed on main via tasks-a8e28e, so the adopted table cannot equal this branch's parser without it, and the branch must absorb main before integration anyway; the merge was clean — cost if wrong: one extra merge commit in history; a rebase would have rewritten reviewed commits.
Task 4: review round 1 revise (claude-opus-5-5): Important 2 — origin_filter_spans_projects never runs --origin none across a project without origin data; editor_preserves_occurrence_keys compares parsed JSON not bytes.
Task 4: minor (deferred): README omits §4.3 decomposition-child sentence, rule 1's tail (labels before/after correction are history; recurrence still combines), explicit "labels speak again" after --no-origin
Task 4: minor (deferred): README omits §4.9 "verify one host end to end; others by same install path covered"
Task 4: minor (deferred): Focus 1 "unknown identity" half untested (origin_survives_unknown_identity covers conflict only; no assert harness_session absent under no identity)
Task 4: minor (deferred): no test for feedback --origin-session <bad>
Task 4: minor (deferred): no test that edit --editor with --origin/--no-origin is a usage error
Task 4: minor (deferred): no test of pretty show note-header origin / origin session labels
Task 4: minor (deferred): `use crate::model::Origin as Asker` alias in feedback.rs
Task 4: minor (deferred): add parses --origin twice (Attribution::parse and apply_fields)
Task 4: minor (deferred): recur_origin_follows_the_table misses (Agent,Agent) and (Mixed,Agent)
Task 4: fix round 1/5 (2 addressed, 0 open — cross-project none test; byte-stable notes test; commits d8caa6f..4346b8c); re-review accept (dispatched sonnet, self-reported claude-opus-5-5; note records harness only)
Task 4: complete (commits d889a41..4346b8c incl. merge 7bc9d02, review clean); tasks-58d20b done
Task 5: dispatched (BASE be0404d, opus); tasks-b11cfd
Task 5: implementer DONE fed20f9; concerns (its calls): retrieval for every scope member; outcomes as optional '; origin: …' audit-note segment; pickup 'ask only what evidence leaves'; dropped 9-of-72 and pasted-review examples; skill names hq-c411df/obs-d98663. Review dispatched (opus).
Task 5: review round 1 revise (claude-opus-5-5): Important 1 — scope skill's origin correction write sits in §2 (scope:161-166), outside §6 revalidation; own edit trips §6 updated check; relation of evidence note to "exactly one audit note" unclear.
Task 5: minor (deferred): re-scope audit-note template (scope:239) lacks the [; origin: <stage outcomes>] segment
Task 5: minor (deferred): re-scope table drop row (scope:232) says "plain note", contradicting --stamp
Task 5: minor (deferred): pickup (tasks skill step 3) has nowhere to record stage outcomes (no audit note at pickup)
Task 5: minor (deferred): dangling "it" in pickup reading rule — write "the record"
Task 5: minor (deferred): "So are two recurrences…" (scope:133-135) reads as listed, not ambiguous
Task 5: minor (deferred): line wrap over width at scope:206-207
Task 5: minor (deferred): judgment call — retrieval for every scope member incl. user (reviewer: follows §4.8, not a finding)
Task 5: fix round 1/5 (1 addressed, 0 open — origin correction moved to §6 write phase, audit note carries evidence; commits fed20f9..8aba3ab); re-review claude-sonnet-5-5 accept
Task 5: complete (commits be0404d..8aba3ab, review clean); tasks-b11cfd done
Task 6: filer subagent dispatched (sonnet) — cases A, B1-B4, C1-C4, D1-D2 in $WORK_ROOT/tasks/tasks-4d39db/demo; retriever dispatch follows
Task 6: filer DONE — A own-0677b0, B1 own-249b63, B2 own-00f6e1, B3 own-c77db0, B4 own-c44636, C1/C2 own-0465cd, C3 frs-9a1c41, C4 own-2a7ce9, D1 own-fac412, D2 own-df3b0c; relay-guard refused 2 $T-variable calls (A's first attempt). Retriever dispatch next (opus).
Task 6: retriever (opus, blind) outcomes all as expected (see notes on tasks-4d39db); report attached sha256 ae3aa727…475f2a; hq feedback hq-4d77cc (subagents dir) and hq-34163a (Codex parent-thread path).
Ruling: R3 the retriever's five open cases (hook-refused call, stderr-capture under Claude Code's merged stdout, subagent transcript location, api-request-blob duplicates, time fit after the note) are filed as a follow-up task instead of edited into skills/scope on this branch — no demonstrated expectation failed, which is the plan's trigger for a fix, and further skill edits would need a fresh review round — cost if wrong: retrievers before the follow-up lands may rule differently on those five cases.
Ruling: R4 Task 6 commits no code, so its task review is the controller's comparison of each recorded outcome against the plan's Step 1-5 expectations (all met), and the final whole-branch reviewer receives the attached report — a separate task reviewer would re-read the same report with less context — cost if wrong: a misread outcome surfaces only at the final review.
Task 6: complete (no code; records only; outcomes as expected); tasks-4d39db stays open until the final review, then its scratch dir is disposed and it closes.
Task 7: implementer DONE except done refused (depends on tasks-4d39db); filed lore-1ae37a, tasks-c96737 (rollout, child), tasks-e407a1 (pilot, child, depends on rollout); wake notes on tasks-516ab4, tasks-2e7d03. Controller closed tasks-4d39db (scratch released, attachment hash matched) then tasks-f42413.
Ruling: R5 Task 7's task review is the controller's check of the filed records and notes against the brief (titles, bodies, parent, dependency, --no-reviewed wake notes all as specified) — the task wrote only tracker records with brief-given text — cost if wrong: a record-text slip found later, fixed with tasks edit.
Task 7: complete (records only)
Final review: dispatched (opus) over 73d3131..f74471f with ledger minors and rulings to triage
Final review round 1 (claude-opus-5-5): with fixes; no Critical/Important; agrees with R1-R5; proposes 2 deferred minors as fix-before-merge.
Ruling: R6 confirm re-grade of Task 5 minor "re-scope audit-note template (scope:239) lacks [; origin: <stage outcomes>]" to Important — a re-scope pass following the template drops the retrieval outcomes and correction evidence the spec requires it to record — cost if wrong: one extra skill-line edit and re-review.
Ruling: R7 confirm re-grade of Task 5 minor "re-scope drop row (scope:232) says plain note" to Important — a literal reader writes an unstamped drop proposal, losing the session pointer §4.8 asks for — cost if wrong: one extra word change and re-review.
Ruling: R8 keep every other deferred minor deferred, as the final reviewer triaged; the Task 2 dead_code minor is already resolved (no allow on Origin::combine) — cost if wrong: small test or wording gaps persist until their follow-ups land.
Final: minor (deferred): pretty show note rows omit root_session (src/show_text.rs:605-633) though the scope skill says to take it from show (JSON has it)
Final: minor (deferred): feedback "origin unchanged" warning (plan-mandated text) tells the reporter to tasks edit the owner's record
Final: minor (deferred): ../../README.md#origin-whose-ask-a-task-is links do not resolve from installed skill dirs (existing pattern); tasks skill paraphrase of the reading rule is a second copy
Final: minor (deferred): no CLI test that tasks check rejects an invalid origin / continuation key
Final review: corrective round 1/5 (2 addressed, 0 open — re-scope template origin segment; drop row --stamp; commits 29b49c2..aad5494, fix a62c1a7); re-review claude-sonnet-5-5 accept.
Deferred minors filed: tests -> tasks-9c0db1; surfaces/README -> tasks-33f80c; skill wording/pickup recording/links -> note on tasks-87eb2c.
