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
`Ctx::claim_guard` (`src/commands/mod.rs:169`), `Ctx::refuse_foreign_live_claim`
(`:121`, called from `edit.rs:144` and `:257`), `commands::park` (`:38`) and
`commands::status` (`:89`). `note` is not among them. Read commands, `check` and shell
completion perform **no relay ancestry walk and no registry access** — they still read
`/proc` for claim liveness, as they do today, and that path is untouched.

The file is also read below the explicit override: `TASKS_SESSION` is resolved first,
and a session that sets it never opens this file, never walks ancestry and never reads
the registry. The platform refusal of §7 is part of the relay level, not a startup
check, so it cannot take away the explicit pair's role as the recovery path.

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

- `agent.process` is non-null and `agent.process.platform` is `"linux"`,
- `agent.process.host` equals tasks' hostname,
- `agent.harness` equals the ancestor's `comm` under the explicit mapping
  `claude → claude-code`, `codex → codex`, `opencode → opencode`,
- `agent.process.pid` equals the ancestor pid,
- `agent.process.start` equals the ancestor's `starttime` as canonical decimal text,
- `agent.process.bootId` equals `/proc/sys/kernel/random/boot_id`.

The harness comparison is load-bearing and not implied by the rest. Schema 1 validates
`harness` and `process` independently, so a `claude-code` row can carry a process handle
that satisfies host, pid, start and boot for an ancestor whose `comm` is `codex`;
without this check that row would be adopted and the claim keyed to the wrong session.

The platform comparison is likewise load-bearing rather than defensive. A schema-valid
Darwin handle has `bootId: null` and so can never satisfy the boot comparison, and
hostname equality proves nothing about platform. Requiring `"linux"` explicitly states
the rejection instead of leaving it as a side effect of a null comparison.

Exactly one match adopts. Zero matches, or more than one, is the error of §5 — neither
the working directory nor a bare pid breaks a tie.

Native session variables are **hints that must agree, scoped to the nearest harness**.
Only the variables belonging to that harness are compared:

| Nearest `comm` | Compared | Rule |
|---|---|---|
| `claude` | `CLAUDE_CODE_SESSION_ID` | must equal the matched `sessionId` |
| `codex` | `CODEX_SESSION_ID`, `CODEX_THREAD_ID` | both must agree with each other and equal the matched `sessionId` |
| `opencode` | none | no native variable exists; nothing is compared |

A variable belonging to a *different* harness is ignored, because a nested harness
inherits its parent's environment: a `codex` process started inside a Claude session
carries `CLAUDE_CODE_SESSION_ID` from that parent while its own proof and its own
`CODEX_SESSION_ID` are entirely valid. Comparing every set variable would reject that
session despite correct evidence.

An empty variable is treated as unset and compared against nothing, matching how
`identity_from` already filters empty values at every native level. For `codex`, if
only one of the two variables is set it is the one compared; if both are set and
disagree with each other, that is the error of §5 rather than a warning, because unlike
the native level there is a verified session available to contradict.

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
`agent.process.bootId`. Both are unconditional here, because §4.3 has already
established that an adopted handle is Linux: a Darwin `start` is an epoch rather than
proc ticks and must never reach `pid_start`, and the way this design prevents that is
by refusing the handle as an identity candidate, not by adopting it partially.

A Darwin handle therefore never yields a claim, a pid, or a TTL fallback. It remains a
parsing case only: schema 1 permits `platform: "darwin"`, so the reader must parse one
without error and then reject it as an identity candidate.

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
| A harness-scoped variable disagrees with the match | both ids and the variable |
| `CODEX_SESSION_ID` and `CODEX_THREAD_ID` disagree with each other | both values |
| The only candidate's `harness` does not match the ancestor `comm` | both, and the mapping |
| The only candidate's handle is not `platform: "linux"` | the platform and the agent id |

The "no agent matches" case includes the real Codex situation where a live harness
ancestor exists but the registry is empty because SessionStart runs at the first turn,
not at launch. That is an error with recovery instructions, not terminal identity.

None of these is a warning, and none degrades to a lower level: reaching level 3 after
the scope test said "in scope" is the silent identity switch §2.6 forbids.

These are errors **on acquisition**. On a task that already carries a claim they are held
rather than raised, and are raised only if ownership proof also fails to establish the
caller's right to act (§6.2). When raised there, the message additionally names
`existing.session` so the operator can recover with the explicit pair.

## 6. Continuity

### 6.1 Two questions, not one

