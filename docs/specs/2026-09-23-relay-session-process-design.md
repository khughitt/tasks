# Relay identity: the session's own process and registry schema 2

Task: tasks-2dd094. Amends `docs/specs/2026-09-22-relay-ancestry-identity-design.md`
(tasks-8921f4), which stays authoritative for everything this document does not change.
Upstream: relay `docs/specs/2026-09-23-session-process-handle.md` (relay-cb616b, approved
after two reviews, implemented at relay `45f4c47`). That spec fixes the recognition table,
the non-session roles, the schema bump and the verification it assigns to tasks. This
document settles only how tasks adopts them.

Status: approved 2026-09-23; implemented and verified live on Linux, 2026-09-23.

## 1. Decision and scope

Relay now publishes, as `Agent.process`, the handle of the process that *is* the session:
the nearest harness process, with no terminal requirement, recognized for Claude Code by
`comm` `claude` or by a version-shaped `comm` whose executable is
`…/claude/versions/<comm>`. A nearest Claude process that is the background daemon or a pty
host yields no handle. The registry moves to schema 2, and a schema-1 file is superseded.

tasks adopts the same rule in the one walk it already has. That walk serves both the scope
test (spec §4.2) and the nearest-boundary condition of ownership proof (spec §6.2), so
both change together, which is the point: a process that relay publishes as a session must
be the process tasks treats as the boundary, or every acquisition in a background session
refuses, and a nested `2.1.280` session under an outer `claude` could continue the outer
session's claims by proof.

In scope: the recognition table and roles in the ancestry walk, the harness a boundary
carries, the schema-2 reader and the schema-1 refusal, the handle-less refusal text, the
README paragraph, the vendored `ancestry.json` corpus, and the version that names the
minimum. Out of scope: everything else in the parent design (configuration, the ladder,
the match predicates, continuity, `note`), which does not change; Darwin, where tasks'
relay level is already refused (parent §7); relay's terminal lookup, which tasks never
used.

## 2. The walk

### 2.1 Recognizing a harness process

At each level the walk reads `/proc/<pid>/stat` as today. A level is a harness process when:

| Harness | Rule |
| --- | --- |
| `claude-code` | `comm` is `claude`, **or** `comm` matches `^[0-9]+(\.[0-9]+)+$` and `readlink /proc/<pid>/exe` ends in `/claude/versions/<comm>` |
| `codex` | `comm` is `codex` |
| `opencode` | `comm` is `opencode` |

- `exe` is read only for a version-shaped `comm`: the shape gates the read, the path
  decides. No other ancestor's `exe` is read.
- A trailing ` (deleted)` is stripped from the link before comparison, as relay does. The
  kernel appends it when the binary is replaced on disk, which a Claude Code auto-update
  does to every running session; without the strip, an updated background session stops
  being a harness and the walk continues outward to a process that is not the caller's.
- A version-shaped ancestor whose `exe` names anything else (relay's `foreign-version-comm`
  chain, `/opt/tool/1.2.3`) is not a harness, and the walk continues past it.
- An unreadable `exe`, or a link that is not an absolute path, is **unknown ancestry**,
  never "not a harness". This is the parent design's constraint §2.3 applied to a new
  read. A plain shell under a version-shaped process owned by another user is therefore
  refused in relay mode; that shape has not been observed, and the opt-in's recovery (set
  `TASKS_SESSION`) applies.

### 2.2 Non-session roles

When the nearest harness process is `claude-code`, the walk reads `/proc/<pid>/cmdline`
for that process only, splits it on NUL (dropping one trailing NUL; empty text is an
empty argv), and inspects `argv[1]`:

| `argv[1]` | Role |
| --- | --- |
| `daemon` | the background-session daemon |
| `bg-pty-host` | the pty host of one background session |

A role makes the boundary a **hosting process**, not a session. The walk stops there
exactly as it stops at any nearest harness; it never continues outward past the nearest
harness process to find a session. An unreadable `cmdline` is unknown ancestry. Codex and
OpenCode have no roles and their `cmdline` is never read.

### 2.3 What the walk returns

`Scope` becomes:

```rust
pub enum Scope {
    /// The nearest harness process, and it is a session.
    Harness(Boundary),
    /// The nearest harness process is a Claude Code hosting process (§2.2).
    Hosting(Boundary, &'static str),        // the role
    /// The whole chain was readable and held no harness.
    Outside,
    /// A read failed at this pid. Not evidence of a plain shell.
    Unknown { pid: u32, file: &'static str }, // "stat", "exe" or "cmdline"
}

pub struct Boundary {
    pub entry: ProcEntry,           // pid, ppid, comm, start — unchanged
    pub harness: &'static str,      // "claude-code", "codex" or "opencode"
}
```

The boundary carries its harness because `comm` no longer determines it: a `2.1.280`
process is `claude-code`. Every consumer that today maps `comm` to a harness —
`resolve::harness_for`, `hint_for`, `same_session`, and the harness-agreement predicate —
takes the boundary's `harness` instead, and `HARNESS_COMMS` gives way to the table above.
The scoped hints are keyed by harness (`claude-code` compares `CLAUDE_CODE_SESSION_ID`),
so a background session's `2.1.280` boundary checks the same variable an interactive
`claude` boundary does.

