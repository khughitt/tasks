# Configuration

Type: data model; document types 1.
Project and host TOML formats are currently unversioned. Related references are
the [task model](task-model.md) and [document protocol](documents.md).

Project settings are committed with records. Host settings and the registry are
local to a machine. Unknown project/host keys and malformed present files are
errors; an absent optional host file uses defaults. Never repair an invalid local
project by silently selecting another checkout.

## Project settings

`tasks/.config.toml` belongs to the records directory for all storage kinds.
The accepted top-level keys/tables are exactly those below.

| Key/table | Contract and default |
| --- | --- |
| `[[retention_lineage]]` | Rename-managed, ordered Git-anchored edges; omitted when empty. Fields and validation below. |
| `prefix` | Required `[a-z][a-z0-9]{1,7}` project id; agrees with registration and records. |
| `color` | Optional `#RRGGBB` project color, normalized lowercase in JSON. `check` warns with `color_missing` when unset; it never assigns a color. |
| `profile` | Optional single profile name using lowercase letters, digits and hyphens, with no leading hyphen. Absent means unassigned; tasks validates the grammar, not whether the profile exists. |
| `shorthand` | Optional list of private aliases, each nonempty single-line text; defaults to empty. Uniqueness across projects is checked by consumers when routing, not by `tasks check`. |
| `[identity]` | Optional snapshot of public identity, written only by tasks; fields described below. |
| `spec_dirs` | Optional list; defaults to `docs/specs`, `docs/designs`, `docs/superpowers/specs`, `docs/superpowers/designs`. |
| `plan_dirs` | Optional list; defaults to `docs/plans`, `docs/superpowers/plans`. |
| `[tags]` | Optional label-to-single-line-meaning dictionary; unknown labels warn, vocabulary is advisory. |
| `[needs.<name>]` | Required nonblank single-line `meaning`, optional `exclusive = false`; unknown keys fail. Names use lowercase letters/digits/hyphens and no leading hyphen. |
| `[feedback]` | Optional nonblank single-line `scope` declaring the shared tooling the project owns for feedback routing; every registered, reachable, unretired project accepts `tasks feedback` by default. |
| `[attachments]` | Limits and multipart validator declarations described below. |
| `[growth]` | Optional positive `warn_bytes`; default min(65,536, floor(max_bytes/4)), at least 1. An explicit larger threshold is accepted with a warning, not clamped. |
| `[checkout]` | Required `path` when present; relative to records or absolute. Missing `kind` means linked; the other accepted kind is `detached`. Managed by init/relocation. |
| `[retired]` | Required `date` (real `YYYY-MM-DD` from year 1) and `reason` (nonblank single line without control characters); optional `successor` (well-formed prefix). Lifecycle metadata managed by retire/migrate. |
| `[predecessor]` | One retired generation: required `prefix`, `successor_alias` (the successor's former prefix), and a frozen `suffixes` array of six lowercase hex digits. Created by a two-step prefix handoff; reserves those suffixes for new task IDs and lets literal reclaimed-prefix IDs route to that generation. Unknown fields and invalid/self prefixes or suffixes refuse. |
| `[git]` | Only `commit`, boolean default false; opt-in scoped automatic record commits. |

`[identity]` requires `name`, `purpose`, `source` and `refreshed` strings, with an
optional `aliases` list defaulting to empty. Name, purpose and each alias are
nonempty single-line text. `source = "head"` requires `commit` and `blob`, recording
the root repository's HEAD commit and identity blob at refresh; `source = "seeded"`
accepts neither. `refreshed` records the refresh time in UTC (RFC 3339). Unknown
identity fields or sources are config errors. Never hand-edit the snapshot.

A retention lineage edge has exactly `version` (integer 1), `from_prefix`,
`to_prefix`, `layout`, `from_subdir`, `to_subdir`, `commit` and `retained_ref`
(strings). Subdirectories are normalized relative to the same records Git root
(`.` names its root); commit is a full lowercase 40-hex commit and retained_ref
is a named `refs/heads/` branch reaching it. Edges preserve layout, chain without
cycles or conflicting endpoints, and end at the live pairing. Each source config
at its cutover commit must contain exactly its preceding edges. Retention reads,
rename and check reject missing or inconsistent proof; an alias alone is not proof.
Rename writes these fields; do not hand-edit them to repair missing history.

`spec_dirs`/`plan_dirs` define allowed working-document roots, not a public
reference index. Root lists must be nonempty and contain normalized relative
directories, with no empty, `.` or `..` components or colon in the first segment.
One trailing slash is stripped. Omitted lists use defaults; empty lists are errors.
See [documents](documents.md).

Attachment limits are byte counts. `max_bytes` is positive, default 2,097,152
(2 MiB). `multipart_max_bytes` defaults to 16,777,216 (16 MiB), with maximum
268,435,456 (256 MiB). `multipart_max_parts` defaults to 16, maximum 256.
`multipart_validators` is optional: absence refuses multipart publication; `[]`
explicitly declares no extra project validators. Entries have `match` and
`command` (an argv array). See [attachments](attachments.md#multipart-validators)
for matching, execution, cap and failure contracts.

Record-growth warnings report active record size, archived notes, attachments and
logical history, and suggest shortening the body only when `body_bytes` is at
least half of `record_bytes`; they do not authorize truncation or raise
attachment caps. `archive-notes` is a separate preview/apply protocol.

## Registry and locations

The registry is `$XDG_CONFIG_HOME/tasks/projects.toml`, otherwise
`$HOME/.config/tasks/projects.toml`. No usable home/config root is an error.
An absent registry is empty; an unreadable/malformed one fails. Registry roots
are host paths, with leading `~/` expansion supported.

| Table | Stored value |
| --- | --- |
| `[projects]` | Live prefix to registered main checkout path. |
| `[records]` | Detached prefix to its actual records directory, already including the project directory. |
| `[aliases]` | Retired prefix to canonical live prefix, flattened to one hop. |
| `[predecessors.<live>]` | Host copy of the live project's `[predecessor]`, populated by init and rename/adoption. The displaced-ID index is derived only from these tables, without scanning project directories. Each predecessor is distinct and cannot itself be an alias. |
| `[groups]` | Group name to sorted unique live-prefix array. |
| `[locations.<prefix>]` | Optional current `.worktrees` `storage` path and `former` array. Each former entry records `root`, optional `storage` and UTC `until` date. |

Group names use the tag grammar and cannot equal live/retired prefixes or an
unfinished rename's reservation. Members must be live registered prefixes.
Aliases and location history support old ids/paths; they do not make stale
checkouts writable. Commands normalize paths and validate collisions. Use
`init`, `group set`, `unregister`, `rename`, `retire` and `resolve` instead of
constructing migration state by hand. [Project operations](project-operations.md)
explains recovery, registered-main versus worktree routing, and location history.

Claims, mutation locks, proposal state, rename/move/migration journals and
provider caches are implementation state with command-owned formats. They are
not additional user settings. Do not synchronize or edit a claim to acquire work.

## Host settings and document providers

The optional host file is `$XDG_CONFIG_HOME/tasks/config.toml`, otherwise
`$HOME/.config/tasks/config.toml`. XDG must be nonempty and absolute; empty,
unset or relative XDG uses absolute HOME. If neither base is usable, configuration
resolution fails. The file accepts only the following sections and keys:

```toml
[identity]
relay = false

[docs.schemes]
# scheme = "document-provider-command"

[output]
headers = false
```

`identity.relay` defaults false. Enabling it changes claim identity resolution
to consult relay after an explicit session override; unreadable relay ancestry
can refuse a mutation. It does not enable arbitrary note provenance strings.
See [ownership](ownership.md#identity-and-liveness).

Scheme names match `[a-z][a-z0-9-]*`; `files` is reserved. Commands must be nonblank
and NUL-free. [Documents](documents.md#external-providers) specifies the literal
argument, closed stdin, timeout, envelope validation and no-fetch behavior. No host config file
means no external providers. A configured command is executable host policy,
not a command copied automatically from a task record.

`output.headers` defaults false and controls pretty table headers. Explicit
`--headers` or `--no-headers` overrides valid `TASKS_HEADERS=true|false`, which
overrides the file. The opposing global flags conflict, including across a
subcommand. JSON and non-table output gain no header decoration.

Ordinary invocations read and validate the complete host file once, sharing that
observation across presentation, identity and document providers before command
side effects. Invalid config or environment is an error even under a winning flag
or JSON output. Unknown keys, wrong types, duplicate definitions and invalid
TOML/UTF-8 fail; strings are not coerced to booleans. Only a missing optional file
or parent supplies defaults. A directory, unreadable file or dangling file symlink
fails; usable symlinks are allowed. Help, version, completion and the local-only
`attachments` eligibility query keep their host-config-free behavior.

Upgrade installed readers before writing new keys: older strict readers reject
them. Existing project settings and the registry remain separate contracts. The
cross-tool convention is in ops's `docs/user-preferences.md`.

## Environment and precedence

| Variable | Meaning |
| --- | --- |
| `TASKS_FORMAT` | `json` (default) or `pretty`; explicit format flag overrides a valid value. Invalid set values still fail. `--json` and `--pretty` conflict. |
| `TASKS_HEADERS` | Exactly `true` or `false`; explicit `--headers`/`--no-headers` overrides a valid value, then `output.headers`, then false. Empty, non-UTF-8 and other values fail on ordinary invocations even under a flag. |
| `TASKS_OWNER` | Advisory owner override, then git branch, then USER. Chosen value must pass owner grammar. |
| `TASKS_SESSION`, `TASKS_SESSION_PID` | Explicit claim identity and optional process identity; see ownership before using a PID across namespaces. |
| `CLAUDE_CODE_SESSION_ID`, `CLAUDE_PID`, `CODEX_THREAD_ID`, `CODEX_SESSION_ID` | Native claim/provenance inputs; thread is the Codex worker identity and differing session the root. |
| `TASKS_AGENT`, `TASKS_MODEL` | Opaque filing and completion stamps. Explicit `add --agent` overrides its environment value. |
| `TASKS_MAX_COMPLEXITY` | Picker envelope `low/mid/high`; higher/unassessed tasks hidden. Supported flags override a valid value, not its validation. |
| `TASKS_WITHOUT` | Comma-separated unavailable needs; supported `--without` flags add to it. Environment names absent from scope are ignored. |
| `TASKS_COMPLETE` | Shell completion activation; unset/`0` disables it. |
| `TASKS_COLOR` | `auto`, `always`, `never`; default off. |
| `NO_COLOR` | Nonempty value suppresses environment-selected color. |
| `TASKS_THEME` | `terminal` (default) or `default`; chooses terminal colors or shipped palette. |
| `TASKS_PALETTE` | Explicit `fg=#rrggbb bg=#rrggbb cyan=#rrggbb [magenta=#rrggbb]`, overriding palette discovery. |
| `COLUMNS` | Pretty layout width when available; show wraps up to 100 columns on a terminal. |

Color precedence is explicit `--color`, then nonempty NO_COLOR, then TASKS_COLOR,
then off. Flags and TASKS_COLOR are validated before precedence; an invalid
environment setting is not hidden by a valid flag. Auto needs a terminal and
only styles pretty output. JSON never contains ANSI escapes. Project color is
used for prefixes and `show`; task ids use its fixed lighter variant. In multi-task
views, activity-aware titles use a shared OKLab lightness range while keeping the
project hue; chroma decreases where necessary to fit sRGB. On the builtin dark
palette, title lightness is `0.60 + 0.30a`, where `a` is activity from 0 to 1.
Activity combines meaningful lifecycle and work events with bounded attention from
immediate recorded neighbors; it does not change priority or picker order. Direct
activity has a five-day half-life, with daily burst damping and bookkeeping notes
excluded. The scale points toward whichever of black or white contrasts more with
the theme background (white on a tie), so higher activity normally means lighter
titles on dark backgrounds and darker titles on light backgrounds. Its cold end
is adjusted when needed to preserve 3.5:1 contrast after RGB quantization; the hot
end extends 0.30 lightness toward that ink, bounded by black/white. Unusual
backgrounds may narrow the available range. IDs retain their fixed project shade;
titles can be lighter than IDs and need not equal the original project color.
Unassigned project
titles stay plain, and activity is absent from JSON. Date recency fades over two
years; priorities use bold/color scales where available. Missing terminal colors
produce a warning and plain fallback; a palette without magenta disables only
priority colors. These presentation choices do not alter task values.

`HOME`, `XDG_CONFIG_HOME`, `XDG_STATE_HOME` and related platform directories select
host files/state; configure them consistently for an isolated automation run.
Editor, clipboard and provider executables must be available on PATH when used.
Variables named for internal test stop points are not configuration interfaces.

## Validation and rollout

Use `tasks check` after configuration changes. A read may report undeclared task
tags or unreachable foreign dependencies as warnings, while malformed settings
and unsafe storage fail. Configuration changes that affect synced records need
compatible readers on every host before new writers run. Moving storage or
changing a prefix needs the explicit journal/recovery workflow; editing registry
text is not a substitute for reconciling record and checkout identities.

The predecessor configuration is synced with records; its registry copy is host-local.
For example, `[predecessor]` with `prefix = "forge1"`,
`successor_alias = "forge2"` and `suffixes = ["0e720e"]` in the current `forge` project is paired with
`[predecessors.forge]` carrying the same values in `projects.toml` and alias
`forge2 = "forge"`. Both prefixes must be valid and distinct from each other and
the live prefix; all fields are required and unknown fields refuse. Init imports
the metadata and alias together, in either generation's registration order.
An identical existing alias is accepted. A conflicting alias, live project,
group or predecessor owner refuses before config or registry changes.

A metadata or alias mismatch refuses normal opens and is reported by `check`.
Re-pair through `init --prefix forge --force` in the checkout; for detached
records include `--records <records-root>`. A missing alias is repaired only when
its name is free or already targets this project; a conflicting owner is never
overwritten.
An absent predecessor registration does not remove the reservation or redirect an
old ID into the current project. See the
[two-step handoff and its current limits](project-operations.md#reclaiming-a-retired-predecessors-prefix).

### Terminal project-colour stand-in

`resolve` rows (including their `entry`) and `projects` rows expose `color_256`
next to `color`: an integer xterm-256 index, omitted when that row has no color.
Consumers use this value instead of deriving their own terminal approximation.
Tasks selects the nearest entry in OKLab from indices 16–255 that have at least
3:1 contrast against both `#1e1e1e` and white; equal distances choose the lower
index. This is a fixed fallback palette, independent of the caller's terminal.
The authored RGB color and existing pretty output are unchanged.
