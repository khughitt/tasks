# Scope Skill Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Status:** approved; implementation in progress (2026-09-13).

**Goal:** Ship a deliberate `/scope` pass that turns ideas into supported next actions, briefs, questions, shelves, or drop proposals, and demonstrate it on real Prism work.

**Architecture:** One self-contained `skills/scope/SKILL.md` drives the existing CLI. It gathers evidence before assigning verdicts, preserves existing parents and sources, and records enough context for another person or agent to continue. No Rust changes, clustering command, helper framework, or new dependency.

**Tech Stack:** Markdown skills, the installed `tasks` CLI, git, existing project evidence, and fresh-context agent trials.

**Spec:** `docs/specs/2026-09-12-scope-pass-design.md` §§4–6. Goal: `tasks-019c60`; this piece: `tasks-0d50ff`. CLI prerequisite `tasks-470e8c` is done; its implementation is an ancestor of this plan on `scope`.

## Global Constraints

- Continue in `.worktrees/scope/` on branch `scope`; this is the same design session. Paths in this document are relative to that worktree unless explicitly qualified. Show the user paths relative to the relevant main checkout.
- Pin the tasks worktree path before visiting Prism. Every read/write of `tasks-...` tracking records from another checkout uses `tasks -C <tasks-worktree> ...`; otherwise prefix routing would update the registered tasks main checkout. The Prism pass has its own independently pinned root.
- Explicit ids are the batch, exactly. Otherwise select one coherent cluster of roughly 3–5 eligible ideas; at most three briefs or draft specs per pass. The ceiling is not a quota.
- `scoped` means the approach is established and correctness has a check. `briefed` ideas stay `idea`; writing a brief does not commit implementation.
- Unscoped means the current checkout; `--project` means the registered root. All later member commands and evidence reads use that fixed root. Do not silently route foreign ids out of an unscoped batch.
- Revalidate existing ideas immediately before their first write: unchanged `updated`, still `idea`, no live claim. Changed members receive no edits or scope note. The check is not a lock; propagate CLI failures and report partial work honestly.
- Preserve existing parent links, source links, and notes. Create no duplicate goal, brief, spec, or research task on a rerun. Drops are proposals only; no `drop`, `--parallel`, or hand edits to task records.
- Required skill-testing background: superpowers:writing-skills. Capture actual baseline behavior before authoring the skill, then repeat the same scenarios with it. Trial transcripts stay in ignored scratch; retain a concise evidence note in `docs/notes/`.
- Real acceptance writes only notes/specs and task records in a dedicated Prism worktree. No feature implementation, live Prism checkout edits, merges, pushes, or runtime skill-install changes are part of this plan. The installed CLI already supports the required operations.
- `tasks start` the plan-step record before implementation, `tasks done` in its commit, `tasks check` before every commit. Run `just check` for skill/docs changes and one `just gate` at final closeout. Do not add Rust tests that assert prose strings.

## Deliverables

| File | Responsibility |
|---|---|
| `skills/scope/SKILL.md` | Complete reusable scoping procedure, including output shapes |
| `skills/tasks/SKILL.md` | Discovery and handoff to brainstorming |
| `README.md`, `AGENTS.md` | Invocation, installation examples, and repository layout |
| `docs/notes/2026-09-13-scope-skill-validation.md` | Reproducible trial inputs, observed failures, results, and real-pass evidence |
| Scope-pass spec and this plan | Accurate completion claims and links to evidence |
| Prism acceptance worktree | Actual cluster handoff, kept separately for review |

There are two implementation tasks: an independently usable, tested skill; then real-project acceptance and any corrections it demonstrates. Do not split each skill paragraph into its own task.

### Task 1: Write and validate the scope skill and its discovery paths

**Files:** Create `skills/scope/SKILL.md` and `docs/notes/2026-09-13-scope-skill-validation.md`; modify `skills/tasks/SKILL.md`, `README.md`, and `AGENTS.md`.

**Interfaces:** Consumes `tasks list/show/root/tree/edit/add/note/shelve`, the existing complexity rubric, and `curate`'s root/revalidation discipline. Produces `/scope [<id>... | --tag <tag>] [--project <prefix>]`, five verdicts, and linked handoff documents. Task 2 loads this file explicitly; a global install is unnecessary for acceptance.

- [x] **Step 1: Prepare the trial packet and record a baseline before writing the skill.**

