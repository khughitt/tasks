# Retrieval demonstration: following demo records back to their filing calls

Retriever: a Claude Code subagent (claude-opus-5-5) that did not file these records. It
worked from the records, the tracker's output and session transcripts only, following
`skills/scope/SKILL.md` §2 "Find the originating ask" (worktree tasks-053e89) and the
README's "Creation notes" and "Origin: whose ask a task is" sections.

## Conventions

- `<bin>`: the worktree binary `.worktrees/tasks-053e89/target/debug/tasks`, called by its literal path.
- `<env>`: `XDG_CONFIG_HOME=<demo>/xdg-config XDG_STATE_HOME=<demo>/xdg-state GIT_CONFIG_GLOBAL=<demo>/gitconfig GIT_CONFIG_NOSYSTEM=1`
- `<demo>`: `$WORK_ROOT/tasks/tasks-4d39db/demo`.
- `F`: the filing session's main transcript `~/.claude/projects/-mnt-ssd-Dropbox-tasks/0f11453f-b2c1-4b7e-949d-cb12a26ba1da.jsonl`.
- `S`: the filing subagent's transcript `~/.claude/projects/-mnt-ssd-Dropbox-tasks/0f11453f-b2c1-4b7e-949d-cb12a26ba1da/subagents/agent-acbddcd9487c494e9.jsonl`.
- `Lnn` is a line number in the named JSONL file. Times are UTC.
- The extraction scripts are in `$WORK_ROOT/tasks/tasks-4d39db/retrieval-scripts/`
  (`calls.py`, `codex_calls.py`, `subagent_filings.py`). Each one only reads JSONL files
  and prints matching `tool_use` / `function_call` records with their results.

## Record reads (all read-only)

    <env> <bin> -C <demo>/owner list --pretty      # also <demo>/reporter, <demo>/fresh
    <env> <bin> -C <demo>/owner show <id>          # own-* ids; frs-9a1c41 from <demo>/fresh

Each record has the same creation and recurrence stamp:
`harness_session: claude-code:0f11453f-b2c1-4b7e-949d-cb12a26ba1da` (`CLAUDE_CODE_SESSION_ID`).
None has `root_session`.

| id | created | occurrence notes (`at`, text) | other fields |
|---|---|---|---|
| own-0677b0 | 13:27:37 | 13:27:37 created | |
| own-249b63 | 13:27:30 | 13:27:30 created; 13:27:42 feedback from rep (+ detail "first recurrence detail"); 13:27:49 feedback from rep (+ detail "second recurrence detail") | tags feedback, friction |
| own-00f6e1 | 13:27:56 | 13:27:56 created; 13:28:03 feedback from rep; 13:28:03 feedback from rep (no detail notes) | tags feedback, gap |
| own-c77db0 | 13:28:09 | 13:28:09 created; 13:28:14 feedback from rep (+ "first recurrence detail"); 13:28:20 feedback from rep (+ "second recurrence detail") | tags feedback, idea |
| own-c44636 | 13:28:21 | 13:28:21 created; 13:28:30 feedback from rep; 13:28:30 feedback from rep (no detail) | tags feedback, gap |
| own-0465cd | 13:28:40 | 13:28:40 created | source `demo-source-S` |
| frs-9a1c41 | 13:28:50 | 13:28:50 created | source `demo-source-S2` |
| own-2a7ce9 | 13:28:51 | 13:28:51 created | no source |
| own-fac412 | 13:29:08 | 13:29:08 created, `origin: user`, `origin_session: claude-code:572ee51e-2f53-4729-96f8-f122a2cc8305` | field `origin: user`; body quotes user words |
| own-df3b0c | 13:29:11 | 13:29:11 created, `origin_session: codex:00000000-0000-4000-8000-00000000dead` | no origin field |

