# Configuration

Type: data model; document types 1.

Project settings are committed with records. Host settings and the registry are
local to a machine. Unknown project/host keys and malformed present files are
errors; an absent optional host file uses defaults. Never repair an invalid local
project by silently selecting another checkout.

## Project settings

`tasks/.config.toml` belongs to the records directory for all storage kinds.
The accepted top-level keys/tables are exactly those below.

| Key/table | Contract and default |
| --- | --- |
| `prefix` | Required `[a-z][a-z0-9]{1,7}` project id; agrees with registration and records. |
| `color` | Optional `#RRGGBB` project color, normalized lowercase in JSON. |
| `spec_dirs` | Optional list; defaults to `docs/specs`, `docs/designs`, `docs/superpowers/specs`, `docs/superpowers/designs`. |
| `plan_dirs` | Optional list; defaults to `docs/plans`, `docs/superpowers/plans`. |
| `[tags]` | Optional label-to-single-line-meaning dictionary; unknown labels warn, vocabulary is advisory. |
| `[needs.<name>]` | Required nonblank single-line `meaning`, optional `exclusive = false`; unknown keys fail. Names use lowercase letters/digits/hyphens and no leading hyphen. |
| `[feedback]` | Optional nonblank single-line `scope` for feedback reporting. |
| `[attachments]` | Limits and multipart validator declarations described below. |
| `[growth]` | Optional positive `warn_bytes`; default min(65,536, floor(max_bytes/4)), at least 1. An explicit larger threshold is accepted with a warning, not clamped. |
| `[checkout]` | Required `path` when present; relative to records or absolute. Missing `kind` means linked; the other accepted kind is `detached`. Managed by init/relocation. |
| `[retired]` | Required `date`, optional `successor` and `reason`; lifecycle metadata managed by retire/migrate. |
| `[git]` | Only `commit`, boolean default false; opt-in scoped automatic record commits. |

`spec_dirs`/`plan_dirs` define allowed working-document roots, not a public
reference index. Paths are checked by the document resolver; explicit empty roots
are distinct from omitted defaults. See [documents](documents.md).

Attachment limits are byte counts. `max_bytes` is positive, default 2,097,152
(2 MiB). `multipart_max_bytes` defaults to 16,777,216 (16 MiB), with maximum
268,435,456 (256 MiB). `multipart_max_parts` defaults to 16, maximum 256.
`multipart_validators` is optional: absence refuses multipart publication; `[]`
explicitly declares no extra project validators. Entries have `match` and
`command` (an argv array). See [attachments](attachments.md#multipart-validators)
for matching, execution, cap and failure contracts.

Record-growth warnings report active record size, archived notes, attachments and
logical history; they do not authorize truncation or raise attachment caps.
`archive-notes` is a separate preview/apply protocol.

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
| `[groups]` | Group name to sorted unique live-prefix array. |
| `[locations.<prefix>]` | Optional current `.worktrees` `storage` path and `former` array. Each former entry records `root` and optional `storage`. |

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
`$HOME/.config/tasks/config.toml`. It accepts only:

```toml
[identity]
relay = false

[docs.schemes]
# scheme = "command receiving the protocol request on stdin"
```

`identity.relay` defaults false. Enabling it changes claim identity resolution
to consult relay after an explicit session override; unreadable relay ancestry
can refuse a mutation. It does not enable arbitrary note provenance strings.
See [ownership](ownership.md#identity-and-liveness).

Scheme names match `[a-z][a-z0-9-]*`; `files` is reserved. Commands must be nonblank
and NUL-free. [Documents](documents.md#external-providers) specifies the request,
cache, timeout, envelope validation and no-fetch behavior. No host config file
means no external providers. A configured command is executable host policy,
not a command copied automatically from a task record.

## Environment and precedence

| Variable | Meaning |
| --- | --- |
| `TASKS_FORMAT` | `json` (default) or `pretty`; explicit format flag overrides a valid value. Invalid set values still fail. `--json` and `--pretty` conflict. |
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
used for prefixes; task ids use its lighter variant. Recency fades over two
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