Every claim-mutating command today asks a single question — is `existing.session` equal
to mine? — and must resolve caller identity to ask it. Under relay that would make the
registry a dependency of *releasing* a claim and not merely of taking one, so a registry
lost mid-session would strand `park`, `done` and `edit` on work already held. Adopting
`existing.session` unconditionally instead is the opposite failure: any caller could then
act as the owner.

The resolution is to stop asking one question:

- **Acquisition** — may this caller take this task? Needs a resolved identity, which
  under relay mode needs the registry. Every §5 error applies, per constraint §2.6.
- **Continuation** — is this caller the session that already holds this claim? Needs
  proof of ownership, which the claim already carries, and never needs the registry.

### 6.2 Proving ownership without the registry

A relay-acquired claim records `host`, `pid`, `pid_start` and `boot_id` copied from the
matched process handle (§4.4) — the same evidence relay itself used. A caller proves
ownership by re-deriving that evidence locally:

1. `claim.host` equals this host and `claim.boot_id` equals the current boot id, and
2. some ancestor of the caller has pid `claim.pid` and `starttime` `claim.pid_start`.

The §4.2 ancestry walk already produces that chain, so the proof costs one `/proc` walk
and no registry read. If both hold, the caller is a descendant of the very process the
claim names, and is its owner.

This is strictly stronger than adopting `existing.session`. The only caller that can
satisfy it without being the owning session is one genuinely descended from the same
harness process — the shared-process case constraint §2.5 already governs by requiring explicit
`TASKS_SESSION` for agents that claim independently.

The ordering inside `claim_guard`, `refuse_foreign_live_claim`, `park` and `status`, all
under the existing mutation lock:

1. `TASKS_SESSION` set → identity match, exactly as today.
2. identity resolved and `existing.session` equals it → owner.
3. ownership proof holds → owner, **even though identity did not resolve**.
4. otherwise → foreign claim: today's `Claimed` refusal, or takeover with `--force`.

An identity-resolution failure at step 2 is therefore no longer fatal on a task that is
already claimed: the error is carried and raised only if steps 3 and 4 also fail to
establish a right to act. A task with no existing claim goes straight to acquisition,
where a resolution failure stays fatal.

### 6.3 Claims that carry no proof

Acquisition derives `pid_start` from `me.pid` (`src/commands/mod.rs:213`), so a claim
carries process proof only when its identity level supplied a pid. Native Claude claims
without `CLAUDE_PID`, and every Codex claim, have `pid: None` and cannot be proved this
way. For those, step 3 is unavailable and behaviour is exactly today's: identity match or
takeover. Relay-acquired claims always carry proof — which is the case that matters,
since registry loss can only strike a session that had the registry when it acquired.

### 6.4 Mode changes, and the `--force` reconciliation

Relay identity is adopted at **acquisition only**. `existing.session` is never rewritten,
so a claim keeps the identity it was created with through every refresh and release, as
constraint §2.7 requires. Step 3 is what makes that survivable: it lets the owner go on acting
on a natively-keyed claim without the identity scheme still having to produce the same
string.

So a session that enables relay between sessions and meets its own earlier, natively-keyed
claim resolves at step 3 whenever that claim carries proof, and falls to takeover only
when it does not (§6.3). Takeover is the last resort on a mode change, not its ordinary
path, and is recorded in the task's notes as any takeover is.

### 6.5 Registry loss, liveness, and recovery

Liveness reads only the captured claim proof and native process data, and never the
registry: a registry that is gone cannot make an owner look dead, and cannot make a dead
owner look live. Combined with §6.2, a session that acquired under relay and then lost the
registry can still park, close and release everything it holds, and can still be taken over
normally once its process is gone.

When identity does not resolve and ownership proof is unavailable, the §5 error names
`existing.session` verbatim, so the operator can set `TASKS_SESSION` to that string and
act. The explicit pair sits above the relay level, so this recovery works with relay
enabled and on a host where §7 refuses the platform.

### 6.6 Within a session

Identity is resolved independently by each claim-mutating command; there is no cache. The
inputs — ancestry, registry, config — are stable within a session, and a cache would be a
second source of truth for the thing this design exists to make single. The cost is one
`/proc` walk and one JSON read per claim mutation, not per command.

## 7. Platform

Relay ancestry identity is Linux-only in this design: the ancestry walk reads `/proc`,
and no other process-tree source is implemented. `relay = true` on a platform without
`/proc` is a `Config` error at load, naming the platform, rather than a configuration
that resolves every session to "unknown ancestry". Because the config file is
host-local, a machine without a readable process tree simply does not enable it, and
nothing about tasks' behaviour there changes — including its existing
unverifiable/TTL liveness path, which this design does not touch.

