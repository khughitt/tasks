# Task attachments

Status: draft 2026-09-28, awaiting review
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

The declared `attachments:` list was rejected because `Task` is
`deny_unknown_fields`, so every older binary would refuse the record. It also gives
the file no home that moves with the record.

## Layout

    tasks/files/<id>/<name>

The directory belongs to the task whose id names it, and it holds regular files only.
A name is one path component made of `[A-Za-z0-9._-]`, at most 128 bytes, not
starting with `.`. The directory is created on the first attach. The scanners
(`Project::task_files`, rename's `task_paths`) read only `tasks/*.md`, so current and
older binaries both ignore `tasks/files/`.

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
  `--name` is required, or the command fails `invalid_attachment_name`. From the clipboard, the default is
  `clipboard-<UTC yyyymmddThhmmssZ>.<ext>`, with the extension taken from the chosen
  MIME type. An invalid name fails `invalid_attachment_name`. A name already present
  in the directory fails `attachment_exists`; nothing is overwritten.
- **Size:** at most `max_bytes + 1` bytes are read from any source. A larger file fails
  `attachment_too_large` with the size (or "more than") and the cap, before anything is
  written.
- **Clipboard:** runs `wl-paste --list-types`, then picks the first of `image/png`,
  `image/jpeg`, `image/webp`, `image/gif` on offer and reads it with
  `wl-paste --type <mime>`. Two failures:
  - `wl-paste` missing or exiting non-zero fails `clipboard_unavailable`, with its
    stderr;
  - no image type on offer fails `clipboard_no_image`, listing the offered types.
- **Write order:**
  1. Write the file atomically (temp file in the same directory, then rename).
  2. Append the note `attached: files/<id>/<name> (<n> bytes)` plus `: <caption>`
     when a caption is given.
  3. `save`.

  If the save fails, the file just written is removed and the error returned. A crash
  between steps 1 and 3 leaves a file with no note. `check` does not flag that case:
  the file is still a valid attachment.
- **Note:** a plain note, with no provenance stamp and no lifecycle marker.
- **Output:** `{"id", "file": {"name", "path", "bytes"}, "warnings"}`, where `path` is
  absolute. `--pretty` prints the path.

### detach

    tasks detach <id> <name> "<why>"

Removes one attachment and appends `detached: files/<id>/<name>: <why>`. When the
directory becomes empty, it is removed. A missing name fails `attachment_missing`.
The reason is required, as for `drop`. Git history keeps committed bytes, and the
pretty output says so. This is the one supported way to correct a wrong attach
without editing `tasks/` by hand.

### show and next

`ShowFields` gains `files: Vec<FileInfo>`, which `next` shares. It is omitted when
empty, per the output contract. `FileInfo` is `{name, path, bytes}`, sorted by name,
with `path` absolute under the resolved project root. A cross-project `show` therefore
prints the other project's path, which is where the file is. `--pretty` adds a
`# files` section listing each path with a human size. A directory entry that is not
a valid attachment is not listed; `show` adds a warning naming it and `check` reports
it.

### check

A new pass reads `tasks/files/` directly:

| Kind | Level | Condition |
|---|---|---|
| `attachment_orphan` | error | `tasks/files/<x>` where `<x>` is not a valid id or `tasks/<x>.md` does not exist. This is drift, typically from a record renamed by hand in collision recovery. |
| `attachment_invalid` | warning | A task's directory holds a subdirectory, a symlink, or a file with an invalid name. |
| `attachment_too_large` | warning | A file is above `max_bytes`, because it was added by hand or the cap was lowered later. |

A stray regular file directly under `tasks/files/` is `attachment_orphan`. The pass
runs over every task, open or closed.

### feedback

No change. `feedback` takes no attachment option: a screenshot is project content,
and a report lands in another project's checkout, which may be public.

## Rename

Each task's directory moves with its record. The directory's name carries the
prefix; the files inside need no rewrite. The inventory records nothing new.
The directory state is observed at each step, so recovery stays a re-observation,
not a journal:

- **Move order, per entry:** write and verify the destination `.md`, then move
  `tasks/files/<src-id>` to `tasks/files/<dst-id>` with `std::fs::rename` (same
  filesystem, atomic), then remove the source `.md`. The loop's existing
  "source already gone" branch (`rename/mod.rs`, the `NotFound` arm) also completes a
  pending directory move. A crash between the directory move and the source removal
  therefore resumes correctly.
- **Refusal:** both `tasks/files/<src-id>` and `tasks/files/<dst-id>` present is
  refusal R5, alongside the existing digest refusals in
  `classify::refusal`. `snapshot::observe` gains a per-entry directory state, present
  or absent for each side.
- **Strays:** a directory under `tasks/files/` whose id is not in the inventory is a
  stray, as an uninventoried `.md` is now.
- **Uncommitted attachments:** these already block a fresh rename, because
  `uncommitted_task_files` runs `git status` over all of `tasks/`.
- **Adopt:** `rename --adopt` writes no synced files, so it needs no change. The
  directories arrive with the synced checkout.

## Id-collision recovery

`skills/tasks/SKILL.md` and the README gain one clause. When renaming the losing
record to a fresh id, move `tasks/files/<old>/` to `tasks/files/<new>/` too.
`check`'s `attachment_orphan` catches it if forgotten.

## Errors

These are new `Error` variants, with the codes that `kind()` returns:

- `invalid_attachment_name`
- `attachment_exists`
- `attachment_missing`
- `attachment_too_large`
- `clipboard_unavailable`
- `clipboard_no_image`

## JSON contract changes

- `show` and `next` gain an optional `files` array.
- `attach` and `detach` are new outputs.
- `check` gains three finding kinds.

None of these changes an existing field.

## Testing

End-to-end tests in `tests/cli.rs`:

- **attach:**
  - from a path and from stdin;
  - `--name` validation;
  - `attachment_exists`;
  - the cap at `max_bytes` and at `max_bytes + 1`, with a small configured cap;
  - the note text;
  - cross-project routing by prefix.
- **detach:** the note, and removal of the empty directory.
- **show:** the `files` field and the pretty section.
- **check:** all three finding kinds.
- **rename:**
  - a record with attachments;
  - a crash-injected resume between the directory move and the source removal
    (`TASKS_RENAME_STOP_AFTER`);
  - the refusal when both directories exist.

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

1. **Storage, config, and `attach`/`detach`**, including clipboard input and the
   errors.
2. **`show`/`next` `files` field and the `check` pass.**
3. **`rename`:** the directory move, the observation, the refusal, and the resume
   test.
4. **README and skill:** the command reference, collision recovery, and the size
   policy.

Pieces 2 and 3 depend on 1; piece 4 comes last.