Use an ignored, plan-specific scratch directory. Give fresh agents the existing tasks/curate guidance, the following scenario facts, and the request below; do not give baseline agents the proposed scope skill, its design, this plan, or the assessor's answers. The main packet proposes concrete commands/artifacts against supplied snapshots so the mid-pass change is deterministic. Step 3 also executes the three-verdict packet against real scratch-project records; Task 2 remains the real-project acceptance pass.

Request:

> Review this idea batch so other people can pick up the next work. We have one short pass, the backlog is large, and I'd like useful design notes today. Choose the batch, gather the supplied context, and return the commands and documents you would actually write, followed by the user summary. Use the supplied first-show and pre-write snapshots in sequence. Do not ask me to choose a cluster.

Use prefix `demo`, current checkout `.worktrees/review`, registered main checkout `demo-main`, and these records (all are `idea` unless stated). Creation order is the table order; each supplied first `updated` is `2026-09-01T00:00:00Z`. Names are trial-relative, not real repository paths.

| Id | Facts available on first show and evidence read |
|---|---|
| `demo-a10001` | Edited-profile indicator; parent `demo-b10001`; tag `profiles`; latest note is a normal capture note; source `docs/notes/profile-feedback.md`; evidence leaves both saved-snapshot and dirty-key approaches viable |
| `demo-a10002` | Rename without loading; parent `demo-b10002`; tag `profiles`; evidence names existing rename API and one integration boundary to verify |
| `demo-a10003` | New profile button; no parent; tag `profiles`; source same as a10001; evidence leaves creation/reset semantics unresolved |
| `demo-a10004` | Defaults/reset presentation; no parent; tag `profiles`; evidence says neutral and shipped defaults differ, so product decision remains |
| `demo-a10005` | Profile panel refresh; no parent; tag `profiles`; first show is unchanged idea, but pre-write show is `doing` with a live claim and later `updated` |
| `demo-a10006` | GPU estimator; tag `performance`; no relation to profile work |
| `demo-a10007` | Existing profile question; parent `demo-b10004`; latest note `scope: briefed; brief: docs/notes/profile-brief.md`; that goal and its brief already contain open research for the question |
| `demo-a10008` | Another profile question; latest note `curate: duplicate; proposal: drop, demo-a10007` |
| `demo-a10009` | Profile integration discovery; latest note is a finding after an earlier scope note; existing brief `docs/notes/integration-brief.md`, source-goal `demo-b10003`, and open research already cover its unanswered question |

Supply these fixed evidence excerpts in the packet, not only titles:

```text
docs/notes/profile-feedback.md: The indicator could compare the current profile to a
saved snapshot or track edited keys. Behavior after profile reload is not decided.
API excerpt: rename_profile(name, new_name) delegates to store.rename(name, new_name).
No panel caller or integration test for renaming an unloaded profile is supplied.
Reset note: The neutral look and the shipped defaults have different values. Which
one each reset level should load remains undecided.
docs/notes/integration-brief.md: demo-a10009 awaits demo-c10001, an open research task
to establish whether the integration hook reports the active profile name. No answer
has landed. Goal demo-b10003 has this document as its source.
docs/notes/profile-brief.md: demo-a10007 awaits demo-c10002, an open research task
to reproduce what profile loading retains. Goal demo-b10004 contains both records.
```

Give the same immutable excerpts to both trial variants. First and repeated `show` differ only for a10005. Parent b10001 and b10002 are separate open goals.

After the default pass, send this follow-up to each agent:

> Now explicitly scope demo-a10007 and demo-a10009. Existing research has not finished. Reuse the supplied documents and task graph; return only the additional changes you would make.

Run three fresh-context baseline trials and save exact outputs. Go beyond three only when those first three disagree; resolve the specific disagreement rather than increasing every case count. Assess manually against the table below. Record actual failures and quotations; do not invent a failing baseline if the agents already comply. If no failure appears, add the concrete project ambiguity the baseline omitted and repeat before authoring new guidance. Do not weaken the acceptance contract to manufacture success.

| Check | Required result |
|---|---|
| Selection | Default excludes a10007/a10008, admits later-note a10009, selects related profile work without asking the user; explicit follow-up can revisit a10007 |
| Root | Unscoped commands/evidence use `.worktrees/review`; a separate `--project demo` variation uses `demo-main` via the parsed `root` field or pretty output |
| Commitment | Unsettled alternatives remain ideas with a brief; established bounded work may become todo with a check; investigation is not automatically low complexity |
| Relationships | a10001/a10002 retain different parents; only unparented members can acquire the new cluster goal |
| Concurrent change | a10005 receives no write or scope note after the changed pre-write snapshot |
| Handoff | Brief has evidence, constraints, alternatives, unanswered questions, and decomposition; research names question/start/bound/result/idea wake notes |
| Rerun | a10009 reuses its existing goal/brief/research; explicit follow-up does not create another copy |
| Summary | Supported per-member verdicts and document paths; only actual user decisions surfaced; no drop executed |

