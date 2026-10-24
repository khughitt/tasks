# Bounded session summaries

Status: draft for user review; no implementation authorized by this document
Task: tasks-e7424b; answers tasks-4ca7bd under tasks-96c00c
Input: `docs/notes/2026-10-19-cli-consumer-reads-brief.md`
Related gate: tasks-282666, flow-trial-1 readout on 2027-02-16

## 1. Purpose and evidence

An agent's first task read must reveal useful work, current ownership, stops and
warnings within one tool response. Today `prime` duplicates full task summaries in
ready, lanes and a recursive roadmap. The required session-start read in this
session was truncated at a 6,000-token tool budget.

The brief measured 101,001 bytes on 2026-10-19. A fresh 2026-10-24 read at
`3b92493d55e22a552b32e520959091c65ac9ce6f` measured 133,228 stdout bytes, including
the trailing newline. The code is unchanged from `76520d1`; the intervening commits
only start and note this task. Compact section sizes, excluding their outer keys:

| Section | Bytes | Top-level rows |
|---|---:|---:|
| roadmap | 81,549 | 46 roots, with descendants |
| ready | 40,435 | 84 |
| lanes | 10,096 | 7 |
| doing | 822 | 1 |

Parked, closeout and warnings were empty. This is one dataset observation, not a
benchmark. It confirms that limiting ready alone cannot solve the reported issue.

Checked code: `src/commands/list.rs::prime`, `src/output.rs::{PrimeOut,
TaskSummary,ParkedRow,TreeNode}`, `src/lanes.rs::LaneRow`, `src/cli.rs::Command::Prime`,
`src/commands/mod.rs` and the prime integration tests in `tests/cli.rs`.

## 2. Recommendation and alternatives

Add **`tasks --json prime --summary`**, with a hard 12,288-byte (12 KiB) successful stdout
limit, fixed row caps and explicit omission metadata. Reuse the existing rows and
selection logic. Ordinary `prime` remains the complete read with its existing
JSON contract. All numbers and interface choices below are the author's proposed
defaults, not prior user decisions.

The target consumer is this session's `exec_command` response with its observed
6,000-token output cap. At an estimated 3–4 UTF-8 bytes per token for these JSON
rows, 12 KiB is about 3,072–4,096 tokens. Reserving 1,000 tokens for tool framing
and surrounding output leaves at least 904 tokens of additional margin under that
estimate. This replaces the original 16 KiB proposal's narrower margin. The
conversion and reserve are sizing assumptions, not tokenizer measurements or a
guarantee for arbitrary text; §8 requires an end-to-end consumer check before adoption.

A design-only projection of the captured response, using the fixed caps and local
explicit executable/directory in recovery argv, is 13,277 bytes. Applying the
12 KiB limit drops three roadmap roots and yields 11,770 bytes, retaining five
ready rows, five lanes, one doing row and two roadmap roots. The five lanes alone
are 6,564 bytes; more active work can evict additional orientation rows. These are
projection measurements, not results from an implemented summary command.

Alternatives considered:

- **Change default prime immediately.** Rejected for this rollout: consumers can
  depend on full arrays, and session summaries change what trial participants see.
  Opt-in delivery alone does not fix the default session-start instructions; their
  migration is an explicit later rollout, not a claim this task has fixed startup.
- **Cap ready only.** Rejected: the measured roadmap is twice ready's size.
- **Introduce compact task-row schemas.** Rejected initially: existing rows usually
  fit with small caps; a second schema creates another field contract to maintain.
- **Only cap rows, or truncate serialized text.** Rejected: one large body paragraph,
  tag list, claim path or warning can exceed a row-based budget; cutting JSON bytes
  produces invalid data or silently corrupts an instruction.
- **Pagination, configurable budgets, caching or a new command.** Deferred: complete
  prime and focused existing reads supply recovery; none is needed for this summary.

This task produces a user-reviewed spec and then a separately user-reviewed
implementation plan. It changes no parser, renderer, skill instructions or default
read behavior. Implementation and rollout are follow-up work.