## Locating the filing transcript (shared by every demo occurrence)

    ls ~/.claude/projects/*/0f11453f-b2c1-4b7e-949d-cb12a26ba1da.jsonl     # -> F
    python3 -I calls.py F                                                   # tasks add/feedback calls in F

`F` has no `tasks add` or `tasks feedback` call in the filing window. The main line's
records between 13:26:52 and 13:29:25 show an `Agent` dispatch. The demo stamps name
session 0f11453f because a Claude Code subagent inherits its parent's
`CLAUDE_CODE_SESSION_ID`, so the filing calls are in that session's subagent files:

    ls ~/.claude/projects/-mnt-ssd-Dropbox-tasks/0f11453f-…/subagents/
    # per file: first and last timestamp, all records isSidechain
    agent-acbddcd9487c494e9.jsonl  13:26:52 – 13:29:25   # S: the only subagent active 13:27–13:29
    agent-a982db7e8b3c24460.jsonl  13:30:06 –            # this retriever; it makes no add/feedback calls

    python3 -I calls.py S          # plus a listing of every Bash call in S

So no concurrent subagents filed under this session in the window.

Reading notes that apply to every case below:

- **Copies of a call:** `S` holds `api-request-blob` records (L58, L59, L110, L111, L142,
  L143). They have no timestamp and repeat `tool_use`/`tool_result` blocks whose ids
  already appear at timed lines. They are copies of one call, not further candidates, so I
  deduplicated by `tool_use` id.
- **stderr in Claude Code transcripts:** the Bash capture merges stderr into
  `toolUseResult.stdout` and leaves `stderr: ''`. I showed this with a probe in my own
  transcript (`echo out; echo err >&2` gave `stdout: 'out\nerr'`, `stderr: ''`). It also
  shows in `S` L131, where the pretty warning block, which `src/output.rs`
  `pretty_warnings` writes to stderr, appears in `stdout`. So the transcript captured stderr.

## own-0677b0: creation

- Stamp: note `created` at 13:27:37, `claude-code:0f11453f…`. Transcript: `S`.
- Candidates:
  - `S` L53, 13:27:29.349: `T=<bin>; cd $D/owner && <env> $T add "Demo: retrieval of a live creation"`.
    Result L54 (is_error): `PreToolUse:Bash hook error: relay-guard: refusing a write: … a command word held in a variable …`.
    The call never ran, its result came back at 13:27:29.572 (before the note's `at`), and
    it names no id. It is **excluded on time**.
  - `S` L63, 13:27:37.184: `cd <demo>/owner && <env> <bin> add "Demo: retrieval of a live creation"`.
    Result L64: `{"id":"own-0677b0","action":"created","at":"2026-10-25T13:27:37Z","warnings":[]}`.
    This is a full result, the action matches, and `at` equals the note's `at`.
- Outcome, stage 1: **matched**. Rule: exactly one candidate remains, and its full result
  carries `at` equal to the note's `at`.
- Stage 2: not applicable (no `origin_session`).

## own-249b63: creation and two recurrences

The same summary appears in three calls: L55, L69 and L75.

- **Creation** (note 13:27:30). Candidate `S` L55, 13:27:30.412:
  `<env> $T feedback --project own "The demo exporter drops trailing newlines in B1" --category friction`.
  Result L56: `{"id":"own-249b63","action":"created",…,"at":"2026-10-25T13:27:30Z",…}`.
  L69 and L75 return `recurred`, which does not match a creation, and their times do not fit.
  Outcome: **matched**. Rule: full result, `at` equal.
- **Recurrence 1** (note 13:27:42, detail "first recurrence detail"). Candidate `S` L69,
  13:27:42.009: `… feedback --project own "…B1" --category friction -b "first recurrence detail"`.
  Result L70: `{"id":"own-249b63","action":"recurred",…,"at":"2026-10-25T13:27:42Z",…}`.
  L75 has `at` 13:27:49 and a different `-b`; L55 returns `created`.
  Outcome: **matched**. Rule: full result, `at` equal.
- **Recurrence 2** (note 13:27:49, detail "second recurrence detail"). Candidate `S` L75,
  13:27:47.946, with `-b "second recurrence detail"`. Result L77: `"action":"recurred",…,"at":"2026-10-25T13:27:49Z"`.
  Outcome: **matched**. Rule: full result, `at` equal.

## own-00f6e1: creation and two recurrences

- **Creation** (13:27:56). Candidate `S` L82, 13:27:55.805:
  `… feedback --project own "The demo linter hides warnings in B2" --category gap`.
  Result L83: `{"id":"own-00f6e1","action":"created",…,"at":"2026-10-25T13:27:56Z",…}`.
  Outcome: **matched**. Rule: full result, `at` equal.
- **Recurrence 1 and recurrence 2** (both notes at 13:28:03, equal summaries, no detail).
  `S` L87, 13:28:02.855, is one tool call that runs the same command twice:
  `… feedback --project own "…B2" --category gap; <env> <bin> feedback --project own "…B2" --category gap`.
  Result L88 has two lines, both `{"id":"own-00f6e1","action":"recurred",…,"at":"2026-10-25T13:28:03Z",…}`.
  - Candidates for each recurrence: L87 invocation 1 and L87 invocation 2. Both are full
    results with `at` equal to the note's.
  - Outcome for each recurrence: **ambiguous**, with candidates {L87 #1, L87 #2}. Rule:
    "two recurrences in one session with equal summaries in the same second (in JSON too:
    `at` has whole-second precision)". I did not use output order to pick one.

## own-c77db0: creation and two recurrences (pretty output)

- **Creation** (13:28:09). Candidate `S` L93, 13:28:09.271:
  `… feedback --pretty --project own "The demo formatter reorders keys in B3" --category idea`.
  Result L94: `created own-c77db0` followed by the warnings block.
  - This result is full except for `at`. L98 and L104 print `recurred`, so the action does not match.
  - Outcome: **matched**. Rule: the only full-result candidate whose time and arguments fit.
- **Recurrence 1** (13:28:14, detail "first recurrence detail"). Candidate `S` L98,
  13:28:13.964, `-b "first recurrence detail"`. Result L99 (13:28:14.540): `recurred own-c77db0`.
  - L104 is the other `recurred` call with this summary. It ran 13:28:18.853 → 13:28:20.509
    and its `-b` is "second recurrence detail", so neither its time nor its arguments fit.
  - Outcome: **matched**. Rule: the only full-result candidate whose time and arguments fit.
- **Recurrence 2** (13:28:20, detail "second recurrence detail"). Candidate `S` L104,
  13:28:18.853. Result L107 (13:28:20.509): `recurred own-c77db0`. L98's time and `-b` do not fit.
  Outcome: **matched**, by the same rule.

## own-c44636: creation and two recurrences (pretty output)

- **Creation** (13:28:21). Candidate `S` L106, 13:28:19.957:
  `… feedback --pretty --project own "The demo parser swallows errors in B4" --category gap`.
  Result L108 (13:28:21.054): `created own-c44636`. It ran in parallel with L104 (a different summary).
  Outcome: **matched**. Rule: the only full-result candidate (no `at`) whose time and arguments fit.
- **Recurrence 1 and recurrence 2** (both 13:28:30, equal summaries, no detail). `S` L114,
  13:28:30.448, is one tool call that runs the same `feedback --pretty … "…B4" --category gap` twice.
  Result L115: `recurred own-c44636` twice. There is no `at` in pretty output.
  - Candidates for each recurrence: {L114 #1, L114 #2}.
  - Outcome for each: **ambiguous**. Rule: two recurrences in one session with equal
    summaries in the same second.

## own-0465cd: creation (sourced, `add --pretty`)

- Stamp: `created` at 13:28:40. Transcript: `S`.
- Candidates (same arguments: `add --pretty --source demo-source-S "Demo: sourced pretty creation"`):
  - `S` L125, 13:28:40.362. Result L126 (13:28:40.903): `own-0465cd`, with no warning in
    the captured output. This is an id-only result. The call is sourced, and the transcript
    captured stderr (see the reading notes) with no reuse warning, so the call is
    **settled as a creation**.
  - `S` L130, 13:28:44.986. Result L131: `own-0465cd` followed by the warnings block
    `own-0465cd already carries this source and title; reused it, wrote nothing`. **Excluded as a reuse.**
- Outcome, stage 1: **inferred**. Rule: exactly one candidate remains, and it binds without
  a full result, through an id-only result whose action the rest of the call settled.

**Variant: stdout-only capture.** Here I removed every stderr text from the excerpt. The
warning block in L131 is the only stderr text in these two results.
- L126 reads `own-0465cd`, and L131 also reads `own-0465cd`.
- Both are sourced id-only adds with no captured stderr, so each is **unsettled**: either a creation or a reuse.
  - L130 started 13:28:44.986, about 5 s after the note's `at`. On time alone it fits
    poorly. The skill's text still lists both: "beside another call with the same source
    and title, both are listed".
- Outcome: **ambiguous**, with candidates {L125 unsettled, L130 unsettled}. Rule: "An
  id-only sourced add without captured stderr is ambiguous between creation and reuse …;
  beside another call with the same source and title, both are listed."

## frs-9a1c41: creation, read only from a stdout-only excerpt

- Stamp: `created` at 13:28:50. Transcript: `S`.
- Candidate: `S` L135, 13:28:49.728:
  `cd <demo>/fresh && <env> <bin> add --pretty --source demo-source-S2 "Demo: lone sourced pretty creation"`.
  Result L136 (13:28:50.260), with stderr removed: `frs-9a1c41`. No other call in `S` or `F`
  has this source or title.
- Outcome: **ambiguous**, with one candidate whose action is unsettled (creation or reuse).
  Rule: "the only one left has an unsettled action. An id-only sourced add without captured
  stderr is ambiguous between creation and reuse even when it is the session's only candidate."
- I did not apply the procedure to the full capture, as instructed. The full capture shows
  no warning text, so it would have settled as a creation (inferred).

## own-2a7ce9: creation (unsourced, `add --pretty`)

- Stamp: `created` at 13:28:51. Transcript: `S`.
- Candidate: `S` L137, 13:28:50.955: `cd <demo>/owner && <env> <bin> add --pretty "Demo: unsourced pretty creation"`.
  Result L139 (13:28:51.517): `own-2a7ce9`. This is id-only. The call has no `--source`, so it
  is **settled as a creation**, because only a sourced add can reuse.
- Outcome: **inferred**. Rule: one candidate; an id-only result whose action the rest of the call settled.

## own-fac412: stage 1 (filing) and stage 2 (older request)

**Stage 1.**
- Stamp: `created` at 13:29:08. Transcript: `S`.
- Candidate: `S` L158, 13:29:07.720:
  `cd <demo>/owner && <env> <bin> add --origin user --origin-session claude-code:572ee51e-2f53-4729-96f8-f122a2cc8305 "Tell user-originated tasks apart from agent-originated ones so brainstorming questions reach the right person" -b "At present, probably 90+% of the tasks are created by other agents, not me (user), which means that I do not have the context to understand the task"`.
- Result L159: `{"id":"own-fac412","action":"created","at":"2026-10-25T13:29:08Z","warnings":[]}`.
- Outcome: **matched**. Rule: full result, `at` equal.

**Stage 2** (`origin_session: claude-code:572ee51e…`).
- Evidence from the filing session that points into the older session:
  - `S` L153 (13:29:01.871) is a read of `~/.claude/projects/-mnt-ssd-Dropbox-obs/572ee51e-2f53-4729-96f8-f122a2cc8305.jsonl`
    that selects user records at `2026-10-25T10:59:13`.
  - Its result L154 printed `130 2026-10-25T10:59:13.403Z Let's pause the obs-8c7c17 effort …`,
    which is line 130 of the older session.
  - The record's body quotes a sentence from that turn.
- Opened the older session:

      ls ~/.claude/projects/*/572ee51e-2f53-4729-96f8-f122a2cc8305.jsonl
      # scan: user records containing "probably 90+% of the tasks are created by other agents"

  - Line 130 (10:59:13.403Z) is a human record. It is not `isMeta` or `isSidechain`, does
    not consist only of `tool_result` blocks, and does not start with `<`.
  - It is the only human record that holds the quoted words. Line 127 is an
    `api-request-blob` copy.
