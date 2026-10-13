---
id: tasks-b9b2e2
title: Attach several files in one tasks attach call
status: todo
priority: 2
size: s
complexity: mid
process: direct
created: 2026-10-10T13:22:07Z
updated: 2026-10-13T10:55:29Z
depends: []
tags: [feedback, gap, "from:mind6", cli]
agent: codex
---

Why: preserving several files on a task takes one `tasks attach` call each, so an agent saving a worktree's evidence bundled everything into one tar.gz instead. The bundle (4,643,655 bytes) went over the 2 MiB per-file cap and ended up in host-local storage with an attached manifest, which another host cannot restore. Every file inside fit the cap on its own (11 review screenshots of 293–616 KB, two docs already in the main checkout, two regenerable config files). The per-file cap stays (decided with the user, 2026-10-13): attachments hold small chosen evidence, and making the per-file path cheap removes the reason to bundle. The cleanup-rule side (sort ignored files instead of archiving them all) is lore feedback, not this task.

Done: `tasks attach <id> <path> <path>...` takes one or more file paths and stores each as its own attachment with its own `attached:` ledger note, all in one save. All-or-nothing: every name, size (cap), collision (including two paths with the same basename in one call) and storage-safety check passes before any file is written; a failing check writes nothing; a save failure removes every file this call wrote, as the single-file path does today. `--caption` applies to every file; `--name`, `-` (stdin) and `--clipboard` stay single-source and refuse more than one path. JSON: the output becomes `files: [{name, path, bytes}, ...]` for every call (one element for a single path), replacing `file`. Before changing it, search the registered projects (tasks-tui first) and the shipped skills for consumers of attach's `file` field; update them in the same change or note on this task which ones need it. Tests in tests/attachments.rs: several paths, a duplicate basename refused with nothing written, one over-cap file refused with nothing written, `--name` with two paths refused, the single-path case. Update README's attach reference, skills/tasks/SKILL.md, and the attachments spec's Commands section. `just test-one`, `just test-fast`, `just check`; `cargo install --path .`.

Where to look: src/commands/attach.rs::attach (validation before the write; the cleanup on save failure), src/cli.rs `Attach` (source becomes a list), src/output.rs AttachOut, src/attachments.rs, tests/attachments.rs, docs/specs/2026-09-28-task-attachments-design.md.

Context (preserved): mind6 commit 5f736e8 and mind6-9acb7a's notes. Before removing a worktree, an agent saved its ignored files as one archive, which was over the default per-file cap (2 MiB, `[attachments] max_bytes`), so it kept the archive in the host's work storage beside `.worktrees/` (`worktree-archives/`) and attached a 1,194-byte manifest with the location and SHA-256. Considered and rejected at scoping: raising mind6's cap (more bundles in git history for good); an outside, content-addressed store referenced by hash (rebuilds what Git LFS or git-annex does, which the attachments design ruled out; revisit only if single large files such as recordings or datasets keep coming up). A per-project size budget warning in `check` remains a possible separate idea.

Original report (preserved): Attachment size cap requires a separate archive and reference when preserving a larger set of cleanup artifacts.

## Notes

- 2026-10-13T10:49:07Z (main): scope: question; traced to mind6 5f736e8 (4.6 MB worktree-ignored archive over the 2 MiB per-file cap, kept in host storage with an attached manifest); open question whether bulk archives belong in git (raise mind6 max_bytes, drop) or need a documented/first-class out-of-repo reference
- 2026-10-13T10:52:18Z (main): Archive contents checked: 11 review screenshots from .impeccable/review/ (gitignored in mind6; 293-616 KB each, ~4.6 MB total), the 2026-09-18 TUI design+plan (identical copies already in mind6's main checkout, locally excluded), .codex/config.toml and an sdd .gitignore (regenerable). Every file fits the 2 MiB per-file cap; only the bundled tar exceeded it.