## 3. Command and compatibility

`--summary` is a boolean option on `prime`, used with JSON. Explicit `--json`
overrides a valid `TASKS_FORMAT=pretty` by the existing format rules. If the final
resolved format is pretty, fail before scanning with typed error
`summary_requires_json`, exit 1 and the existing JSON error envelope on stderr:
`prime --summary requires JSON; use tasks --json prime --summary`.

Existing scope options (`--project`, `--group`, `--all-projects`), `-C`, `--without`,
`TASKS_WITHOUT`, and `TASKS_MAX_COMPLEXITY` retain their current semantics. `--closed`
remains accepted and, as today, changes no JSON counts. Invalid environment and
scope values still fail through their existing typed errors.

No `--full` switch is needed: removing `--summary` is the explicit full-data route.
With the option absent, serialized fields, row order, recursive roadmap, pretty
rendering, warnings and errors remain unchanged. In particular, no `summary: null`
appears on the existing response. Older binaries reject the new option; consumers
must upgrade the binary before adopting it and must not silently retry without it.

The implementation first lands the new option's row in ops's `cli.toml`, then
adopts it here with the vendoring tool together with the parser change. This design
does not edit the vendored table or authorize a cross-project implementation now.

## 4. Response contract

The successful response retains prime's top-level fields and existing JSON types.
Only these six arrays are sampled; counts, periodic and deferred summaries,
prefix/group/projects, warnings and halts remain complete. Halts retain the current
absence rule when empty. `summary` is present only with `--summary`.

| Array | Initial cap | Selection and preserved order |
|---|---:|---|
| parked | 3 | First current prime parks, most recently parked first |
| doing | 5 | First current prime doing/claimed rows, in current list order |
| ready | 5 | First current eligible rows, in current ready order |
| closeout | 3 | First current eligible closeout goals, in current ready order |
| lanes | 5 | First current lane rows, in current lane order |
| roadmap | 5 | First current forest roots, in current forest order; no descendants |

Included task, parked and lane rows retain **all their existing fields and values**.
An included lane retains full guidance, pick, active, held, causes and steps; its
nested arrays are not sampled. If a large lane cannot fit, omit that entire lane
under the byte rule. Do not clip titles, guidance, paths, next steps, warnings,
identifiers or claim details. A roadmap root retains its entire `TaskSummary` but
has no `children`; descendant omission is explicit below. This is a shallow overview,
not a claim that a goal has no descendants: existing `child_count` and
`open_descendant_count` remain present and unchanged on each shown root.

All six `summary.sections` entries are always present, with:

- `total`: rows in that array before sampling; roadmap counts roots, not nodes.
- `shown`: rows returned in its top-level array.
- `omitted`: `total - shown`, including row-cap and byte-budget removals.
- `first_omitted`: first excluded row's id in original order; absent if none.
  For lanes this is `lane.id`. Preserve a malformed unresolved park's original id
  string, as the current parked row does; full recovery is available even if `show`
  cannot resolve it.

`summary.sections.roadmap` additionally has `descendants_omitted`: the number of
non-root nodes across the complete forest, including children of omitted roots.
Count tree occurrences, not a separate global task count. Every such node is absent
from this shallow roadmap, even if the task appears in ready or another section.

`summary.byte_limit` is always `12288`. `summary.complete` is true exactly when all
six `omitted` values and `descendants_omitted` are zero. It means prime's displayed
data is complete, not that the registry was entirely reachable or no selection
filters applied; warnings still describe those conditions. Metadata totals derive
from the same read snapshot as the returned rows.

`summary.full` is an argv array for a complete JSON prime read. It starts with
`["<running executable>", "--json", "-C", "<effective command directory>", "prime"]`, preserves
the original scope, `--closed` and explicit `--without` arguments, and omits
`--summary`. Its first member is the absolute running executable path returned by
`std::env::current_exe()`, so invoking a worktree binary yields recovery through
that binary, independent of `PATH`. The directory is the absolute effective
directory, including the worktree when local. The example placeholders are not
literal output values. If either path cannot be obtained or represented as UTF-8,
return a typed I/O error; do not substitute `tasks` or a lossy path.
Treat each array member as an argument, never concatenate it into a shell command.
Run it with the same environment and registry. Environment-driven cutoff/without
settings still apply, as they do to full prime today. This is a fresh read, not a
snapshot cursor: claims or records can change between reads.

