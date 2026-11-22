# Session policy hooks

Type: protocol; document types 1.

`tasks hook claim-guard` and `tasks hook halt-notice` consume one strict UTF-8
relay schema-2 SubscriberEnvelope on stdin. Python and Node are replay tools;
the production commands are part of the Rust binary. Native reply encoding
belongs to relay; the retained agy bridge belongs to hq.

## Replies and entry

Claim guard writes exactly `{"decision":"allow"}` or
`{"decision":"deny","reason":"..."}` and exits zero for policy decisions.
Halt notice writes exactly `{}` or `{"context":"..."}` and exits zero.
Reasons and context fit 10,000 UTF-16 code units, including the owner line.
An oversized claim inventory gets a short explicit deny directing the agent to
`tasks claims --all-projects`; oversized notice context fails explicitly.

Both leaves enter before completion, ordinary option validation, HostConfig,
display setup or directory handling. Global `--json`, `--pretty`, `--color`,
`--headers`, `--no-headers` and `-C` are accepted and ignored, including conflicting,
repeated or unusable values. Option values retain their argument slots.
Structural usage errors, help and version keep ordinary CLI semantics.
Display environment and config never alter a hook reply. Only `event.cwd`
selects notice lookup; neither process cwd nor `-C` supplies it.

## Claim policy

The guard reads every registered prefix's claim store directly and never prunes
or changes ownership. Stale claims do not count; unverifiable claims stay protected.
Retry and agy's own-termination exemptions precede identity, registry, claims and
capability reads. Missing evidence cannot earn an exemption.

Codex identity with a `root` member selects a worker on both Stop and SubagentStop.
Workers check their own session claims and never read controller wake facts.
SubagentStop requires a distinct, usable root. Root controllers also report related
worker claims with evidence-based recovery guidance. No own or related claims allows
without a capability read.

A claim-holding controller reads the installed hq harness-facts list command with
a ten-second timeout. Only probed wake capability and the required kind/status
sets grant a child exception. Claude validates the full background snapshot;
invalid private Facts leave child/wake state unknown. Crons alone are not children.
agy busy input requires coverage of both shell and subagent kinds. Other held
claims receive continue, park, done and bounded-wait guidance. A deny grants no
authority to release another session's claim or bypass a user decision.

## Halt authority

The notice resolves an existing absolute event directory within its Git boundary,
then scans only the registered records authority. In-tree, detached and linked
projects and their worktrees use that authority. A historical pre-detach config
must match one verified registered detached pairing; its local records are history.
There is no unregistered/local fallback, admission traversal or profile resolution.
Without local config, verified registered code-tree locations (including nested
projects) still select their authority; ambiguous location/link matches are uncertainty.

No established project or no open halt returns `{}`. Invalid cwd, local config,
registry or pairing produces a one-line cannot-tell-which-project context with
`tasks where` recovery. A known-authority scan failure is an error.
Among tasks tagged halt in idea, todo, doing, blocked or shelved status, the notice
selects oldest created time, then task id. Its start/override text grants no override
permission; the user's instruction still controls that decision.
Title controls and Unicode line separators are visibly escaped before the wire bound
is checked; ordinary Unicode title text is preserved.

## Failure

Input is one JSON object, at most 4 MiB, with no duplicate keys at any nesting level,
including unknown fields. Native input and older envelope schemas are rejected.
Guard input/store/facts failures deny where a reply can be written. Notice input
and known-authority scan failures use nonzero exit, bounded stderr and no context.
Diagnostics never dump input, environment or subprocess output. The facts reader
owns, kills and reaps its process group, including pipe-holding descendants.

## Replay and route status

Run `just test-one --test hooks hooks_replay_smallest`, then
`just test-one --test hooks hooks_replay_corpus`. Python 3 and Node 22 or newer
are required; missing tools fail visibly. No private source checkout or network
fetch is needed. The source manifest under tests/fixtures/hooks records exact
repository commits, original paths and hashes; snapshots are immutable test data.
The decoder calls frozen relay normalization/projection, and the native policy
oracle is frozen ops claim-guard (including agy) or only its halt_notice function.
The source-test coverage map keeps command-boundary and retired dependency cases
separate from successful producer replay.

Replay distinguishes exact reason/decision equivalence from approved corrections:
ownership/encoding/overflow; private-Fact uncertainty and strict input; agy safe
integer normalization (including 1.0 retry and above-safe-range refusal); Codex
worker Stop; displaced hq policy; and registered halt authority/uncertainty.
Every differing decision remains visible in the paired rows.
Both policies use the current Tasks binary for CLI lookups. At a registered
in-tree root, `where` can resolve a worktree with no local configuration, so both
routes emit the halt notice. The nested-project cases still distinguish the
routes.

The frozen relay projection is fixture-tested. It is not a live-route qualification.
The adapted agy bridge remains pending under hq-a83688; native route qualification
and activation belong to relay-d487c1 and the relevant hq owner. Retain old hooks
until those owners prove their routes. These commands do not register, activate,
install or remove hooks. See [work ownership](ownership.md).
