# Task attachments brief

## Problem

A task can link a spec and a plan but cannot carry an image or any other file. UI and design work keeps needing one: a screenshot is the "before" reference, or the evidence that decides a bug's cause. Today the image stays wherever it landed (a downloads folder, a chat paste), and the task holds only a prose transcription of it. The attachment should travel with the record, across checkouts, worktrees, and hosts, and an agent reading the task should be able to open it.

## Current behaviour and evidence

- No attachment support. `Task` (`src/model.rs`) is `deny_unknown_fields`, so a new front-matter field is rejected by every older binary that reads the file. This is the same compatibility cost the provenance stamps paid (README, "Binaries older than the provenance writer").
- `spec` and `plan` are the only file links: repo-relative `.md` paths under `spec_dirs`/`plan_dirs`, validated by `Project::validate_docs` (`src/repo.rs`) and resolved in `src/resolve.rs`.
- The task scanners (`Project::task_files` in `src/repo.rs`, `task_paths` in `src/rename/inventory.rs`) read only `tasks/*.md` and skip everything else, so a directory under `tasks/` is invisible to current binaries. `tasks/rename/` already relies on this.
- `rename` renames `tasks/<id>.md` and rewrites references. It knows nothing about other per-id files. The id-collision recovery in the skill renames only the record.
- Two requests so far: the source thought for `tasks-e7a870` (its pasted screenshot was lost in transit), and the 2026-09-25 note on the same task (a phone screenshot that ruled out a hypothesis on sight).

## Constraints

- Records are managed only through the CLI; the tool has no database or index, and nothing is deleted (`drop` keeps history).
- Task files are committed with the code and sync across hosts through git (and, on some hosts, through a synced folder). Binary blobs grow every clone for good.
- Some registered repositories are public, and `feedback` files into one of them, so an attachment can publish project content.
- Cross-project writes route by the id's prefix (`note`, `edit`), and `attach` should too.
- Agents view an image by reading its path, so `show` must print a path that resolves from the reader's checkout.

## Alternatives

1. **A per-task directory found by convention, no schema change** (lean). `tasks attach <id> <file|-> [--name] [--caption]` copies the file into `tasks/files/<id>/`, and writes a note (`attached: files/<id>/<name>: <caption>`) so the timeline records when and why it arrived. `show` lists the directory, printing paths that resolve from the checkout. `check` warns on a directory with no task and an unreadable file. `rename` and collision recovery move the directory with the record. Older binaries ignore it entirely.
2. **An `attachments` front-matter list** (`[{path, caption}]`, repo-relative, anywhere in the repository), validated by `check` like `spec`/`plan`. It is explicit and can point at existing files, but it breaks older readers (`deny_unknown_fields`) and does not by itself give the file a home that travels with the record.
3. **References only** (a path or URL in a note or `source`). No new code, but it solves nothing: a downloads-folder path is gone on another host.

## Unanswered questions

- Discovered directory (1) or declared field (2)? Also, can an attachment name a file already in the repository without copying it? *Design task.*
- Size policy: a cap per file, recompression on attach (a PNG screenshot is 0.5–2 MB), or Git LFS for the attachments directory (git-lfs is installed on the main host, and no registered repository uses it yet)? *The user decides how much repository growth is acceptable; the design proposes the default.*
- Public repositories: should `attach` refuse, or warn, in a project marked public, and should `feedback` accept attachments at all? *Design task, with the user.*
- Input sources: file path and stdin at minimum; a clipboard source (`--clipboard`, Wayland `wl-paste`) is the fastest route for screenshots. *Design task.*
- Viewing: `show --pretty` prints paths. Rendering images inline in a terminal front end (kitty graphics) belongs to that front end's project, as a follow-up once the format lands. *Design task names the contract.*

## Proposed decomposition

- `tasks-e7a870` (the idea) waits on the design.
- Design task (this pass): write `docs/specs/2026-09-xx-task-attachments-design.md` from this brief, settle the questions above with the user, then split the implementation into children (storage and `attach`, `show`/`check`, `rename` and collision recovery, README and skill).