Illustrative metadata for 84 eligible ready rows and a 46-root, 150-descendant
roadmap, before any extra byte pruning (ids and totals here are examples):

```json
{
  "byte_limit": 12288,
  "complete": false,
  "sections": {
    "parked": {"total": 0, "shown": 0, "omitted": 0},
    "doing": {"total": 1, "shown": 1, "omitted": 0},
    "ready": {"total": 84, "shown": 5, "omitted": 79, "first_omitted": "demo-000006"},
    "closeout": {"total": 0, "shown": 0, "omitted": 0},
    "lanes": {"total": 7, "shown": 5, "omitted": 2, "first_omitted": "demo-000106"},
    "roadmap": {"total": 46, "shown": 5, "omitted": 41, "first_omitted": "demo-000206", "descendants_omitted": 150}
  },
  "full": ["<running executable>", "--json", "-C", "<effective command directory>", "prime"]
}
```

This object is the `summary` value, not the whole response; existing top-level
arrays remain arrays, with `[]` when empty, and sparse row fields remain sparse.
For an empty project all six totals/shown/omitted values and descendants_omitted
are zero, complete is true, all six arrays are empty and full is still present.

## 5. Byte limit and failure behavior

The hard limit covers the complete compact JSON stdout response, UTF-8 encoded,
including object keys, metadata, escaping and the one trailing LF. It is a byte
guarantee, not a token guarantee for every harness or a runtime/memory bound. The
consumer sizing and estimated token margin are given in §2.

Build the same complete prime result first, preserving its scan, dependency,
claim/copy precedence, cutoff, needs, halt and warning logic. Record totals, remove
roadmap descendants, and take each section's initial prefix. Serialize the actual
candidate, including all omission metadata and full argv. While it exceeds the
limit, remove the last row from the first nonempty section in this order:

1. roadmap
2. lanes
3. closeout
4. ready
5. parked
6. doing

Recompute shown, omitted, first_omitted and complete after each removal, and
measure again. This retains a prefix of every section and prioritizes ownership,
resume instructions and eligible work over orientation. A huge first row may
empty its section; do not skip it to promote lower-ranked work. The first_omitted
id is a recovery handle, not an eligible replacement pick. Even metadata for that
id can be large; it stays exact and counts against the budget.

If the result still exceeds the budget with every sampled array empty, fail with
`summary_too_large`, exit 1, no stdout, and the existing error envelope on stderr.
Use this fixed bounded detail, without embedding the unbounded warning or path:
`prime summary exceeds 12288 bytes with required metadata; rerun the same JSON prime without --summary and capture stdout to a file`.
Warnings and halts are never dropped, truncated, deduplicated anew or silently
redirected to make a successful summary fit. Their existing order is retained.
This explicit failure is the cost of preserving actionable safety information.

Malformed task records and other existing fatal errors still fail before sampling;
an excluded row cannot conceal a parse error. Unreachable-project, unknown-halt,
claim and stale-copy warnings remain warnings rather than record errors, but their
combined size can now make `--summary` fail with `summary_too_large` even when full
prime succeeds. This is an intentional opt-in exit-status change, requiring the
consumer failure path in §8. Eligibility and start-safety rules remain unchanged.

## 6. How an agent uses and expands the summary

The summary is a first read, not an exhaustive scheduling view. Under a session
cutoff it remains essential to pick through ready/next, never the roadmap or parks.
`start` remains the authority for current claims and gates; summary omissions do
not authorize takeover, closing a goal or bypassing a halt.

Recovery uses existing commands in the same scope and environment:

