# Relay ancestry identity for claims

Task: tasks-8921f4. Parent goal: tasks-c9199a.
Approved upstream source: ops `docs/specs/2026-09-16-relay-design.md` §6 "Tasks",
reviewed after d7e2c33. That section fixes the constraints; this document settles the
two things it deferred to tasks — **configuration** and **identity continuity** — and
nothing else.

## 1. Decision and scope

A tasks session can today name itself four ways, in order: the explicit
`TASKS_SESSION`/`TASKS_SESSION_PID` pair, `CLAUDE_CODE_SESSION_ID`,
`CODEX_SESSION_ID`/`CODEX_THREAD_ID`, and the Unix session-leader pid (`sid:<pid>`).
Each level yields a session string and, where the level can prove it, a pid. Nothing
in that ladder can name an OpenCode session at all, and a Codex claim carries no pid,
so it lives by the TTL rather than by process proof.

Relay publishes a validated registry of live agents keyed by `<harness>:<sessionId>`,
each with a process handle. This design adds one **opt-in** level to the ladder that
adopts the matched agent's id as the claim session, and adopts its process handle into
the claim fields tasks already has.

The benefit is a shared **identity**, not delegated liveness. Tasks keeps its own
liveness implementation for every claim, unchanged.

In scope: the configuration surface, the ancestry scope test, the registry match, what
is written into a claim, the error surface, and continuity across commands and mode
changes. Out of scope: any change to liveness logic, to the claim file format, to park
or escalation records, to lifecycle-note provenance (`harness_session`, settled by
ops-79f409 and already shipped), and any Node process spawned by tasks — the registry
is read directly in Rust.

## 2. Constraints taken as given

From ops §6, not re-litigated here:

1. `TASKS_SESSION`/`TASKS_SESSION_PID` stay authoritative and are never merged with a
   session or pid from another source.
2. Relay identity is explicitly configured, and is scoped to harness callers even when
   enabled host-wide. Outside a harness tree, native identity is used **without reading
   the registry**.
3. Unreadable ancestry is *unknown*, not evidence of a plain shell.
4. Matching is by pid **and** start token, plus boot id on Linux. The nearest matching
   ancestor is chosen only when exactly one registry agent matches it; an unmatched
   inner harness is never skipped to reach an outer one.
5. Exact process identity does not imply distinct logical subagents. Agents sharing one
   process must set `TASKS_SESSION` to claim independently.
6. A missing record, missing proof, or ambiguous match inside a configured harness tree
   is a visible identity error on new acquisition, never a silent fall to terminal
   identity.
7. An already-held claim keeps its identity through refresh and release. Relay identity
   is adopted between sessions, never during one.
8. Every ownership decision stays under the existing mutation lock.

## 3. Configuration

Relay identity is enabled by a new host-local file, `$XDG_CONFIG_HOME/tasks/config.toml`,
falling back to `$HOME/.config/tasks/config.toml` — the same resolution
`Registry::path` already uses for `projects.toml`, and an error when neither variable
is set:

```toml
[identity]
relay = true
```

- A missing file, or a file without `[identity].relay`, means relay identity is off.
- Malformed TOML, a non-boolean `relay`, or an unknown key under `[identity]` is a
  typed `Config` error. The file is host-local and hand-written, so a typo must be
  loud rather than silently disable the feature.
- Unknown top-level tables are an error for the same reason.

Rejected: a key in `projects.toml` (that file maps prefixes to roots; identity is not
a property of the project set), a `TASKS_RELAY` environment variable (a fifth variable
in a ladder that already has four, and trivially inconsistent between a harness and a
plain shell), and a key in the per-project `tasks/.config.toml` (it is committed and
syncs between hosts, so enabling relay in one repo would enable it on a machine with no
registry).

The file is read only where identity is resolved. `claims::identity` is reached from
`Ctx::claim_guard`, `Ctx::refuse_foreign_live_claim`, `commands::park` and
`commands::status` — the claim-mutating paths. Read commands, `check`, and shell
completion never open it, never walk `/proc`, and never read the registry.

## 4. Resolution

### 4.1 Position in the ladder

The relay level sits immediately below the explicit pair and **above** the native
harness variables:

1. `TASKS_SESSION` (+ `TASKS_SESSION_PID`)
2. **relay ancestry identity**, when configured and in scope
3. `CLAUDE_CODE_SESSION_ID`
4. `CODEX_SESSION_ID` / `CODEX_THREAD_ID`
5. `sid:<pid>`

Relay is above the native variables because the matched `Agent.id` is the same
qualified key those variables would produce, verified against process proof rather
than trusted. When relay is off, or the caller is out of scope, levels 3–5 behave
exactly as they do today.

### 4.2 The scope test

