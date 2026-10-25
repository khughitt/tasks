# Creation and occurrence provenance: who filed a task, and whose ask it is

Status: approved for planning (2026-10-25, user, conditional on the round-3 P2 fix in this revision 3; reviewed rounds 2-3 by codex/gpt-6.1-sol)
Task: tasks-a58a90 (wakes tasks-516ab4 and tasks-2e7d03)
Brief: docs/notes/2026-10-20-feedback-ask-preservation-brief.md

## 1. Intent

An agent picking up a task cannot tell who asked for it. The record names the harness
and model that ran `add` (`agent`), but not the session, and nothing says whether the
user asked for the work or an agent decided to file it. Two consequences:

- **Questions go to the wrong person.** The user's words in the session that led to
  tasks-2e7d03: "probably 90+% of the tasks are created by other agents, not me (user),
  which means that I do not have the context to understand the task and provide a
  proper response". The global instructions now tell an agent to read a task's origin
  before asking the user about intent; today that read starts with grepping session
  stores for the id.
- **Feedback loses asks.** A scope pass judging a report reconstructs its origin by
  hand (tasks-516ab4); in the coverage-drop audit both lost asks among user-relayed
  reports were the user's own words.

The user's definition, which this design adopts: a task is the user's when "I provide
the prompt that leads to the task creation", not when the user ran the command; agents
"still help capture the tasks and run the commands". The other kind is "indirect --
created based on agent feedback, downstream dependencies, etc."