| Need | Complete or focused read |
|---|---|
| Exact full prime sections, including warnings | `summary.full`, with stdout captured to a file |
| Every goal offered for closeout, including omitted goals | `summary.full`, then read its `closeout` section; `list` and `tree` cannot substitute for this invitation |
| Next eligible work or additional ready candidates | `tasks next`; `tasks ready -n 10` |
| A known omitted task's record and claim | `tasks show <id>` |
| All parks and their resume instructions | `tasks list --parked`, captured if large |
| Every lane's guidance, active work and pick | `tasks lanes`, captured if large |
| One goal's whole subtree | `tasks tree <id>`, captured if large |
| Registry-wide claims for dispatch planning | `tasks claims`, captured if large |

`next` may prefer an agent-waiting park and is not promised to equal ready[0].
`claims` is registry-wide and is not a scope-exact substitute for prime.doing;
full prime is the authoritative recovery for that section, including unclaimed
doing records and claims found in other checkouts. Supply the original scope
arguments on commands that accept them. Keep the original -C directory for local
reads: `--project` deliberately switches to the registered checkout.

A full read written to a file is then parsed locally and read in bounded pieces;
printing that file whole recreates the original problem. For example, a consumer
can extract one section or an array slice with a JSON parser. The CLI adds no
pagination promise, and repeated reads need not reproduce the original snapshot.
If doing or parked has omissions, obtain its full data before scheduling parallel
work or concluding no resumable task exists. A lane's pick is only an invitation
under the existing lane semantics, not a reservation of a shared resource.

## 7. Acceptance examples for the future implementation

Use current full prime as the oracle for selection, order and row values, under
stable fake clocks, registries and claims. Every successful summary must parse as
JSON and be at most 12,288 stdout bytes including LF. These are future test
requirements; this design task does not claim they already run.

| Case | Required result |
|---|---|
| Empty project | Section arrays empty; exact zero metadata; complete true; counts, periodic/deferred and full recovery present |
| Large roadmap | At least 200 eligible leaves in a multi-level forest; at most 5 roots, no children, exact excluded-root and descendant totals; ready remains the first eligible prefix |
| Ordinary large ready list | At least 80 eligible rows; no more than 5, same order/values as full prime; exact omitted count and sixth id when 5 fit |
| Lanes | At least 7 lanes covering ready, held, waiting, paused and empty; at most 5 full lane rows, unchanged picks/causes/held data; prefix truncation preserves full view's cross-lane hold decisions |
| Oversized lane | One lane with enough active rows or guidance to exceed the entire budget; omit atomically, expose first_omitted and full route; never clip guidance or recompute its pick on the retained subset |
| Claims and parks | More than caps, with a task claimed only in another checkout, a locally stale copy, an unverifiable claim and an unresolved park; sampled rows preserve full provenance and full claim/park objects; omitted totals recover the rest |
| Halts and warnings | An open halt, an unreachable scoped member and stale-claim warnings; full halt rows and warning strings remain exact even if every orientation row must be dropped; selection matches full prime |
| Required metadata overflow | Enough warnings, halts, scope data or first_omitted metadata to exceed the budget with sampled arrays empty; summary_too_large, exit 1, empty stdout, bounded stderr detail |
| Large and escaped strings | Multibyte Unicode, control characters, quotes and long titles/tags/paths; exact JSON byte measurement, whole-row eviction, no clipping or broken UTF-8 |
| Cutoff, unavailable needs, paused lanes, future deferral, due recurrence | Same ready and closeout candidate sets as full prime, then prefix caps; complete does not mean these filters were absent |
| Group/all-project and local worktree reads | Existing prefix/group/projects contract, warnings and copy precedence unchanged; full argv restores the running executable, scope and directory, even with another tasks binary first on PATH; retired/unreachable members follow current rules |
| Format and existing errors | Explicit JSON wins over valid pretty env; resolved pretty summary rejects; malformed env, record and unknown scope still produce typed errors |
| Existing callers | All preexisting full JSON and pretty tests unchanged; no summary key without the new flag; normal full output remains unbounded |