Before the registry is opened, tasks walks its own ancestry on Linux:
`/proc/self/stat` → ppid → `/proc/<ppid>/stat`, upward, starting at the parent and
stopping at pid 1. At each level it reads `comm`, `ppid` and `starttime`.

A level whose `comm` is exactly `claude`, `codex` or `opencode` is a harness ancestor —
the same three names relay's own adapters match (`src/adapters/*.js`). The tty
predicate those adapters apply belongs to qualified process resolution, not to this
test: a headless harness must not silently become a shell caller.

- **No harness ancestor, whole chain readable** → out of scope. Fall to level 3.
  The registry is not opened and the config's `relay = true` has no effect.
- **A harness ancestor found** → in scope, continue to §4.3.
- **The chain becomes unreadable before either verdict** → *unknown*. This is the
  error of §5, not a fall to level 3.

The nearest harness ancestor is the one matched. An unmatched inner harness is an
error; the walk does not continue outward past it to find an outer harness that does
match.

### 4.3 The registry match

The snapshot is read from `$RELAY_STATE_DIR/agents.json`, else
`$XDG_STATE_HOME/relay/agents.json`, else `$HOME/.local/state/relay/agents.json`.
Relay writes it atomically with mode 0600 in a 0700 directory and refuses a
non-private path; tasks reads it, validates schema 1 in Rust, and treats a snapshot
that fails validation as unavailable (§5). Tasks never writes to it and never spawns
Node.

A candidate agent matches the nearest harness ancestor when **all** hold:

- `agent.process` is non-null and `agent.process.host` equals tasks' hostname,
- `agent.process.pid` equals the ancestor pid,
- `agent.process.start` equals the ancestor's `starttime` as canonical decimal text,
- on Linux, `agent.process.bootId` equals `/proc/sys/kernel/random/boot_id`.

Exactly one match adopts. Zero matches, or more than one, is the error of §5 — neither
the working directory nor a bare pid breaks a tie.

When `CLAUDE_CODE_SESSION_ID`, `CODEX_SESSION_ID` or `CODEX_THREAD_ID` is set, it is a
**hint that must agree**: the matched agent's `sessionId` must equal it, or acquisition
fails. A disagreement means the process proof and the environment describe different
sessions, and guessing which is right is exactly what §2.6 forbids.

### 4.4 What is adopted

On a match, the resolved `Identity` is:

- `session` = `agent.id`, i.e. `<harness>:<sessionId>` — the claim key.
- `tagged` = the same string. In relay mode the raw and tagged forms converge; they
  differ today only because the native levels store a raw id and qualify it separately.
  Note that the historical Claude tag is `claude:<id>` while a relay agent id is
  `claude-code:<id>`; they are different strings and this design does not rewrite
  existing records that carry the old form.
- `pid` = `agent.process.pid`.

The claim then records the matched proof in the fields it already has:
`pid_start` = `agent.process.start` parsed as `u64`, and `boot_id` =
`agent.process.bootId`, **only when `agent.process.platform` is `"linux"`**. A Darwin
handle's `start` is an epoch, not proc ticks, and must never be written into
`pid_start`; on a non-Linux handle tasks records `pid` alone and the claim falls to the
existing TTL path.

Under §7 a Linux host cannot legitimately meet a Darwin handle — `process.host` must
equal its own hostname. The platform check is kept anyway, as a schema guard rather
than a live branch: schema 1 permits `platform: "darwin"`, so a corrupt or foreign
snapshot can present one, and the cost of refusing to treat its `start` as proc ticks
is a single comparison. It is tested as a format case, not as a supported platform.

This is a real gain for Codex, whose claims carry no pid today and therefore live by
the TTL: under relay mode a Codex claim gains pid, start and boot proof, and its
liveness becomes verifiable by the existing implementation with no change to that
implementation.

## 5. Errors

Inside a configured harness tree, each of these is a typed `Config` error on **new
acquisition**, naming the cause and instructing the caller to set
`TASKS_SESSION`/`TASKS_SESSION_PID`:

| Cause | Message names |
|---|---|
| Ancestry unreadable before a verdict | the pid whose `stat` could not be read |
| Registry absent, unreadable, or not private | the path tried |
| Snapshot fails schema-1 validation | the validation failure |
| No agent matches the nearest harness ancestor | the harness comm and pid |
| More than one agent matches | the matching agent ids |
| A native session variable disagrees with the match | both ids and the variable |

The "no agent matches" case includes the real Codex situation where a live harness
ancestor exists but the registry is empty because SessionStart runs at the first turn,
not at launch. That is an error with recovery instructions, not terminal identity.

None of these is a warning, and none degrades to a lower level: reaching level 3 after
the scope test said "in scope" is the silent identity switch §2.6 forbids.