The walk's reads are injected as today (`read_stat` plus two new readers for `exe` and
`cmdline`), so unit tests stay hermetic.

### 2.4 Consequences for resolution and proof

- **Resolution** (parent §4.3). `Hosting` is in scope, like any harness boundary, and is
  refused before the registry is read: "the nearest Claude Code process, pid N, is its
  `daemon` (or `bg-pty-host`), not a session", with the usual `TASKS_SESSION` recovery.
  Relay publishes no handle there, so no registry row could match. `Unknown` names the
  file that could not be read.
- **Proof** (parent §6.2). Condition 2 compares the claim's pid and start with the
  `Harness` boundary. `Hosting`, `Outside` and `Unknown` prove nothing, as `Outside` and
  `Unknown` already do. No change to `proves_ownership` beyond the type: the stronger
  guarantee comes from the walk. In the held-claim case, an outer `claude` holds a claim
  and a nested `2.1.280` runs under it; the nested caller's boundary is now the `2.1.280`
  process, whose pid is not the claim's, so `start`, `park` and `done` refuse as foreign
  and fall to acquisition, which needs a resolved identity of its own.

## 3. Registry schema 2

`snapshot::parse` accepts `schema: 2` only. The agent and handle shapes do not change in
schema 2; the version marks the new meaning of `Agent.process`.

- `schema: 1` is refused with its own message: "relay registry: schema 1 is superseded by
  schema 2; run `relay reap`, or wait for any hook event". Like every other validation
  failure it does not name the path, which the parser never sees. The resolver wraps it
  with the `TASKS_SESSION` recovery as it wraps every registry error. It is never read as
  an empty registry.
- Any other value stays "schema must be 2".

A schema-1 file can hold rows that borrowed another session's handle (relay spec, "Rows
written under the old rule"). Refusing it is what keeps a new tasks from matching one.
Held claims are unaffected: continuation never reads the registry.

## 4. Refusal text and README

**Handle-less agent** (tasks-962300, `resolve::no_match`). The diagnosis stays; its reason
changes. A handle-less row for this session now means relay's hook found no session
process of that harness among its ancestors, or found a hosting process first. The message
becomes: "agent <id> is this <harness> session but relay recorded no process handle for
it: relay's hook found no <harness> session process among its ancestors, or found only a
daemon or pty host". The test's "controlling terminal" assertion goes.

**README**, "Relay identity (opt-in)":

- The sentence saying a headless session (`claude -p`, `codex exec`) is published without
  a handle and must use `TASKS_SESSION` is replaced: headless, nested and background
  sessions each carry their own handle and claim as themselves.
- "Walks its own process ancestry to the nearest harness process" gains the recognition
  rule in one sentence (a Claude Code process is `claude` or a versioned binary under
  `claude/versions/`; its daemon and pty host are not sessions).
- A new paragraph states the rollout precondition from the relay spec: with the opt-in on,
  every `tasks` that can run on the host is **0.2.0 or later** before relay publishes
  schema 2 there, checked with `tasks --version` for each `tasks` on `PATH` in each
  harness's environment. An older `tasks` reads a schema-2 registry as invalid and refuses
  fresh acquisition, but can still continue an outer session's held claim from a nested
  session, which no registry format can stop.

**Version.** `Cargo.toml` moves from 0.1.0 to **0.2.0** in the same change, and the README
names 0.2.0 as the minimum. tasks has no tagged releases, and the relay spec's
precondition needs something an operator can read off an installed binary. Rejected:
naming the commit, which `tasks --version` cannot show and which a `cargo install` from a
later checkout would make meaningless.

The tasks skill's relay paragraph says nothing about headless sessions or schema and does
not change.

## 5. The vendored corpus

relay's `test/fixtures/ancestry.json` is copied to `tests/fixtures/relay/ancestry.json`
from relay `45f4c47`, byte for byte. A sibling `tests/fixtures/relay/README.md` records
the source path, the pinned revision, and that the file is refreshed only by copying a
newer relay revision and updating both. The corpus's own `schema: 1` is its fixture format
version, unrelated to the registry schema.