The expected next action is not a predetermined feature choice. Judge whether the decision follows the supplied evidence and contract.

- [x] **Step 2: Author one self-contained skill from the spec and observed failures.**

Use this frontmatter:

```yaml
---
name: scope
description: Use when asked to scope captured ideas, review an idea backlog for next actions, or run /scope.
---
```

Use numbered sections in operational order. The following is the implementation contract; trial failures determine which wording needs emphasis:

1. **Select and fix the root.** Distinguish default/tag selection from explicit ids. Show the real discovery commands: `tasks list --status idea [--tag <tag>] [--project <prefix>]`, then `tasks -C <root> show <id>` for latest notes. List rows lack notes, so titles alone cannot establish eligibility. Empty pools end with a short result. Foreign ids or a missing unscoped project require correcting invocation before writes. `tasks --pretty root <id>` prints a path; JSON requires `.root`. Never call registered-root resolution for the unscoped worktree case. Choose largest coherent group, oldest on a tie; take roughly 3–5, or oldest three singletons if no pair exists. Fewer eligible ideas yield a smaller pass. Explicit ids bypass prior-note exclusion, not status/claim checks or the document ceiling.
2. **Gather evidence.** First `show` supplies `task.updated`, body, links and notes, plus top-level claim and relationships. Read source, parent/tree, relevant code/specs/history and duplicate candidates in the same checkout. Mindful sources use `mindful --json show` after confirming local CLI usage; unavailable context becomes a named unknown, never an invented finding. Confirm or narrow the provisional cluster; unrelated members are left untouched. Before authoring, look for both an existing brief and an attached draft spec so either handoff can be updated in place.
3. **Choose the next action.** Include the spec's five-verdict table and exact note form `scope: <verdict>; <what changed>[; brief: <path>][; proposal: <text>]`. `scoped` rewrites why/done/where and sets todo, priority, size and complexity by the existing rubric. `briefed` stays idea. `question` retains status and collects questions in one body heading. `shelved` uses `tasks shelve <id> "<wake condition>"` plus the scope audit note. `drop` changes only a proposal note with a supporting commit/id. Unknowns do not justify invented priorities, automatic low research ratings, or implementation tasks masquerading as decisions.
4. **Write the handoff.** Use the six brief sections in §4.4 under `docs/notes/`; use `docs/specs/` and `Status: draft` only for one reviewable design, attached through `--spec`. Reuse a shared parent where present. Otherwise a new priority-2 todo goal parents only previously unparented members; existing goal members remain associated by document and note. Preserve their sources. Do not rewrite shared goals incidentally. Reuse existing research whose question still stands. If a proposed new goal would have no member or follow-up children, do not create an empty ready goal merely to hold a document; retain the handoff association through member notes.
5. **Make follow-ups answerable.** A research body has five explicit fields: Question; Where to start; Bound; Expected result; Ideas it wakes. Include a concrete example using the shape below. File a high-complexity design task only when unresolved questions require design; make it depend on the prerequisite research. A single measurement question need not acquire a design task too.
6. **Revalidate, write, record.** Re-show immediately before each member's first write. Use top-level `claim.live`, `task.status`, and `task.updated`. Skip changed/claimed/non-idea members; do not create artifacts that falsely say skipped members were processed. Documents may identify them as related context. Use the CLI for records and explicit paths for documents; do not claim/reset ideas via `start` as a scoping shortcut. Preserve a complete record of successful operations if a later command fails; never report skipped or failed operations as completed. Do not create a goal or doc if all intended members were skipped. Exactly one scope audit note per successfully processed member; the shelf command's own note is separate. There is no per-row approval checkpoint.
7. **Summary and reruns.** State cluster/reason; each member's verdict or skip reason; paths; then actual drop proposals, questions and shelves/wake conditions, omitting empty groups. A later research finding must note every waiting idea as part of completion so it returns to the default pool. Explicit reruns update the existing handoff; do not replace sources or make a fresh goal because the old one was not part of the selected batch.

Concrete research-body example:

```markdown
Question: Does the existing rename API require loading the profile first?
Where to start: the rename_profile implementation and its panel caller, linked from the brief.
Bound: Trace that call and reproduce one rename of an unloaded profile; no new UI implementation.
Expected result: Record the observed behavior and a recommendation on this task and in the brief.
Ideas it wakes: On completion, run tasks note on each named waiting idea with the finding, in the same commit as this result.
```

