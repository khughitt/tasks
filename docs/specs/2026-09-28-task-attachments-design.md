# Task attachments

Status: draft 2026-09-28, revised after review (collision ownership, storage symlinks,
rename baseline, detach ordering); awaiting user review
Task: tasks-a2d903 (goal tasks-ce2f58, idea tasks-e7a870)
Brief: docs/notes/2026-09-25-task-attachments-brief.md

## Goal

A task can carry a screenshot or another file that travels with the record across
checkouts, worktrees, and hosts. An agent reading the task gets a path it can open.
Success means `tasks attach` stores a file beside the record, `show` (and `next`)
print its path, `check` reports attachment drift, and `rename` moves attachments with
their records. Older binaries keep reading every record unchanged.

## Decisions

| Question (brief) | Decision |
|---|---|
| Discovered directory or declared field | Directory found by convention, `tasks/files/<id>/`. No front-matter change. |
| Reference a file already in the repository | No. An attachment is always a copy under the task's directory. Repository files are linked from notes or `--source` as today. |
| Size policy | A per-file cap, default 2 MiB, set per project. Files are stored byte for byte: no recompression, no Git LFS. |
| Public repositories and feedback | `feedback` takes no attachment option. `attach` makes no visibility check; nothing records visibility locally. |
| Input sources | A file path, stdin (`-`), and `--clipboard` through `wl-paste`. |
| Viewing | `show` prints absolute paths, as it does for `spec_path`. Inline rendering belongs to a terminal front end, later. |
| Which record owns a file | The record's own `attached:` and `detached:` notes, the ledger. `check` compares the ledger with the directory. |

The declared `attachments:` list was rejected because `Task` is
`deny_unknown_fields`, so every older binary would refuse the record. It also gives
the file no home that moves with the record.

## Layout

    tasks/files/<id>/<name>