Include a boundary test at the exact byte limit and one byte beyond it, including
metadata changes caused by the final eviction. Build fixture subprocess environments
key by key with fake credentials and temporary HOME/config/state; isolate git from
real configuration and inherited GIT_* handles before fixture writes. Use the
repository's just test front door for every test run.

## 8. Implementation boundaries and rollout gate

The implementation plan, after this spec's user review, must cover:

- The ops-owned option row and adoption order, with no hand edit of tools/cli.toml.
- CLI parsing, command dispatch and JSON-only format validation, including env format.
- One summary transform after existing prime construction; reuse TaskSummary,
  ParkedRow, LaneRow, TreeNode and the existing error/rendering conventions. Add no
  second scanner, selection engine, row schema or dependency.
- Typed errors, serialization byte accounting and the acceptance fixtures above.
- README usage/recovery guidance for the opt-in binary feature; keep the separately
  gated skill and session guidance out of that implementation change.
- Separate binary and consumer rollout steps, test/check routes, the consumer's
  overflow recovery and token-budget pilot, and the later default decision.

**Binary delivery is not held by the trial gate.** After the document reviews and
normal implementation checks, the new unused binary option, its tests and README
can land. Existing calls return the same data and keep the same defaults. Do not
bundle changes to agent-loaded instructions or consumers into that delivery.

**Consumer adoption is held by the trial gate.** `skills/tasks/SKILL.md` is loaded
in other projects' sessions: mentioning `--summary` there can cause enrolled agents
to use it immediately. Keep even opt-in skill guidance, hooks, session instructions
and enrolled-consumer changes out until the 2027-02-16 flow-trial-1 readout, unless
the user, as **flow-trial-1's owner**, records a no-effect verdict for that specific
change. The owner is identified in flows's
`docs/notes/2026-10-22-records-only-direct-path-brief.md` §5; the frozen trial's
`evals/trials/flow-trial-1.md` records the readout date. This split applies the gate
recorded by tasks-282666 to changes in what enrolled sessions see or what verdict
tools measure. This spec neither supplies a no-effect verdict nor authorizes rollout.

The deferred consumer rollout must handle `summary_too_large` explicitly. Retain
the original invocation because failed summary output contains no `summary.full`.
Use that same executable, effective directory, scope and environment, remove only
`--summary`, and capture the full JSON read to a caller-chosen persistent file.
Emit a short visible notice naming the summary failure and the capture's actual
path, so session startup supplies actionable context instead of blank stdout.
Read the file in bounded sections, including all warnings/halts and any omitted
ownership or closeout data before acting. Do not print the whole file or claim the
bounded summary succeeded. If full capture fails, report its error and retain a
nonzero consumer result; other summary errors are reported rather than retried.

Before adopting the consumer, run one end-to-end pilot through its actual tool
with the 6,000-token cap, using representative JSON and the overflow/recovery case.
Check that framing plus the response fits without truncation and that recovery
reports the saved path and remains readable. Record the actual output/token
accounting when the harness supplies it; otherwise record whether truncation
occurred and the captured byte count without claiming an exact token count. A
different or smaller consumer budget needs its own check. A failed pilot stops
adoption and returns the cap or consumer framing for review; it does not weaken
the complete-warning rule. This pilot is rollout verification, not a claim that
12 KiB universally fits every tokenizer or tool wrapper.

After the gate permits adoption, recommend explicit `tasks --json prime --summary`
in session consumers. Assess making summary the CLI default in a separately
reviewed breaking-contract migration: inventory full-data consumers, define their
explicit full-data interface, prove compatibility and announce the change before
switching. This design recommends retaining the current default rather than adding
an unused --full option now or silently changing arrays for existing consumers.

No listing-performance work, cache, project-identity fields, whole-registry
read-path migration, host configuration change or live rollout belongs to this task.
On completion of both document reviews, update the input brief and wake
tasks-4ca7bd with the approved contract, implementation remaining, and trial gate,
before this branch merges. Do not mark either review approved from a builder check.