The refusal belongs to the relay level, not to program startup or to config parsing.
`TASKS_SESSION` is resolved first, so on a host where the platform is refused the
explicit pair keeps working unchanged and remains the recovery path — the platform
refusal can never take away the escape from it. A caller that is out of scope (§4.2)
likewise never reaches the refusal, since the relay level is not consulted at all.

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
  byte-for-byte; the §7 platform refusal fires below the explicit pair, so a session
  with `TASKS_SESSION` set is unaffected by it.
- The scope test: no harness ancestor with a readable chain, a harness ancestor at
  each depth, an unreadable level before a verdict, and an unmatched inner harness
  that must not be skipped for a matching outer one.
- The match: exact match; wrong pid; wrong start; wrong boot id; wrong host; two
  agents on one handle; a null process handle.
- Harness agreement: a `claude-code` row whose handle satisfies host, pid, start and
  boot for a `codex` ancestor is refused, not adopted — the case that passes every
  other predicate.
- Platform: a schema-valid Darwin handle parses without error and is then **refused**
  as an identity candidate. No test expects a Darwin handle to yield a claim, a pid,
  or a TTL fallback.
- Scoped hints: a `codex` ancestor with a matching `CODEX_SESSION_ID` and an inherited,
  unrelated `CLAUDE_CODE_SESSION_ID` from an outer Claude session resolves successfully;
  a `claude` ancestor with a disagreeing `CLAUDE_CODE_SESSION_ID` fails; an `opencode`
  ancestor with either variable set compares nothing; empty variables are unset; the two
  Codex variables disagreeing with each other is an error.
- Schema: versioned handle fixtures copied from relay-06b1da, including the oversized
  Linux `start` value, labelled as a format test.
- The Codex case: a live `codex` ancestor with an empty registry must produce the
  explicit-identity error, not terminal identity.
- Config: missing file; `relay = false`; `relay = true`; malformed TOML; non-boolean;
  unknown key; neither `XDG_CONFIG_HOME` nor `HOME`.

Continuity, the §6 mechanism, unit where the inputs can be injected:

- **Owner without the registry.** A relay-acquired claim, the registry then removed:
  the owning caller's park, done and edit all succeed via ownership proof.
- **Foreign caller without the registry.** The same claim, a caller whose ancestry does
  not contain the claimed process: refused as foreign, not admitted by proof.
- **Proof mismatch.** The claimed pid exists but with a different `starttime` (pid
  reuse); a different `boot_id`; a different `host`. Each refuses.
- **No proof available.** A claim with `pid: None` (§6.3) falls to identity match or
  takeover, and never to proof.
- **Held error.** Identity fails to resolve on a task that is already claimed by this
  caller: the error is not raised. The same failure with no existing claim: raised.
- **Mode change.** A natively-keyed claim carrying proof is continued by a
  relay-resolved session at step 3, without `--force`; the same claim without proof
  requires `--force` and records the takeover.

End-to-end in `tests/cli.rs`, against the built binary in temp repos with
`XDG_CONFIG_HOME`, `RELAY_STATE_DIR` and a fixture snapshot:

- Acquisition under relay mode writes a claim keyed by the agent id with the handle's
  proof in `pid`, `pid_start` and `boot_id`.
- Park and release with the registry removed after acquisition.
- Takeover of a relay-acquired claim whose process is gone.
- Each §5 error reaches stderr as a `config` error object at exit 1, and the
  already-claimed variant names `existing.session`.
- Concurrent acquisition under relay mode still produces exactly one winner, under the
  existing mutation lock.

## 10. Decomposition

Suggested step children for the implementation plan, in order:

1. The host config file and its typed errors.
2. The relay snapshot reader and schema-1 validation, including Darwin handles as a
   parse-only case.
3. The ancestry walk, shared by the scope test and by ownership proof.
4. The relay level in `identity_from`: match predicate, harness agreement, scoped hints,
   and the §5 errors.
5. Claim-field adoption and the §7 platform refusal.
6. **Continuity**: the acquisition/continuation split, ownership proof, the held-error
   rule, and the `--force` reconciliation across `claim_guard`,
   `refuse_foreign_live_claim`, `park` and `status`. This is the step that changes
   existing call sites rather than adding a new level, and it carries the owner and
   foreign-caller tests of §9.
7. The end-to-end tests, and the skill and README updates.

The plan is written and reviewed before any of it is implemented.