The directory belongs to the task whose id names it, and it holds regular files only.
A name is one path component made of `[A-Za-z0-9._-]`, at most 128 bytes, not
starting with `.`. It contains no space or colon, so it parses unambiguously out of a
note. The directory is created on the first attach. The scanners
(`Project::task_files`, rename's `task_paths`) read only `tasks/*.md`, so current and
older binaries both ignore `tasks/files/`.

### Ownership ledger

The directory's name says where a file lives, but after a merge it cannot say which
record put it there: an id collision merges two records' directories into one
(see **Id-collision recovery**). Ownership therefore lives in the record, in two note
forms:

    attached: <name> (<n> bytes)
    attached: <name> (<n> bytes): <caption>
    detached: <name>: <why>

A name is *live* in a record when its most recent ledger note is `attached:`. Notes
carry the name only, never the id or a path, so `rename` and collision recovery leave
them valid. Ledger notes are plain notes: no provenance stamp, no lifecycle marker. A
user note with the same text is read as a ledger entry, as with lifecycle markers.

### Storage safety

`attach` and `detach` write and delete under `tasks/files/<id>/`, so both storage
directories must be real directories inside the project:

- `tasks/files` and `tasks/files/<id>` are inspected with `symlink_metadata`, never
  followed. Each must be absent or a directory. A symlink or any other file type fails
  `attachment_unsafe`, before any read of the source.
- Missing directories are created with `create_dir`, not `create_dir_all`. It fails on
  an existing symlink rather than writing through it.
- An attachment is removed only after `symlink_metadata` shows it is a
  regular file. The temporary file is opened with `create_new`.
- Everything runs under the project's mutation lock, which serializes `tasks` writers.
  A concurrent non-`tasks` process swapping a directory for a symlink between the check
  and the write is out of scope: whoever can write `tasks/` can already write anywhere
  that user can.

`tasks/` itself is the project's own directory and is not checked.

## Configuration

    [attachments]
    max_bytes = 2097152

`max_bytes` is optional, and so is the table. The default is 2 MiB (2097152). The
value must be a positive integer, or `open` fails with `Error::Config`. The new
`AttachmentsConfig` struct is `deny_unknown_fields`, like `[feedback]`. Rename's
config rewrite goes through `toml::Value`, so the table survives a rename.

## Commands

### attach

    tasks attach <id> <path | -> [--name <name>] [--caption <text>]
    tasks attach <id> --clipboard [--name <name>] [--caption <text>]

- **Routing and locking:** routes by the id's prefix and holds the mutation lock
  (`open_id_write_ctx`), like `note`. Any status is accepted, including closed tasks:
  evidence often arrives after the fact.
- **Name:** from a path, the basename unless `--name` is given. From stdin,
  `--name` is required, or the command fails `invalid_attachment_name`. From the
  clipboard, the default is `clipboard-<UTC yyyymmddThhmmssZ>.<ext>`, with the
  extension taken from the chosen MIME type. An invalid name fails
  `invalid_attachment_name`. Nothing is ever overwritten.
- **Size:** at most `max_bytes + 1` bytes are read from any source. A larger file fails
  `attachment_too_large` with the size (or "more than") and the cap, before anything is
  written.
- **Clipboard:** runs `wl-paste --list-types`, then picks the first of `image/png`,
  `image/jpeg`, `image/webp`, `image/gif` on offer and reads it with
  `wl-paste --type <mime>`. Two failures:
  - `wl-paste` missing or exiting non-zero fails `clipboard_unavailable`, with its
    stderr;
  - no image type on offer fails `clipboard_no_image`, listing the offered types.
- **Write order:** the file is written before the ledger, so a failure never leaves a
  ledger entry with no file.
  1. Check storage safety and read the source into memory, up to the cap.
  2. If `<name>` exists and is live in the ledger, fail `attachment_exists`. If it
     exists, is not live, and holds identical bytes, skip to step 4: an earlier attach
     was interrupted, and this run finishes it. If it exists with different bytes, fail
     `attachment_exists`; the detail names the unrecorded file.
  3. Write the file atomically: a `create_new` temp file `.<name>.tmp-<pid>` in the
     same directory, fsync, then rename.
  4. Append the `attached:` ledger note and `save`.

- **Failures:** if the save fails, the file this run wrote is removed and the error
  returned; a file written by an earlier run stays, since this run cannot tell it
  apart. A crash between steps 3 and 4 leaves a file with no ledger entry.
  `check` reports it as `attachment_unnoted`, and rerunning the same attach finishes
  it.
- **Output:** `{"id", "file": {"name", "path", "bytes"}, "warnings"}`, where `path` is
  absolute. `--pretty` prints the path.

### detach

    tasks detach <id> <name> "<why>"

Removes one attachment and records why. The reason is required, as for `drop`. This
is the one supported way to correct a wrong attach without editing `tasks/` by hand.
It also clears an unrecorded file left by an interrupted attach.

The ledger is written before the file is deleted, so no failure destroys a file that
the ledger still calls live:

1. Check storage safety. If `<name>` is neither present as a file nor live in the
   ledger, fail `attachment_missing`. A present entry that is not a regular file fails
   `attachment_unsafe`.
2. Unless the ledger already records `<name>` as detached, append
   `detached: <name>: <why>` and `save`. If the save fails, return the error; nothing
   has been deleted.
3. Remove the file if it is present. If the directory is now empty, remove it too.

Recovery:

- **Removal fails or is interrupted after step 2:** the file stays, and the ledger
  says detached. `check` reports `attachment_unnoted`. Rerunning the same detach skips
  step 2 and finishes the removal, with no second note.
- **Uncommitted files:** removal is permanent for a file that was never committed.
  Git history keeps committed bytes. The help text and `--pretty` output say this.

### show and next

`ShowFields` gains `files: Vec<FileInfo>`, which `next` shares. It is omitted when
empty, per the output contract. `FileInfo` is `{name, path, bytes}`, sorted by name,
with `path` absolute under the resolved project root. A cross-project `show` therefore
prints the other project's path, which is where the file is. `--pretty` adds a
`# files` section listing each path with a human size.

`show` lists the valid regular files present on disk. For this task, it adds a
warning for each finding that `check` would report: an unsafe directory, an invalid
entry, an unnoted file, or a missing file. An unsafe directory lists nothing and is
never read through.

### check

A new pass reads `tasks/files/` directly:

| Kind | Level | Condition |
|---|---|---|
| `attachment_unsafe` | error | `tasks/files` or `tasks/files/<id>` is a symlink or not a directory. The pass does not descend into it. |
| `attachment_orphan` | error | `tasks/files/<x>` where `<x>` is not a valid id or `tasks/<x>.md` does not exist. |
| `attachment_unnoted` | error | A valid file in a task's directory that is not live in that record's ledger. Causes: an interrupted attach or detach, or files merged into the wrong directory by an id collision. |
| `attachment_missing` | error | A name live in a record's ledger with no file in its directory. Causes: a file removed by hand, or a record renamed without its files. |
| `attachment_invalid` | warning | A task's directory holds a subdirectory, a symlink, a leftover temp file, or a file with an invalid name. |
| `attachment_too_large` | warning | A file is above `max_bytes`, because it was added by hand or the cap was lowered later. |

A stray regular file directly under `tasks/files/` is `attachment_orphan`. The pass
runs over every task, open or closed, and compares every record's ledger with its
directory, including records that have no directory. The four drift kinds are errors,
like `doc_missing`, so the pre-commit gate stops a commit that separates a file from
its owner.

### feedback

No change. `feedback` takes no attachment option: a screenshot is project content,
and a report lands in another project's checkout, which may be public.

## Rename

Each task's directory moves with its record. The directory's name carries the
prefix; the files inside need no rewrite, and ledger notes name no id.

Observing the directories alone cannot tell a completed move from a destination
orphan that was already there, or a task that never had files from one whose files
disappeared. The inventory therefore records a baseline:

- **Baseline:** `InventoryEntry` gains a required `attachments: bool`, which is
  whether `tasks/files/<src-id>` existed when the inventory was built. `build` refuses
  an unsafe source directory (see **Storage safety**).
- **Old inventories:** the field has no serde default. An inventory written by an
  older binary fails to load, with a typed error naming the inventory file. That
  binary never moves directories, so finishing with it and then renaming the
  directories by hand is the documented recovery. Renames are rare and host-local, so
  guessing the baseline would be the silent fallback this project refuses.
- **Observation:** `snapshot::observe` records each entry's directory state,
  `(source, destination)`, each present or absent, read with `symlink_metadata`.
  `classify::refusal` adds three codes:

  | Baseline | Source dir | Dest dir | Verdict |
  |---|---|---|---|
  | `true` | present | absent | move pending |
  | `true` | absent | present | moved |
  | `true` | present | present | R5: unexpected destination attachments |
  | `true` | absent | absent | R6: attachments missing from both sides |
  | `false` | absent | absent | nothing to move |
  | `false` | any | present | R5: unexpected destination attachments |
  | `false` | present | absent | R7: source attachments appeared after the inventory |

  A fresh rename runs the same table before its first write, so a destination orphan
  that was already there refuses as R5 up front.
- **Move order, per entry:** write and verify the destination `.md`, then move
  `tasks/files/<src-id>` to `tasks/files/<dst-id>` with `std::fs::rename` (same
  filesystem, atomic), then remove the source `.md`. The loop's existing
  "source already gone" branch (`rename/mod.rs`, the `NotFound` arm) also finishes a
  pending directory move. A crash between the two steps therefore resumes correctly.
  `files_done` requires every entry's directory verdict to be "moved" or
  "nothing to move".
- **Strays:** a directory under `tasks/files/` whose id is not in the inventory is a
  stray, as an uninventoried `.md` is now.
- **Uncommitted attachments:** these already block a fresh rename, because
  `uncommitted_task_files` runs `git status` over all of `tasks/`.
- **Adopt:** `rename --adopt` writes no synced files, so it needs no change. The
  directories arrive with the synced checkout.

## Id-collision recovery

Two branches that each created `tasks/<id>.md` also each write
`tasks/files/<id>/`. Git merges the two directories into one. Files with different
names combine silently; a name both sides added is an add/add conflict. The
directory cannot say which record owns what, but each side's ledger can. Moving the
whole directory with the losing record would take the winner's files.

`skills/tasks/SKILL.md` and the README extend the existing procedure. The first step
is unchanged: keep the winning record as `<id>`, and write the losing record to a
fresh `<new>` with its `id` fixed. Then:

1. **Move the loser's files.** For each name live in the loser's ledger and not in the
   winner's, move `tasks/files/<id>/<name>` to `tasks/files/<new>/<name>`.
2. **Split names both ledgers hold.** Resolve the file's add/add conflict by side,
   using the side the loser's record came from:
   - `git show :2:tasks/files/<id>/<name>` is the `HEAD` side, and `:3:` the merged
     side.
   - The winner's side stays at `tasks/files/<id>/<name>`, and the loser's side is
     written to `tasks/files/<new>/<name>`.
   - Identical bytes produce no conflict; copy the file to both directories.
3. **Verify.** Run `tasks check`. `attachment_unnoted` and `attachment_missing` must
   be clear for both ids.

`check` detects a skipped or misapplied step: a loser's file left under `<id>` is
`attachment_unnoted` against the winner and `attachment_missing` against `<new>`.

## Errors

These are new `Error` variants, with the codes that `kind()` returns:

- `invalid_attachment_name`
- `attachment_exists`
- `attachment_missing`
- `attachment_unsafe`
- `attachment_too_large`
- `clipboard_unavailable`
- `clipboard_no_image`

## JSON contract changes

- `show` and `next` gain an optional `files` array.
- `attach` and `detach` are new outputs.
- `check` gains six finding kinds.
- The rename inventory gains a required per-entry `attachments` field. It is
  host-local state, not output.

None of these changes an existing field.

## Testing

End-to-end tests in `tests/cli.rs`:

- **attach:**
  - from a path and from stdin;
  - `--name` validation;
  - `attachment_exists` for a live name, and for an unrecorded name with different
    bytes;
  - finishing an interrupted attach: an unrecorded file with identical bytes gets only
    the note;
  - the cap at `max_bytes` and at `max_bytes + 1`, with a small configured cap;
  - the ledger note text;
  - cross-project routing by prefix;
  - a failing save: make `tasks/` read-only after `tasks/files/<id>` exists; the new
    file is removed, and a pre-existing file is kept.
- **Storage safety:** `tasks/files` and `tasks/files/<id>` each as a symlink to a
  directory outside the project.
  - `attach` fails `attachment_unsafe` and nothing is written outside.
  - `detach` fails `attachment_unsafe` and nothing is deleted outside.
  - `show` lists nothing and warns.
  - `check` reports `attachment_unsafe`.
  - An attachment that is itself a symlink: `detach` refuses it.
- **detach:**
  - the ledger note, and removal of the empty directory;
  - a failing save leaves the file in place;
  - finishing an interrupted detach: a `detached:` ledger with the file still present
    removes it with no second note;
  - clearing an unrecorded file.

  Interrupted states are built with `tasks note` (ledger text) and by writing files
  directly, not by crash injection.
- **show:** the `files` field, the pretty section, and a warning per finding.
- **check:** all six finding kinds, and the collision case: two ledgers and one merged
  directory, before and after the recovery procedure.
- **rename:**
  - a record with attachments and one without;
  - a crash-injected resume between the directory move and the source removal
    (`TASKS_RENAME_STOP_AFTER`);
  - each of R5, R6 and R7, including R5 on a fresh rename with a destination orphan;
  - an inventory missing the `attachments` field failing with the typed error.

The clipboard path gets a unit test of MIME selection and extension mapping, plus an
end-to-end test with a stub `wl-paste` on `PATH`.

## Out of scope

- Recompression.
- Git LFS.
- A visibility marker.
- Inline image rendering; tasks-tui can build it on `files[].path`.
- Attachments on `feedback`.
- Listing attachments in `list` or `prime`.

## Decomposition (children of tasks-ce2f58)

1. **Storage, config, and `attach`/`detach`:** the storage-safety checks, the ledger
   parser, clipboard input, and the errors.
2. **`show`/`next` `files` field and the `check` pass.**
3. **`rename`:** the inventory baseline, the directory observation, R5–R7, the move
   order, and the resume test.
4. **README and skill:** the command reference, collision recovery with ledger-based
   splitting, and the size policy.

Pieces 2 and 3 depend on 1; piece 4 comes last.