## 6. Continuity

- **Within a session.** Every claim-mutating command resolves identity independently;
  there is no cache. The inputs — ancestry, registry, config — are stable within a
  session, and a cache would be a second source of truth for the thing this design
  exists to make single. The cost is one `/proc` walk and one JSON read per claim
  mutation, not per command.
- **A held claim.** `claim_guard` compares `existing.session` to the resolved session.
  A claim acquired under native identity and later met by a relay-resolved session is
  a *foreign* claim by that comparison, which is correct: they are different identity
  schemes and tasks cannot prove they are the same session. The mode change is
  therefore made between sessions, per §2.7. A session that enables relay mid-flight
  and finds its own earlier claim foreign takes it over with `start --force`, which
  records the takeover in the task's notes as it does today.
- **Registry loss while holding a claim.** Refresh and release read
  `existing.session`, not a fresh registry lookup, so a claim already held stays held
  and releasable with the registry gone. Liveness reads only the captured claim proof
  and native process data; the registry is never consulted to decide whether an owner
  is alive.
- **Recovery.** The explicit pair is the documented escape from every error in §5 and
  keeps working with relay enabled, since it sits above the relay level.

## 7. Platform

Relay ancestry identity is Linux-only in this design: the ancestry walk reads `/proc`,
and no other process-tree source is implemented. `relay = true` on a platform without
`/proc` is a `Config` error at load, naming the platform, rather than a configuration
that resolves every session to "unknown ancestry". Because the config file is
host-local, a machine without a readable process tree simply does not enable it, and
nothing about tasks' behaviour there changes — including its existing
unverifiable/TTL liveness path, which this design does not touch.

Native Darwin claim liveness, and a Darwin process-tree source, are separate work and
are not prerequisites for this one.

Rejected: accepting the flag everywhere and letting §4.2's unknown-ancestry error fire
per command — it converts a one-time configuration mistake into a per-command failure
with no better diagnosis.

## 8. Implementation shape

- A new `src/relay.rs` holds the snapshot types, schema-1 validation, and the path
  resolution. It has no dependency on `claims`.
- A new ancestry walk lives beside it, returning `{ pid, comm, ppid, starttime }` per
  level. `ProcStat::Found` carries only state and starttime and is consumed by
  liveness; rather than widen that public enum for a different consumer, the walk gets
  its own row type and both share the "fields after the last `)`" parsing rule, which
  is already documented and tested at `src/claims.rs:662`.
- `src/config.rs` holds the new host config file and its typed errors.
- `claims::identity_from` gains the relay level. Its existing signature already takes
  the environment and the session pid as parameters for testability; the relay level
  is injected the same way, so the unit tests stay hermetic and no test reads the real
  `/proc` or the real registry.

## 9. Testing

Unit, against injected inputs:

- The ladder: explicit pair beats relay; relay beats both native variables; out of
  scope falls to the native variables unchanged; relay off leaves today's behaviour
  byte-for-byte.
- The scope test: no harness ancestor with a readable chain, a harness ancestor at
  each depth, an unreadable level before a verdict, and an unmatched inner harness
  that must not be skipped for a matching outer one.
- The match: exact match; wrong pid; wrong start; wrong boot id; wrong host; two
  agents on one handle; a null process handle; a Darwin handle (pid adopted,
  `pid_start` and `boot_id` absent).
- Agreement: a native variable equal to the matched `sessionId`; one that disagrees.
- Schema: versioned handle fixtures copied from relay-06b1da, including the oversized
  Linux `start` value, labelled as a format test.
- The Codex case: a live `codex` ancestor with an empty registry must produce the
  explicit-identity error, not terminal identity.
- Config: missing file; `relay = false`; `relay = true`; malformed TOML; non-boolean;
  unknown key; neither `XDG_CONFIG_HOME` nor `HOME`.

End-to-end in `tests/cli.rs`, against the built binary in temp repos with
`XDG_CONFIG_HOME`, `RELAY_STATE_DIR` and a fixture snapshot:

- Acquisition under relay mode writes a claim keyed by the agent id with the handle's
  proof in `pid`, `pid_start` and `boot_id`.
- Takeover and release with the registry removed after acquisition.
- Each §5 error reaches stderr as a `config` error object at exit 1.
- Concurrent acquisition under relay mode still produces exactly one winner, under the
  existing mutation lock.

## 10. Decomposition

Suggested step children for the implementation plan, in order: the host config file;
the relay snapshot reader and schema-1 validation; the ancestry walk; the relay level
in `identity_from` with its errors; claim-field adoption and the Darwin rule; the
end-to-end tests and the skill and README updates. The plan is written and reviewed
before any of it is implemented.
