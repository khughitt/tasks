# Scope skill validation

Date: 2026-09-13

## Method

The controlled packet used nine fixed `demo` ideas, immutable evidence excerpts, an
unscoped `.worktrees/review` checkout, a registered `demo-main` checkout, and a changed
pre-write snapshot for `demo-a10005`. It tested default selection, a `--project demo`
variation, and an explicit rerun of `demo-a10007` and `demo-a10009`. The approved plan's
[Task 1](../plans/2026-09-13-scope-skill.md#task-1-write-and-validate-the-scope-skill-and-its-discovery-paths)
retains the reproducible inputs and acceptance checks. Exact transcripts are temporary
ignored trial scratch; this note retains the findings and representative quotations.

Three fresh agents ran the packet without the skill, followed by three fresh agents with
the first candidate. The first three green trials disagreed only on cross-goal research
reuse, so the wording received one focused clarification and one focused follow-up trial.
Each wording revision also receives one actual CLI packet in a fresh isolated fixture.

## Baseline: 3 trials

All baseline trials missed the contract in the same areas:

- They excluded `demo-a10009` because a brief and research existed, although its later
  finding note made it eligible again.
- They invented verdicts such as `scope: ready`, `scope: decision`, and
  `scope: already covered`. Some used `proposal:` for unresolved decisions instead of
  reserving it for a supported drop.
- They omitted the required six-section handoff, cluster goal, or answerable research
  tasks. One invented behavior: `Done means edits make the indicator visible and
  returning to the saved state clears it.`

The baselines did consistently preserve different existing parents, avoid rerun
duplication, and skip `demo-a10005` after its status, claim, and timestamp changed. Two
of three kept all unscoped commands under the fixed worktree root; one omitted that
discipline on initial reads and writes.

## Initial green: 3 trials

All three candidates correctly:

- admitted the later-note `demo-a10009`, selected the five-member profile cluster, and
  skipped the changed `demo-a10005` without a write or audit note;
- fixed unscoped work to `.worktrees/review` and `--project demo` work to `demo-main`;
- preserved the different parents and sources of `demo-a10001` and `demo-a10002` while
  parenting only unparented members under a new goal;
- used canonical verdicts and audit notes, kept unresolved alternatives as ideas, wrote
  all six brief sections, and shaped bounded research with all five required fields;
- reused existing artifacts on the explicit rerun of `demo-a10007` and `demo-a10009`.

The only disagreement was initial-pass research reuse. One trial duplicated the reload
question already owned by `demo-c10002`, one reused it, and one filed only the distinct
rename investigation. The candidate now explicitly searches open research across
related goals, including ideas excluded from the selected batch, before creating a task.

One focused fresh trial then created only the distinct unloaded-rename research. It
reused `demo-c10002` and `demo-c10001` on rerun, maintained the fixed roots and existing
parents, and skipped the changed member without writing it. No further wording change
was needed.

## Actual CLI packets

The first wording revision ran once against a real ignored scratch project with isolated
`XDG_CONFIG_HOME` and `XDG_STATE_HOME`. The unscoped pass wrote only its review worktree;
the registered main checkout stayed clean. `tasks root <id>` returned a JSON object whose
`.root` pointed to main, while all selected reads and writes remained fixed to review.
The list rows omitted notes, and eligibility and verdict evidence came from `show`.

The actual records passed the three remaining verdict branches:

- `drop`: the idea stayed `idea`, with one `scope: drop` note and an ancestor commit as
  evidence; no drop command ran;
- `shelved`: the task acquired the native shelf note plus exactly one scope audit note;
- `question`: the idea stayed `idea`, gained one `## Open questions` section, and received
  exactly one scope audit note.

`tasks check` passed and no brief or spec was forced for these three singletons. A first
launcher attempt omitted Cargo's bin directory from its isolated `PATH` and failed before
the CLI ran or wrote anything; the corrected environment completed normally.

The clarified wording received a second fresh CLI packet with the same three verdicts.
The inspected records again had one drop audit with the idea retained, one native shelf
note plus one shelf audit, and one question audit with the question in the body. The main
checkout stayed clean; root JSON pointed to main, list rows lacked notes, all operations
stayed fixed to the review root, and the final check reported zero errors and warnings.
Its first writes warned that a retained sibling review worktree held newer copies, an
expected fixture artifact that did not change the chosen root or final records. One note
rendered an escaped apostrophe; that output polish did not affect the verdict or audit
contract.

## Controlled trial totals

Task 1 completed three baseline trials, three initial green trials, one focused green
trial prompted by their sole disagreement, and two actual CLI packets, one per wording
revision. The real acceptance below applies the skill to current Prism code and records; it is
separate from these controlled trials.


## Real Prism acceptance

Explicitly loaded `skills/scope/SKILL.md` from the tasks `.worktrees/scope` checkout,
then ran the unscoped equivalent of `/scope prism-b8b589 prism-920f31 prism-8a8eac
prism-49a068 prism-ad2b12` from Prism `.worktrees/scope-acceptance`. The registered
Prism main was clean at `6941146`; `git worktree add` created the acceptance checkout
from that commit. Its justfile has no setup recipe. Every explicit task command after
root selection used `-C` with the appropriate pinned checkout, including tasks-repo
tracking from Prism. No registry changes or selected-idea starts occurred.

The five first-show and pre-write records were unchanged, unclaimed ideas. The separate
`prism-e08ee6` was already todo and remained context only. Read all open records, both
related goal trees, the profile/reset source and tests, the plugin contract, current
context/reset specs, and relevant ancestor commits. The original Mindful reference
could not be resolved: verified `mindful show --help`, then `mindful --json show
1e2513d2f5ea48609022559f3c687d01` returned `no thought matching`. The brief names this
unknown; the captured source strings and bodies remain intact.

Results at Prism commit `6887eae9d02078914067003b5e23d4326d3f2bbc` on
`scope-acceptance`:

| Member | Verdict | Evidence / next action |
|---|---|---|
| prism-b8b589 | briefed, stays idea | Direct persistence exists; marker lifetime across asynchronous writes/switches remains a design choice. |
| prism-920f31 | briefed, stays idea | Existing inactive rename/delete paths and tests establish backend support; management targeting remains a panel design. |
| prism-ad2b12 | briefed, stays idea | Neutral writes and remove-override differ; existing prism-bf3ae9 already owns the decision. |
| prism-8a8eac | shelved | Existing capture defers New's initial values until reset/autosave decisions, with a 2026-11-10 review. |
| prism-49a068 | shelved | Existing capture defers default presentation until reset/autosave decides base reachability, with the same review. |

One 550-word, six-section brief; zero draft specs, research tasks, promotions, drops,
or question verdicts. New goal `prism-3415ef` parents only the three previously
unparented members. `prism-b8b589` and `prism-920f31` keep `prism-2f0b4b`. New
high-complexity design `prism-e37618` covers only those two profile controls and requires
finding notes on both waiting ideas when complete. Existing `prism-bf3ae9` is reused for
reset, with its wake-note instruction in the brief; neither it nor either existing goal
was rewritten. No new research is warranted for behavior already established by code
and tests. Every member received exactly one scope audit note; shelves also received
the native shelf note. Original bodies, sources and prior notes were checked intact.

Current artifacts after integration, relative to Prism's main checkout:

- `docs/notes/2026-09-13-profile-editing-brief.md`
- `tasks/prism-3415ef.md`
- `tasks/prism-e37618.md`

The pass changed those three files and only the five selected task records. A hash
comparison of all 161 main-checkout task/document files, plus clean main git status,
confirmed main was untouched. `just gate` passed: zero task errors/warnings, 327 Node
tests, and the Lua plugin suite. Explicit `tasks -C <prism-worktree> check`,
`git diff --check`, and the pre-commit hook passed. The retained branch was committed
without merging, pushing, changing runtime plugin links, or installing the skill globally.

## Default exclusion and explicit rerun

A read-only `list --status idea` plus `show` inspection excluded the three briefed
members because each latest note starts with `scope:`. The two shelved members were
absent from the idea pool. No unrelated cluster was processed.

The explicit rerun `/scope prism-b8b589 prism-920f31 prism-ad2b12` re-read the members,
existing goal, design, reset/autosave work and handoff. Their questions remained
unanswered. It reused the same brief, goal, design and reset task and made zero writes:
no new evidence justified editing artifacts or adding another audit note. Task/document
hashes before and after were identical, including sources, parents and prior notes.
No skill wording correction was demonstrated by this pass or rerun.

## Integration

On 2026-09-13, the tasks feature was merged to `main` at `5e24819`, including
goal closeout `644e283`. Prism `main` was fast-forwarded to acceptance commit
`6887eae`. The paths above now resolve in Prism's main checkout; the worktree paths
in the trial narrative and exact summary below describe the original acceptance run.
Both main checkouts passed `just gate` after integration: 167 unit and 275 CLI
tests in tasks, and 327 Node tests plus the Lua suite in Prism, with zero task
errors or warnings. Both merged branches and their worktrees were then removed.

## Exact user summary

> Profile editing cluster: these five ideas share profile controls, write targets, and reset behavior.
>
> - prism-b8b589 — briefed; marker lifecycle needs design.
> - prism-920f31 — briefed; inactive rename/delete already works; panel targeting needs design.
> - prism-8a8eac — shelved; New’s starting state waits for reset/autosave decisions.
> - prism-49a068 — shelved; default presentation waits for reset/autosave decisions.
> - prism-ad2b12 — briefed; reuse prism-bf3ae9 to settle reset semantics.
>
> Handoff: Prism `.worktrees/scope-acceptance/docs/notes/2026-09-13-profile-editing-brief.md`; goal prism-3415ef; design prism-e37618. The original Mindful source was unavailable.
>
> Shelved: prism-8a8eac and prism-49a068 wake after prism-bf3ae9 settles reset semantics and prism-46035b settles wallpaper autosave; retain their 2026-11-10 review date. Explicit rerun found unchanged evidence and reused the handoff without writes.

## Tasks closeout

Final `just gate` passed: formatting, Clippy with warnings denied, zero task check
errors/warnings, 167 unit tests and 275 CLI tests. Both `tasks-39a041` and
`tasks-0d50ff` completed in this change. `prime` offered `tasks-019c60` for closeout
with zero open descendants, but its foreign live claim remained held; the umbrella
was left unchanged for its holder. The pre-commit `tasks check` passed after closure.