A unit test in `src/relay/ancestry.rs` reads it with `include_str!` and runs every chain
through the walk. Each row supplies `stat` (pid, ppid, comm, and a synthetic start),
`exe` and `argv`; a `null` `exe` or `argv` is an unreadable read, so a chain that needed
one it does not carry fails as unknown rather than passing. The walk starts at row 0's
parent, as tasks is the process at row 0 (relay's hook). Assertions per chain:

- `session` non-null → `Scope::Harness` at that pid, with the chain's harness.
- `session` null → the exact scope named for that chain in a tasks-local table, since
  relay's null does not say which of tasks' outcomes applies. At `45f4c47`:

  | Chain | Expected |
  | --- | --- |
  | `daemon-nearest` | `Hosting` at pid 300, role `daemon` |
  | `pty-host-nearest` | `Hosting` at pid 400, role `bg-pty-host` |
  | `claude-ancestor-under-codex` | `Unknown { pid: 20, file: "cmdline" }` |

  The last one is not a hosting verdict and not a Claude boundary: relay walks for the
  `codex` harness and never reads the `claude` row's arguments, so the corpus carries
  `argv: null` for it, while tasks walks for every harness at once, meets that `claude`
  process first and must read its `cmdline` (§2.2). An unreadable read is unknown, so
  the vendored chain pins that rule too.
- A chain with a null `session` that the table does not list fails the test, so a
  refreshed corpus forces a decision instead of passing on a loose predicate.

The readable cross-harness case, which the corpus cannot express, is a tasks-local unit
test beside it: the same `claude-ancestor-under-codex` rows with `argv: ["claude"]` on
pid 20 return `Harness` at pid 20 with harness `claude-code`. The vendored file stays byte
for byte.

The `terminal` column is not asserted; tasks has no terminal lookup. The Darwin chains
run too: the walk is pure over its readers, and their comm-only rows need no `exe`.

## 6. Testing

Unit, against injected readers:

- The corpus test of §5.
- An `exe` ending in ` (deleted)` still matches; a relative or unreadable `exe` for a
  version-shaped ancestor is `Unknown { file: "exe" }`; a non-version `comm` never has its
  `exe` read (the reader panics if called).
- `cmdline` is read only for the nearest `claude-code` process; unreadable is
  `Unknown { file: "cmdline" }`; an empty argv is a session.
- Resolution: a `2.1.280` boundary matches a `claude-code` row and compares
  `CLAUDE_CODE_SESSION_ID`; `Hosting` refuses without calling the loader and names the
  role; the harness-agreement predicate uses the boundary's harness.
- Proof: `same_session` and `hint_for` by harness; a `Hosting` scope never proves.
- Snapshot: schema 2 parses; schema 1 is refused with the supersession text; schema 3
  is refused as invalid. The existing parse tests move to schema 2.

End-to-end in `tests/cli.rs`, extending the harness shims:

- A **version shim**: `sh` copied to `<home>/.local/share/claude/versions/2.1.280` and run
  by that path, so its `comm` is `2.1.280` and its `exe` ends in the versions path. The
  copy is made by a `cp` child process rather than by the test thread, so no sibling
  thread can inherit a writable descriptor to it and fail its `execve` with `ETXTBSY`
  (the hazard `harness_shim` documents).
- `WRITE_REGISTRY` writes schema 2.
- **Fresh acquisition fails closed in both pairings**: a schema-1 registry refuses with
  the supersession message. The other pairing (an older tasks against schema 2) is the
  current binary's refusal, which is not re-tested here; it is recorded as covered by
  the `schema must be 1` check the old reader has.
- **Background session**: a version shim publishing its own row acquires a claim keyed
  by its agent id with its own pid.
- **Held claim**: an outer `claude` shim acquires a claim, then runs a nested version shim
  that tries `start`, `park` and `done` on it, once with no hint and once with the outer
  `CLAUDE_CODE_SESSION_ID` inherited. All three refuse, and the claim still names the
  outer session.
- **The review chain**: tasks → version shim → `claude` shim with a correct outer row and
  no hint: refused, naming the `2.1.280` pid, never claiming as the outer session.
- **Hosting**: a version shim whose argv[1] is `bg-pty-host` refuses before reading the
  registry. `sh -c` cannot put a chosen word in argv[1], so the copied `sh` runs a script
  *file* named `bg-pty-host`, by that relative name, which leaves its cmdline as
  `[<versions path>, "bg-pty-host"]`. The same shape with `daemon` under a `claude` shim.

Live, on this host, before `done`: with the opt-in on, a private `RELAY_STATE_DIR`, and
relay at `45f4c47` or later installed as the hook for a scratch Claude Code session, an
outer interactive session and a nested `claude -p` each acquire their own claim. The run
uses the worktree's binary by explicit path and a scratch `XDG_CONFIG_HOME`; no host
pointer is repointed. If relay's hooks cannot be attached to a scratch session without
changing host configuration, the step parks `--reason environment` with the exact
command, rather than being skipped.

## 7. Rollout

This lands before relay's resolver is released (relay-c19c0a depends on this task).
After it lands, `cargo install --path .` on each host with the opt-in on; no host has it
on today. Nothing is migrated: claims keep their recorded proof, and a claim acquired by
the old walk from an interactive `claude` boundary is still proved by the new walk,
which names the same process.

## 8. Decomposition

Plan steps, in order. Each commit builds green, so a change to `Scope` lands with every
consumer it breaks:

1. The walk and what reads it: the vendored corpus and its README, `Boundary`, the
   recognition table, the `exe` and `cmdline` readers, roles, `Scope::Hosting` and
   `Unknown { file }`, the corpus test, and resolution and proof keyed by harness,
   including the `Hosting` refusal.
2. Schema 2: the reader, the supersession refusal, the handle-less text, and the test
   registries moved to schema 2.
3. End-to-end: the version shim and the acceptance cases of §6.
4. README, version 0.2.0, the parent spec's amendment notes, and reinstall.
5. The live check.