The actual task substitutes repository paths and waiting idea ids found during the pass. Preserve the full source material when shortening task bodies. These are instructions for the agent, not CLI-enforced task kinds.

- [x] **Step 3: Repeat the same trials with the skill, then exercise the remaining verdict branches.**

Run three fresh-context trials of the identical packet with the new skill loaded. Check every result against every applicable row; keep the baseline/green variants comparable. Fix observed misunderstandings and rerun affected cases. Reading the skill back is not a passing application test.

Also execute one small explicit-id packet in a real ignored scratch project per wording revision. Use a disposable XDG_CONFIG_HOME and XDG_STATE_HOME for every fixture command, initialize with tasks init --prefix demo, commit its seed data, and create a .worktrees/review linked worktree. Never register this fixture in the real user registry. Create ideas through the CLI and extract add results from the top-level id field; add returns {id, action}, not a task object. One idea has a real fixture commit proving it landed (proposal only), one waits on a named external capability (shelf with that wake condition), and one requires an unprovided personal preference with no useful design alternatives (question in body). Run the skill unscoped from the review worktree with these explicit ids and inspect the resulting files and show output, not a proposed transcript. Expect three supported verdicts, exactly one scope note per member, an additional shelved: note from shelve itself, no executed drop, and no forced brief/spec. Verify fixture main is unchanged, root defaults to a JSON object pointing at registered main while unscoped writes stayed in the review worktree, and list rows lack notes so verdict/eligibility reads used show. Reset to the clean seed in a new trial worktree for each wording revision; keep this packet to one execution per revision.

Record inputs, observed baseline errors, representative output excerpts, green outcomes, and limitations in the validation note. Keep trial transcripts in ignored scratch, not committed reports. Do not introduce a test runner or a Rust test merely to scan skill text.

- [x] **Step 4: Add discovery and design handoff pointers.**

- In `skills/tasks/SKILL.md` step 2, extend “Never pick an idea; scope it first” with `Use the scope skill for a deliberate idea review.` Add one recording-work bullet showing `/scope [<id>... | --tag <tag>] [--project <prefix>]`. In “With superpowers”, state that a design task from a scope brief is where brainstorming attaches and finishes a draft design.
- In `README.md`'s Agent skill section, describe scope as deliberate idea review between capture and maintenance. Add scope symlink examples beside the existing curate examples for both harness directories. These are documentation examples; do not execute global install commands from a development worktree.
- In `AGENTS.md`, add the deliberate `/scope` invocation and the new `skills/scope/SKILL.md` layout entry. Do not make scoping an automatic session sweep.
- Keep curate's existing bounds intact; its `scope:` pending-prefix text already landed with the CLI.

- [x] **Step 5: Check and commit the tested skill.**

Inspect the frontmatter, referenced CLI help and every doc link; run `git diff --check`, `tasks check`, and `just check`. Record the trial evidence on the step task, close that step, and commit the named skill/docs/task files with `feat(scope): add a tested idea scoping skill`. Keep the parent skill piece open for Task 2's real acceptance. The scope-pass design status becomes `scope skill written; real-cluster acceptance pending`, not fully implemented yet. Correct §4.5's known stale “four parts” count to the five fields already enumerated in this same Task 1 commit.

### Task 2: Run real Prism acceptance, correct the handoff, and close the skill piece

**Files:** Modify the skill/discovery text only for demonstrated defects; extend `docs/notes/2026-09-13-scope-skill-validation.md`; update the scope-pass spec, this plan and task records. In the separate Prism worktree, create/update only the pass's selected records, necessary new goal/follow-up records, and its brief or draft spec.

**Interfaces:** Consumes Task 1's tested skill and installed CLI. Produces a reviewable real cluster handoff and rerun evidence. Task 1's behavior tests do not substitute for this acceptance pass.

- [ ] **Step 1: Prepare an isolated checkout of current Prism.**

Resolve Prism through `tasks --pretty root prism-b8b589`; read its `AGENTS.md` and inspect status/history. Recheck the candidate ids with read-only `show`; snapshot current statuses, parents, sources, notes and claims. On 2026-09-13 the five below are ideas. The sixth provisional spec example, `prism-e08ee6`, is already todo and is context only.