- User's words vs pasted text in that turn:
  - One paragraph begins with `> ` ("I can write the spec straight from the record and
    code, but first I have one question …"). It is pasted agent text.
  - The rest is the user's own, including "Are you able to tell who created the task?",
    "At present, probably 90+% of the tasks are created by other agents, not me (user),
    which means that I do not have the context to understand the task" and "do tasks
    differentiate ones a user created vs. agent originated ones?".
- Outcome: **matched**. Rule: "the filing session's evidence names a specific location in
  it (such as a `session-logs` read that printed the turn)". This agrees with the field
  `origin: user` under the README's reading rule 1.

## own-df3b0c: stage 1 (filing) and stage 2 (older request)

**Stage 1.**
- Stamp: `created` at 13:29:11. Transcript: `S`.
- Candidate: `S` L163, 13:29:11.454:
  `… add --origin-session codex:00000000-0000-4000-8000-00000000dead "Demo: older request in an unreadable session"`.
- Result L164: `{"id":"own-df3b0c","action":"created","at":"2026-10-25T13:29:11Z","warnings":[]}`.
- Outcome: **matched**. Rule: full result, `at` equal.

**Stage 2.**
- Filing-session evidence: `S` contains no read of this session and no quote from it.
- Searched every store on this host:

      ls ~/.codex/sessions/*/*/*/*00000000-0000-4000-8000-00000000dead* ~/.codex/archived_sessions/*00000000-…dead*
      find ~/.codex ~/.codex-work ~/.claude ~/.claude-work -name '*00000000-0000-4000-8000-00000000dead*'

  Every search found nothing.
- Outcome: **unavailable**. Rule (step 7): "When the older session cannot be read, the
  older request is unavailable. The filing event's outcome stands on its own." The session
  is not in this host's stores, which can mean another host or a pruned store.

## Limit: an unstamped older record (tasks-fa3525)

    tasks show tasks-fa3525          # real tracker, read-only, the one allowed bare call

- created 2026-10-20T11:40:51Z, `agent: claude-code`, tags feedback, friction, from:lore, no `origin` field.
- It has 11 notes. None has text `created`, and the first note is `attached: …` at
  11:41:06Z. None is a `feedback from` note.
- The record predates the creation-note writer, and existing records are not backfilled.
- Outcome, stage 1: **unavailable (no stamp)**. Rule (step 2): "With no stamp, the outcome is unavailable."
- Stage 2: not applicable (no occurrence names `origin_session`).
- A later plain note (11:41:07Z) is stamped `claude-code:737a5757…` and says the audit ran
  in that session. It is not an occurrence note, so the procedure does not use it as a
  filing stamp. Searching for the call by id or time is not part of stage 1 either.
- Origin: unknown (field absent, no occurrence labels).

## Limit: an archived Codex session (`~/.codex/archived_sessions`)

    ls ~/.codex/archived_sessions | wc -l                  # 594 rollouts, 2026-01 .. 2026-07
    grep -l -E 'tasks[^a-z]{1,6}(add|feedback)' …/*.jsonl  # 16 files mention the words
    python3 -I codex_calls.py <those 16>                   # function_call / custom_tool_call inputs only

- **No archived rollout holds a `tasks add` or `tasks feedback` tool call with an id in its
  output.**
  - The 16 text hits are instruction text and file contents (for example `science-tool tasks add`
    in grep output), plus 2 `spawn_agent` arguments in one rollout whose brief text mentions the words.
  - The archive spans 2026-01 to 2026-07, before this tracker's prefix ids were in use.
  - So the case as specified has no instance on this host.
- I checked whether an archived session can be located by id at all, using
  `019ef267-48c9-71e3-a64b-1f7869355e6b`:

      ls ~/.codex/sessions/*/*/*/*019ef267-48c9-71e3-a64b-1f7869355e6b*    # not found
      ls ~/.codex/archived_sessions/*019ef267-48c9-71e3-a64b-1f7869355e6b* # found; session_meta.id matches

- Outcome: **reachable by hand, not through the session-logs tooling.**
  - The session-logs skill's store table lists only `~/.codex/sessions/YYYY/MM/DD/`.
  - `session-episodes` scans `SESSION_LOGS_CODEX` or `~/.codex/sessions` and has no
    lookup-by-id command; its `show` takes episode ids.
  - Reader gap: archived Codex sessions (and `~/.codex-work`) are outside the tool's default
    roots and the skill's table. The scope skill names them in step 3, and obs-d98663, the
    reader that skill defers to, is still `todo`.

## Limit: a Codex subagent filing, followed through its parent thread

Attachments on tasks-fa3525 (sha256):

| file | sha256 |
|---|---|
| audit-labels.csv | `976b878d…69d4` |
| audit-method.tar.gz | `24656e7d…a6e8` |
| coverage-drop-audit.md | `63efc83b…d236` |

    mkdir …/audit-extract/audit-method && tar -xzf …/audit-method.tar.gz -C …/audit-extract/audit-method --no-same-owner

- The tarball holds `bin/{locate,locate2,inventory,packets}.py` and `data/judge-prompt.md`.
  I read them only with grep and ran nothing from them.
- `packets.py` L85–101 reads `session_meta.source.subagent.thread_spawn.parent_thread_id`
  and, for Codex subagent origins, adds the parent thread's human turns.
- `coverage-drop-audit.md` says six origins were Codex subagents. It does not list which.
  I found five of them by scanning the Codex stores for subagent rollouts (by first line)
  whose `tasks feedback` output names an audited id:

      python3 -I subagent_filings.py audit-ids.txt ~/.codex/sessions ~/.codex/archived_sessions ~/.codex-work/sessions
      # tasks-ae03ed, ops-4e97bf, relay-ebd6f5, tasks-f27fe3, ops-39b8e9

Path for **tasks-ae03ed** (created 2026-10-13T20:17:11Z, `agent: codex`, no creation note):

1. **Subagent session.** `~/.codex/sessions/2026/10/13/rollout-2026-10-13T16-13-32-01a136e6-1cc1-7c73-9357-3178cb6d97ab.jsonl`.
   - `session_meta`: `thread_source: subagent`,
     `source.subagent.thread_spawn = {"parent_thread_id": "01a12f9f-8550-7973-832b-e6fd98ffb80c", "depth": 1, "agent_path": "/root/task15_gate7", …}`.
   - Filing call L171, 20:17:11.145Z: `tasks feedback --project tasks "A cross-project note could not find a task known in a linked worktree; rerunning from its worktree succeeded, contrary to write-command routing guidance." --category friction`
     (its `workdir`, another project's worktree, is omitted here).
   - Output L175: `{"id":"tasks-ae03ed","action":"created",…}`. There is no `at` (older binary).
2. **Parent thread.** `~/.codex/sessions/2026/10/12/rollout-2026-10-12T06-19-05-01a12f9f-8550-7973-832b-e6fd98ffb80c.jsonl`.
   - L4376 at 20:13:32.515Z is `spawn_agent` with `task_name: "task15_gate7"`, which
     matches the child's `agent_path`. Its `message` (the dispatch brief) is encrypted (`gAAAA…`).
3. **The ask.** I listed the parent's human turns up to the filing time.
   - The last one before the spawn is L4357 at 20:11:20Z, "C3", a two-character choice
     between options the agent had offered.
   - The session's opening request (L12) asks to implement a plan with subagent-driven
     development. Its project-specific path is omitted.
   - No human turn asks for this report or names the friction it describes.

Outcomes:
- **Stage 1 by the procedure as written: unavailable (no stamp)**, because the record
  predates the creation note. The audit bound it by title and time.
  - If the call is bound that way, it is the one full-result `created` candidate for this
    title in the subagent session.
  - Under the stamped procedure the record would carry `harness_session codex:01a136e6…`
    and `root_session codex:01a12f9f…`. Here the parent is the root (depth 1).
- **Parent-thread path: reachable.** It runs subagent → `thread_spawn.parent_thread_id` →
  parent rollout → `spawn_agent` (matched by `task_name`) → parent's human turns.
  - The ask behind the filing is the subagent's own observation during its dispatched
    work. The brief that would show any instruction to report is encrypted, and the
    parent's human turns hold no such ask.
- Reader gap: the session-logs skill does not document `source.subagent.thread_spawn` or
  the encrypted `spawn_agent` brief, and `session-episodes` skips subagent rollouts. I
  walked the path by hand from the store layout.

## Limit: another host

Records do not name the host that filed them. A stamp whose session is in none of this
host's stores (as for own-df3b0c's origin session) cannot be told apart from a pruned
store or a session on another host. I did not search other hosts, as the skill instructs:
"a session filed on another host is a stated limitation, not a search."

## Where the skill text left room for doubt

1. **Hook-refused call** (own-0677b0 L53): the skill names full, id-only and unreadable
   results, but not a call refused before it ran. I excluded it on time (it returned before
   the note's `at`), and also because it shows no action.
2. **Telling whether stderr was captured:** a Claude Code record carries a `stderr` field
   that is empty even when stderr was emitted, because the harness merges stderr into
   `stdout`. A reader who trusts the field would settle no sourced add (ambiguous); one who
   knows the merge settles it. The skill does not say how to establish capture.
3. **Where Claude Code subagent filings live:** the stamp names the parent session, but the
   calls are in `<session-uuid>/subagents/agent-*.jsonl`. Neither the session-logs skill's
   table nor its id-search snippet (`projects/*/*.jsonl`) reaches that directory, and
   `session-episodes` does not either.
4. **Duplicate records:** `api-request-blob` records repeat `tool_use` blocks, which look like
   extra candidates unless they are deduplicated by `tool_use` id.
5. **Time fit for a same-argument call after the note** (own-0465cd variant, L130): this
   call started 5 s after the creation `at`. The skill's "both are listed" wording kept it
   in the list.

## Disclosure

While locating the filing subagent in `F`, I saw these short fragments, though I did not
read the full records:
- the first ~300 characters of the filer's dispatch prompt (L844);
- one controller progress line that lists case letters against ids (L880, in a command
  that appends to a file under `.superpowers/`);
- the first 260 characters of the filer's last command (`S` L168), which writes a file
  under `.superpowers/`.

I opened no file under `.superpowers/`. Every outcome above rests on the record reads and
on the transcript calls cited.
