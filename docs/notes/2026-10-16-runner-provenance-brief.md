# Brief: provenance for runners tasks does not read natively

Scope pass, 2026-10-16. Idea: tasks-b13c2c. Related context: tasks-970049 (Codex model
stamping, shelved until relay-2cd4b1 publishes its call contract).

## Problem

crew runs pi workers on task items, and their notes reach `tasks` with
`TASKS_SESSION=crew:<attempt id>` and `TASKS_OWNER=crew`. Those notes get no harness
provenance, so obs cannot join an attempt to the task it worked. The note shows only
`by: crew`. What's needed is a decision on how a runner that is not Claude Code or Codex
becomes a provenance source, and on the stamp it writes. crew's first slice does not
depend on it, because crew keeps the attempt-to-task join in its own attempt record.

## Current behaviour and evidence

- `src/provenance.rs` (`resolve_from`, `validate`) builds a `HarnessProvenance`
  (`harness_session`, `harness_session_source`) only from `CLAUDE_CODE_SESSION_ID`,
  `CODEX_THREAD_ID` or `CODEX_SESSION_ID`. `validate` refuses any other source. A
  qualified `TASKS_SESSION` (`claude-code:` or `codex:`) is only compared against the
  native value and is never a source. Other `TASKS_SESSION` values never become
  provenance.
- `Note` (`src/model.rs`) stores `at`, `by`, `text` and the optional flattened provenance,
  and nothing else. The session a note was written under is not kept unless it is
  provenance.
- Native-only sourcing comes from ops-79f409 (lifecycle-note provenance). The relay
  identity design (`docs/specs/2026-09-22-relay-ancestry-identity-design.md` §1) keeps
  provenance out of its scope. Its harness table already has a row for a harness with no
  native variable (`opencode`: nothing compared).
- pi 1.1.0 (`pi --help`) has `--session-id <id>` ("Use exact project session ID,
  creating it if missing"), so a runner can choose the pi session id. The help documents
  no environment variable that passes that id to tool subprocesses. That is unknown, not
  absent.

## Constraints

- `TASKS_SESSION` stays authoritative for claims and is never merged with another source
  (relay identity design §2.1).
- Note JSON is the contract. A new `harness_session_source` value or prefix is an
  addition to `validate`'s closed list, so obs's readers must accept it before anything
  writes it.
- crew core design (crew-3fc20c, done): run identity belongs to crew's common runner, and
  an attempt record references the authoritative records.

## Alternatives

1. **pi as a native source.** If pi passes its session id to tool subprocesses in a
   variable, add a `pi:` prefix with that variable as its source, the same way as Claude
   Code and Codex. crew runs each attempt with `--session-id <attempt id>`, or records the
   pi session id in its attempt record. Then obs joins either directly or through crew's
   record. This keeps provenance native and covers pi run by hand too.
2. **Declared runner prefixes on `TASKS_SESSION`.** Accept `TASKS_SESSION=<runner>:<id>`
   as provenance (`harness_session_source: "TASKS_SESSION"`) for runners named in a
   config list, starting with `crew`. This needs nothing from pi. It does make an
   override that any process can set count as provenance, which is the trust line
   ops-79f409 drew.
3. **Through relay.** crew registers each pi worker with relay, and provenance comes from
   the relay match. This reuses identity machinery, but relay's design excludes
   provenance, and pi has no relay adapter yet.

Lean: 1 if the research below finds a variable, otherwise 2 with a declared list.

## Unanswered questions

1. Does pi pass its session id to tool subprocesses, and under what name? Does
   `--session-id` make that id equal to the one crew chose? — research task below.
2. Which id does obs need to join on: the crew attempt id or the pi session id? — obs's
   owner or crew's design (crew docs/specs/2026-10-13-crew-core-design.md §9).
3. Is a declared `TASKS_SESSION` runner acceptable as provenance (alternative 2)? — the
   user, against ops-79f409's native-only rule.

## Proposed decomposition

- tasks-e7f671 — research: does pi pass a session id to tool subprocesses (Q1). Wakes
  tasks-b13c2c.
- tasks-b13c2c waits on that answer, and on Q2 and Q3 where alternative 2 is in play.
  Once decided, it becomes a direct task: a source in `src/provenance.rs`, a `validate`
  prefix, and an obs reader check.