Create a fresh Prism worktree with `git worktree add .worktrees/scope-acceptance -b scope-acceptance` from the Prism root (use an unused suffix if it already exists). Run `just setup` if that checkout defines it. Do not change the Prism registry root. Do not copy or overwrite live task records to hide differences between the new checkout and main; document an uncommitted relevant input as a limitation and select current eligible work from the actual checkout.

- [ ] **Step 2: Run one real pass with the skill explicitly loaded.**

Before leaving the tasks worktree, capture its path for tracking commands:

```sh
scope_tasks_root=$(pwd -P)
```

From the Prism acceptance checkout, explicitly load the skill from that tasks checkout and invoke the equivalent of:

```text
/scope prism-b8b589 prism-920f31 prism-8a8eac prism-49a068 prism-ad2b12
```

This is deliberately unscoped: member reads and writes must stay in the Prism worktree, not the registered main. Revalidate before writes. If an example is no longer an eligible idea, skip it; do not restore its prior status for the test. Gather current context and choose verdicts from evidence, not from this plan. A missing source is an explicit open question, not a reason to fabricate a recommendation. Follow Prism's task protocol for the implementation-step tracking, while the idea-review skill itself uses its own no-start/revalidation discipline for selected ideas.

Save the exact resulting user summary and links in the tasks-repo validation note. Record which selected ids stayed ideas, became actionable, or received another verdict; the number of briefs/specs; created follow-up ids; and the evidence for the decisions. Show the concrete handoff paths relative to Prism's main checkout. Check that relevant main-checkout files were unchanged by the pass.

- [ ] **Step 3: Exercise the default exclusion and explicit rerun against the resulting state.**

Read the idea pool and its latest notes to show processed members are excluded by the default rule; do not process another unrelated cluster just to test exclusion. Then run an explicit rerun on the processed ideas that remain ideas. Compare artifacts and task graph before/after: no second goal/document, no duplicate research, unchanged existing parents and sources, retained prior notes. Updates must add evidence or sharpen a question; do not force changes simply to demonstrate a write. If no processed ideas remain ideas, record that fact and use Task 1's explicit-rerun scenario as the existing-brief coverage.

If the pass demonstrates a skill defect, correct that wording, rerun its controlled scenario, and use the explicit rerun for the real handoff. Do not repeatedly add live notes until a transcript looks clean. Record partial operations and repairs honestly. Run Prism's required checks and commit only the pass-owned files; preserve that branch/worktree for user review, without merge or push.

- [ ] **Step 4: Record acceptance and finish the tasks branch.**

Use `tasks -C "$scope_tasks_root" note tasks-0d50ff "<acceptance result>"` with the actual Prism checkout/commit, member ids, summary, and what the skill text got wrong (or an evidence-backed “no correction needed”). Update the validation note with baseline/green counts and the real pass/rerun observations. Do not claim a real pass if only scripted trials ran.

Correct the spec's status to implemented only after these checks exist; the research-field count was already corrected in Task 1. Mark this plan's steps completed from the tree and evidence. Grep README, AGENTS, skills and current specs for stale pending-skill or contradictory workflow claims and fix relevant drift.

Run `just gate` once on the final tasks tree, then close this step and `tasks-0d50ff` with `tasks check` before committing. Inspect `prime` closeout for `tasks-019c60`: close it only when both pieces and acceptance are complete and claim rules allow it. Do not force another session's live claim; record completed pieces and leave the goal for its holder if necessary. Commit as `docs(scope): record real-cluster acceptance`. Retain both branches for review; integration is separate.

## Self-review

| Requirement | Coverage |
|---|---|
| §4.1 selection, exclusion, default/autonomous choice | Task 1 selection contract and baseline/green packet; Task 2 post-pass pool inspection |
| §4.2 same checkout and evidence | Task 1 two-root variation; Task 2 unscoped real worktree |
| §4.3 all verdicts and revalidation | Task 1 main packet plus three-verdict packet; Task 2 actual records |
| §4.4 briefs/specs and parent preservation | Task 1 contract/graph checks; Task 2 artifact and parent diff |
| §4.5 bounded research and idea wake notes | Required five-field body and trial output checks |
| §4.6 reruns | Task 1 existing-brief case and Task 2 explicit rerun |
| §4.7 audit and summary | Exact note shape, pending proposal behavior, saved real summary |
| §5 discovery and protocol | Task 1 docs/pointers; Task 2 drift/status audit |
| §6 skill acceptance | Task 2 real Prism pass, not a substitute CLI test |

No CLI changes or new harness are planned. Scenario identifiers are fixed test inputs; operational roots, source paths and verdicts are discovered from actual project state. Task 2 depends on Task 1.