The user also proposed, for feedback, capturing context at filing ("exact user input
and summary of surrounding context") or extracting it later from session logs, with
the risk that logs do not persist. This design takes the pointer route for the tool
and leaves the words in the report's body, where lore-c01bc8's rule already puts them
(§4.8).

## 2. Current behaviour

Checked at `dccbdce`:

- `add::blank` (src/commands/add.rs) builds every new record with empty notes; it is
  shared by `add` and `feedback`. `add::run` and `commands::create` record no session.
- A sourced `add` whose source and title already exist returns `reused` and writes
  nothing.
- `feedback` creation writes no note. Recurrence appends `feedback from <prefix>:
  <summary>` and optionally `detail from <prefix>: <body>` with `append_note`, unstamped,
  under the fixed author `feedback`.
- `append_stamped_note` (src/commands/mod.rs) and `provenance::resolve_from` already
  resolve native session identity: `CLAUDE_CODE_SESSION_ID`, or Codex's
  `CODEX_THREAD_ID` before `CODEX_SESSION_ID`. Unknown identity leaves the pair absent;
  a conflict leaves it absent and warns without values. Lifecycle notes (`started`,
  `parked …`, `done`, …) and `note --stamp` use it.
- A stamped note serializes as one indented continuation,
  `provenance: {"harness_session":…,"harness_session_source":…}`. `HarnessProvenance`
  denies unknown fields, and the frontmatter parser rejects unknown keys, so any new
  stored field needs every host that reads synced records on a current binary.
- No harness exposes a turn or tool-call id to a running command. The CLI's output
  (the created id) does land in the filing session's transcript as a tool result.

## 3. Goals, non-goals, anti-goals

Goals:

1. Every new record, from `add` or `feedback`, says which session filed it, and says
   so explicitly when that is unknown or conflicting.
2. Every feedback occurrence (creation and each recurrence) is stamped separately.
3. A record can say whose ask it is (`user`, `agent`, `mixed`), supplied by the filer
   from its own evidence, correctable later, never inferred by the tool.
4. A filer can link an older session where the ask was made, when that is not the
   filing session; a Codex worker's filing names its root thread.
5. A later agent can follow a record to its originating ask, or to an explicit
   unavailable result, without relying on commit time.

Non-goals:

- Building a session reader. hq-c411df's reader (built as obs-d98663) and hq-c8aef1's
  task-to-session command own that; this design gives them the stamp to read.
- Cross-tool tracing (obs-c5c599), drop verification and carry (tasks-441019),
  candidate search at filing (tasks-a9744c).
- Backfilling existing records. Records filed before this lands stay as they are.
- Copying transcript content into records.

Anti-goals (a failure even if it works):

- The tool deciding whose ask a task is from `agent`, `parent`, `source`, tags,
  session presence or commit time.
- A reused `add` stamping the later caller as the creator.
- Provenance becoming claim authority, or `TASKS_SESSION` becoming a provenance source.
- A feedback record in a public owner repository carrying the reporter's project
  content because of this change.

## 4. Design

### 4.1 The creation note

Every `add` that creates a record, and every `feedback` that creates one, writes one
generated note in the same write: text `created`, `at` equal to the record's `created`
timestamp, stamped as lifecycle notes are. Its author is fixed: `add` for `add`, and
`feedback` for `feedback` (the reporter's owner name must not reach the owner's file).

Call (mine): a fixed author rather than the owner name `start` uses. `add` does not
resolve an owner today, and `owner_name` can fail (an invalid `TASKS_OWNER` or branch,
no branch and no `USER`), so using it would add a creation failure. With
`add --project X` it would also name X's branch, not the filer's. The session is the
provenance; the author only labels the event.

- Known identity: the note carries the native pair.
- Unknown identity: the note lands without the pair.
- Conflicting or invalid identity: the note lands without the pair and the command
  warns, naming variables, not values. Creation is never blocked.
- `reused`: nothing is written, as today.

A reader identifies the creation note as the first note whose text is `created` and
whose `at` equals `created`. "No creation note" means the record predates this change
or was written by another tool; "creation note without a session" means the filer's
identity was unknown. Both are unknown, but distinguishably so.

Call (mine): the note is always written, even with nothing known, so an unknown filer
is recorded rather than indistinguishable from an old record. The alternative, writing
it only when it carries something, saves a line per record and loses that distinction.

### 4.2 Feedback occurrences

- Creation: the creation note of §4.1.
- Recurrence: the existing `feedback from <prefix>: <summary>` note and its optional
  `detail` note are stamped. Their texts are unchanged, so existing consumers keep
  parsing them.

An occurrence's **occurrence note** is its first note: `created` for a creation, the
`feedback from` note for a recurrence. Only occurrence notes carry the occurrence keys
of §4.3 and §4.4 (`origin`, `origin_session`, `root_session`). A `detail` note carries
the native pair alone.

Each occurrence therefore carries its own session, and the reporting session of a
recurrence is never confused with the session that created the record.

### 4.3 Origin: whose ask the record is

One optional value, `origin`:

| Value   | Meaning |
|---------|---------|
| `user`  | Every ask in the record comes from the user's own prompt: they described it, asked for it to be filed, or the report relays their words. |
| `agent` | No ask in the record comes from a user prompt: an agent's observation, follow-up, feedback report, decomposition piece, plan step, dependency or scope filing. |
| `mixed` | The record holds both: at least one ask from the user's prompt and at least one distinct ask the agent added. |
| absent  | Unknown. |

Origin classifies the record's asks, not the filing. Who ran the command, whether the
agent chose to file on its own (for example under the feedback rule), and how the agent
phrased the asks never change the value. This follows the user's definition in §1,
which separates whose prompt led to the task from who captured it. A report that
relays only the user's words is `user`, even when the agent filed it unprompted. It is
`mixed` only when the agent also added an ask of its own, such as a proposed remedy
the user did not ask for. The report body marks which ask is whose, as lore-c01bc8
requires.

A decomposition child of a user-requested goal is `agent` unless the user asked for
that piece; its parent link already leads to the user's ask.

**Who supplies it.** The filer, from evidence in its own session, with `--origin` on
`add` and `feedback`. When unsure it omits the flag. The tool never derives it.

**Where it lives.** Two places, with different jobs:

- **Evidence, per occurrence.** The creation note and each feedback recurrence note
  record the origin supplied at that occurrence, if any. These never change.
- **The record's judgment.** A frontmatter field `origin` holds the current judgment
  for the record as a whole. It is set at creation from `--origin`, combined on
  recurrence, and corrected with `edit`.

**Recurrence.** A recurrence with origin X updates the record's field:

- the field equals X, or X is absent: unchanged;
- the field is `mixed`: unchanged;
- the field and X are different known values: `mixed`;
- the field is absent and X is known: unchanged, with a warning that the record's
  origin is unknown and this occurrence says X. The occurrence note keeps X, so a
  reader asking "does any occurrence carry the user's words" still finds it.

**Correction.** `tasks edit <id> --origin <value>` changes the field only; occurrence
notes stay as history. The skill asks a corrector to add a stamped note citing its
evidence (`tasks note <id> "<evidence>" --stamp`). Anyone with evidence may correct:
typically a scope pass that has read the origin session. To overrule a mistaken
occurrence label, set the right value. Evidence that the user did not ask means
`agent`, and evidence of a user ask means `user` or `mixed`, so a corrector with
evidence can always name one. `--no-origin` withdraws the judgment: the field becomes
absent and the occurrence labels speak again under the rule below.

**The reading rule.** Every consumer reads origin through one rule, which the README
states once:

1. **When the field is set, it is the record's origin.** It supersedes every
   occurrence label, including the creation label. Labels written before or after a
   correction are history; a later recurrence still updates the field by the combine
   rule above.
2. **When the field is absent, the origin is unknown, but the occurrence labels are
   partial evidence.** Any label `user` or `mixed` means the record carries some of
   the user's words. Otherwise the labels say nothing about the unlabelled
   occurrences, so the origin stays unknown.

**Consumers.**

- An agent deciding whom to ask about intent applies the rule. `user` means the user
  holds the intent. `agent`, `mixed` or unknown means reading the origin first (§4.7).
- tasks-441019 treats a record as carrying the user's words when rule 1 gives `user`
  or `mixed`, or rule 2 finds a `user` or `mixed` label. So a corrected `agent`
  overrides a mistaken creation label of `user`. Until this lands, tasks-441019 keeps
  judging from report text.

Call (mine): three values, not a vocabulary of agent kinds (follow-up, feedback,
decomposition, scope). The user's distinction is binary with a mixed middle, and the
kinds are mostly derivable already: the `feedback` tag, `parent`, `source`, the plan
step. An independent kind vocabulary for `add` would also go against the brief.

### 4.4 Older origin sessions and Codex workers

- `--origin-session <key>` on `add` and `feedback` names the session where the ask was
  made when that is not the filing session: for example, a later session filing
  something the user asked for days earlier. The key uses the stored grammar
  (`claude-code:<id>` or `codex:<id>`, nonempty, control-free). It is recorded on that
  occurrence's note as `origin_session`. It says where to look; it is not a claim that
  the user asked, which stays `origin`'s job.
- A Codex worker whose `CODEX_THREAD_ID` differs from a valid `CODEX_SESSION_ID`
  records the root on its occurrence notes as `root_session: codex:<root>`. The audit
  could recover 6 Codex subagent origins only through their parent thread, because the
  dispatch brief is encrypted. Lifecycle and other stamped notes stay as they are; the
  README's "notes gain no root field" is narrowed to "only occurrence notes do".

Call (mine): root on occurrence notes only, keeping the earlier root decision intact
for lifecycle notes. Claude Code subagents run under the parent's
`CLAUDE_CODE_SESSION_ID` (the builder confirms this), so their stamp already names the
parent, whose store holds the subagent transcripts.

### 4.5 Storage: creation note versus creation metadata

The brief asked for this comparison.

- **Session in frontmatter** (`created_session` beside `agent`): compact in `show` and
  queryable. But it is a second mechanism for the same fact lifecycle notes already
  store, it leaves recurrences needing notes anyway, and it is a new key.
- **Session on a creation note** (chosen): the same shape as every other stamped
  event, uniform across creation, recurrence and lifecycle, and readable by obs's
  existing projection of stamped notes once `created` joins its text list.
- **Origin in note text** (`created (origin: user)`): no schema change, but parsing
  text is fragile, and an append-only note cannot be corrected without note-integrity
  work that tasks-1f6d08 has not designed yet.
- **Origin in frontmatter plus per-occurrence evidence** (chosen): correctable now,
  filterable, and it keeps what each occurrence said.

The chosen storage adds optional keys to the note continuation JSON (`root_session`,
`origin`, `origin_session`) and one frontmatter key (`origin`). The native pair stays
all-or-nothing; the new keys may appear without it (an origin supplied under unknown
identity). The continuation line keeps the name `provenance:`.

Validation on read (the parser, so it also governs loading), checking each value's form
only:

- `origin` is `user`, `agent` or `mixed`.
- `origin_session` and `root_session` use the stored key grammar. `root_session` must
  be a `codex:` key and appears only with a native pair whose source is
  `CODEX_THREAD_ID`.
- The read side does not check which note a key sits on. The writer places the keys
  only on occurrence notes (§4.2), and a reader that wants occurrences selects them by
  that rule. A read-time placement check would make a hand-repaired file fail to load
  over something that loses no data.
- An `origin_session` equal to the note's own session is kept as given: redundant but
  harmless, and it cannot be compared when identity is unknown.

### 4.6 Surfaces

- **JSON.** Task objects gain `origin` (omitted when unset). Notes gain `root_session`,
  `origin` and `origin_session` beside the existing pair, each omitted when absent.
  These are additive.
- **Write output.** The JSON output of `add` (action `created`) and of `feedback`
  (actions `created` and `recurred`) gains `at`: the timestamp of the occurrence note
  that call wrote. A `reused` add has no `at`. The tool result in the filing
  transcript then names the occurrence it produced, not only the record (§4.7).
  Additive. Pretty output is unchanged: `add --pretty` keeps printing the bare id,
  because scripts capture it, and `feedback --pretty` keeps `<action> <id>`. §4.7
  handles the action that `add --pretty` omits.
- **Pretty `show`.** An `origin:` overview line after `agent:` when set. Note rows show
  origin and origin session with the existing provenance column.
- **Filters.** `list`, `ready` and `search` accept `--origin <user|agent|mixed|none>`,
  repeatable and widening, as `--process` does.
- **Writes.** `add --origin`, `add --origin-session`, `feedback --origin`,
  `feedback --origin-session`, `edit --origin`, `edit --no-origin`.
- **`check`.** Rejects an invalid `origin` value or an invalid key in a note's
  continuation, as it does for other fields. It does not warn about absent origin.

The new flags start as rows in ops's `cli.toml`, adopted here with the parser change
(AGENTS.md rule).

### 4.7 Finding the originating ask

The procedure, written into the scope skill's evidence step and pointed to from the
tasks skill's pickup step:

It has two stages. The filing event always comes first; an older request, when the
record names one, is found from the filing event's evidence, never searched for
directly by the record's id or time.

**Stage 1: the filing event, in the filing session.**

1. Read the reading rule's origin (§4.3) and the occurrence notes from `tasks show`.
   Pick the occurrence wanted, usually the creation; for feedback, each recurrence in
   turn.
2. Take that occurrence's filing session from its stamp; for a Codex worker, also
   `root_session`. With no stamp, the outcome is unavailable.
3. Locate that transcript with the session-logs tooling: live stores, archived stores
   (`archived_sessions`) and other harness homes on this host. Use hq-c411df's reader
   once obs-d98663 ships.
4. Bind the occurrence to its call. Candidates are the `tasks add` or `tasks feedback`
   calls in that session whose arguments fit the occurrence (title or summary, flags)
   and whose time fits the note's `at`. A `show` or any other mention of the id is not
   a candidate. What a candidate's result shows decides how strongly it binds:
   - **Full result.** JSON output names the id and an action. `feedback --pretty`
     prints `<action> <id>`, so it is full except for `at`.
     - A `reused` action excludes the candidate.
     - A matching action (`created` for a creation, `recurred` for a recurrence) keeps
       it.
   - **Id-only result.** `add --pretty` prints only the id. On reuse it adds a
     stderr warning, "<id> already carries this source and title; reused it, wrote
     nothing". The action is settled from the rest of the call:
     - The reuse warning is in the captured result: excluded as a reuse.
     - The call has no `--source`: a creation, since only a sourced add can reuse.
     - A sourced call without that warning, where the transcript captured stderr: a
       creation.
     - Otherwise it is unsettled: still a candidate, but either a creation or a reuse.
   - **Unreadable result:** a candidate on arguments and time alone.

   Outcomes:
   - **matched:** exactly one candidate remains, and its result is full and either
     carries `at` equal to the note's `at` or is the only full-result candidate whose
     time and arguments fit this occurrence.
   - **inferred:** exactly one candidate remains, and it binds without a full result:
     an id-only result whose action the rest of the call settled, or an unreadable
     result whose time and command text match. This also covers a created title that
     differs from the command text, as in 9 of 72 audited origins.
   - **ambiguous:** more than one candidate remains. This includes a creation that
     cannot be told from a reuse (an id-only sourced add without captured stderr,
     beside another call with the same source and title). Other examples: two
     recurrences in one session with equal summaries in the same second, or
     concurrent subagents under one Claude session. List the candidates; never pick
     one.
   - **unavailable:** no candidate remains although the transcript was read. Record
     what was searched.
5. Read the request, the action, the tool result and any later correction around the
   bound call, not only the nearest human turn. A human turn can contain pasted agent
   text (the 2026-10-20 Codex review was pasted into a human turn), so quote the
   user's own words apart from pasted text, and say when they cannot be told apart.

**Stage 2: an older request, when the occurrence names `origin_session`.**

6. From the bound filing call in the filing session, collect what points into the
   older session: transcripts read, the user's words quoted, the record's body and
   references. Then open the older session and find the request those point to.
   - **matched:** the filing session's evidence names a specific location in it, such
     as a session-logs read that printed the turn.
   - **inferred:** the request is found by content.
   - **ambiguous:** several turns fit.

   The record's id and the filing time are not anchors here; the request predates
   both.
7. When the older session cannot be read, the older request is unavailable. The
   filing event's outcome stands on its own.

**Outcomes.** Record one per stage:

- matched, ambiguous (with its candidates) or inferred;
- unavailable: no stamp; the session is not in this host's stores, which can mean
  another host or a pruned store; or the transcript was read and no candidate
  remains;
- lookup failed: a tool error, kept separate from unavailable, as the scope skill
  already requires.

Other-host retrieval is an explicit limitation. Records do not name the filing host:
hostnames in public owner repositories are a disclosure, and lifecycle notes do not
record them either.

### 4.8 Guidance

In this repository, updated by the change that alters the behaviour:

- **README.** Lifecycle notes gain `created` in the generated-text list. The creation
  provenance section covers origin, origin session and root.
- **skills/tasks.** At filing, pass `--origin` when you know whose ask it is, and
  `--origin-session` for an older ask. At pickup, read origin before asking the user
  about intent, and follow §4.7 for non-user origins.
- **skills/scope.** The evidence step follows §4.7 and records its outcome. Scope and
  brief notes are written with `--stamp`, so a later agent can find the session behind
  a requirement edit: today the 2026-10-25 scope note on tasks-2e7d03 is unstamped,
  and its session was found only by grepping commit messages. A scope pass that finds
  origin evidence corrects `origin` with a stamped evidence note.

Outside it:

- **ops: an in-plan step that lands first.** Land the `cli.toml` rows for §4.6's flags
  in the ops checkout. Then adopt them here with ops's `vendored adopt`, in the same
  commit as the parser change. `parser_surface_equals_table` makes this ordering
  binding.
- **lore: a filed task, not an in-plan edit.** The global instructions are a live
  global surface whose changes the user approves. The plan files a lore task, after
  this lands, to:
  - have the Feedback rule pass `--origin user` or `--origin mixed` when a report
    relays the user's words, alongside lore-c01bc8's body rule;
  - let the Decisions clause on intent cite `origin`.

  The report body keeps the user's words, as lore-c01bc8 requires; the tool keeps the
  pointer and the marker.

Consumers, informed by this design and not built here: obs-c5c599 (add `created` and
the occurrence keys to its projection), hq-c8aef1 (task id in, creating session out,
from the stamp), obs-d98663.

### 4.9 Rollout

The new keys make older binaries reject records that carry them. This is the same
constraint lifecycle provenance and `move` had: install the new binary on every host
that reads synced records before records filed by it sync. Verify one host end to end;
the other hosts updated by the same install path are covered.

## 5. Acceptance

CLI tests (tests/cli.rs, against the built binary, each subprocess given an
environment built key by key):

- `add` with a known Claude identity, a known Codex identity, a Codex worker (root
  recorded), no identity, and conflicting identity. Each writes exactly one creation
  note with the expected keys, and conflict warns without values.
- A sourced duplicate `add` returns `reused`, writes nothing, and adds no note under a
  different session.
- `feedback` creation writes a stamped creation note under author `feedback`.
  Recurrence stamps both of its notes and leaves their texts unchanged.
- Origin: set at creation, absent when omitted, and each row of the recurrence table
  in §4.3, including the absent-field warning. `edit --origin` and `--no-origin`
  change the field and leave notes untouched.
- Correction supersedes a mistaken label. Create with `--origin user`, then
  `edit --origin agent`. JSON gives `origin: agent` while the creation note keeps
  `origin: user`, and the reading rule (§4.3) yields `agent` and "no user words". Then
  `edit --no-origin`: the field is absent and rule 2 sees the `user` label again. The
  test asserts the stored JSON the rule reads at each step. The README states the
  rule with this case as its example, and tasks-441019's design cites it.
- Write output: `add` and `feedback` JSON carry `at` equal to the occurrence note's
  `at`, and a `reused` add carries none. Two recurrences of one record under one
  session identity give two outputs, each `at` naming its own note.
- `--origin-session` is validated and recorded. A bad key is refused before anything
  is written.
- JSON and pretty `show` expose the new fields. `list --origin` filters, `none`
  included. `check` rejects invalid values.
- A record written by the new binary round-trips byte for byte through an unrelated
  edit.

Retrieval demonstrations run before merge, in a scratch project with an isolated
registry (`XDG_CONFIG_HOME` and `XDG_STATE_HOME`), with the worktree's binary invoked
by explicit path. They never install it over the tracker on `PATH`. Each outcome is
recorded on the implementing task:

- A live record filed from a real harness session with the worktree binary: stage 1
  matched from its stamp.
- Two recurrences of one feedback record in one real session: each occurrence
  matched to its own call. A second run with `--pretty` and equal summaries in quick
  succession reports matched only where time and arguments separate the calls, and
  ambiguous with both candidates otherwise.
- Pretty creation, then pretty reuse: in one real session, `add --pretty --source S`
  creates a record, and the same command again reuses it. The creation binds as
  inferred: an id-only result, with its creation settled by the absence of the reuse
  warning in the captured stderr. The second call is excluded by its reuse warning. A
  variant where the transcript did not capture stderr reports ambiguous between the
  two calls. An unsourced `add --pretty` binds as inferred, as a creation.
- An older request: a record filed in session B with `--origin-session A`, after B
  read the request in A. Stage 1 matches the filing in B, and stage 2 finds the
  request in A from B's evidence. A second record naming an unreadable session
  reports stage 2 unavailable while stage 1 stands.
- An archived Codex session (`archived_sessions`): found, or an explicit limitation
  naming the reader gap.
- A Codex worker: the CLI tests cover the `root_session` stamp with a simulated
  environment. The parent-thread reading path is shown on one of the audit's existing
  Codex subagent filings. A live Codex worker run is not required.
- An unstamped (older) record: unavailable, named as such.
- Another host: an explicit limitation.

tasks-a58a90 itself closes when the design and the plan are reviewed. These
demonstrations are acceptance for the implementing steps.

Blind at-filing pilot (post-rollout, a child task): take at least three reports filed
after rollout, covering user, agent and mixed origins where available. Build a
disposable project at each record's creating commit. A fresh agent restricted to the
reporting session must recover each distinct ask and name the task that covers it. A
miss is a finding against §4.7 or the guidance, not a pass.

## 6. Calls made in this design

These are the author's calls, not user decisions; review them:

1. The creation note is always written (§4.1).
2. Three origin values, no agent-kind vocabulary (§4.3).
3. Origin is a correctable record field plus per-occurrence history. Recurrence
   combines into it, and an absent field is not overwritten. A set field supersedes
   every occurrence label; an absent field leaves the labels as partial evidence
   (§4.3).
4. `root_session` on occurrence notes only (§4.4).
5. Session stamp on a note, not in frontmatter (§4.5).
6. Pointer, not captured words. The user's words stay in the body per lore-c01bc8
   (§1, §4.8).
7. No filing host recorded; other-host retrieval stays a limitation (§4.7).
8. Scope notes get `--stamp` (§4.8).
9. The creation note has a fixed author (`add` or `feedback`), not the owner name
   (§4.1).
10. Occurrence keys go on occurrence notes only. The read side validates form, not
    placement (§4.2, §4.5).
11. The ops rows are an in-plan step that lands first. The lore guidance is a filed
    task for the user to approve (§4.8).
12. Retrieval demonstrations run before merge, by explicit path. The blind pilot runs
    after rollout (§5).
13. `add` and `feedback` output gains `at`, so a tool result names its occurrence,
    not only its record (§4.6, §4.7).
14. Retrieval has two stages, and an older request is reached only through the filing
    event's evidence (§4.7).
15. `add --pretty` output stays the bare id. Retrieval settles its action from the
    call's `--source`, its arguments and the reuse warning, rather than changing
    output that scripts capture (§4.6, §4.7).

## 7. Left to the builder

- The internal type for the extended continuation (for example a `NoteProvenance`
  with the pair optional as a unit), keeping the README's serialization example valid.
- Confirming that Claude Code subagents inherit the parent's `CLAUDE_CODE_SESSION_ID`.
  If they do not, record the finding and extend §4.4.
- The order of the new keys in the continuation JSON (stable, documented).
- Pretty layout details.
