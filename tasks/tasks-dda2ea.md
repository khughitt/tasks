---
id: tasks-dda2ea
title: Accept prefix-free task IDs for the current project
status: todo
priority: 2
size: m
complexity: mid
process: direct
created: 2026-10-01T16:27:45Z
updated: 2026-10-01T16:32:16Z
depends: []
tags: [cli]
agent: codex
---

Why: inside the current project, accept both tasks show 2578c3 and tasks show tasks-2578c3, consistently across CLI task-ID inputs.

Assessment: feasible with bounded complexity. TaskId::parse already requires a full prefix plus exactly six lowercase hex digits, and commands::parse_id is the shared user-input boundary. Cross-project hex collisions cause no ambiguity when a bare suffix always expands to the original current project's prefix. No registry-wide search, abbreviated suffix matching, or alternate stored-ID representation is needed.

Done:
- Accept exactly six lowercase hex digits as shorthand for the project located from the invocation's cwd or -C, including subdirectories and worktrees.
- Resolve that local prefix before ID-directed routing. Keep it fixed for every ID argument in the invocation: changing the command's destination to a foreign project must not change what a bare suffix means.
- Apply the rule consistently to positional IDs and ID-valued flags, including root, tree, lifecycle/note/edit/attachment commands, --parent filters, add/edit --parent and --depends, dep --on/--rm, and feedback --recur.
- Explicit --project changes the command's destination or read scope, not the meaning of shorthand. A local shorthand outside that scope follows existing not-found/foreign-parent validation; use a full foreign ID to name another project's task.
- Outside a located project, reject bare suffixes with a clear typed error asking for a full ID, even with --project or --all-projects. Full IDs retain existing routing outside projects.
- Missing local tasks fail locally; never search other projects for a matching suffix. Reject shorter, uppercase, and otherwise malformed suffixes.
- Keep TaskId::parse and on-disk frontmatter, filenames, claims, references, and JSON strict and fully qualified. Preserve existing retired-prefix canonicalization, stale-checkout refusal, mutation locking, and record-home checks.
- Audit completion's separately parsed subject and ID candidate matching. Support local suffix fragments while keeping canonical full-ID suggestions and existing foreign-project completion.
- Document the rule and examples in README.md and skills/tasks/SKILL.md; update the multi-project design's CLI input description to distinguish shorthand from canonical stored IDs.
- Trailing-period normalization is tracked separately in tasks-1e32d6; compose it before shorthand expansion if that support is present. Neither task needs to wait on the other.

Where to look: src/commands/mod.rs::run/open_id_ctx/open_id_read_ctx/load/parse_id/apply_fields, src/commands/root.rs, src/commands/tree.rs, src/commands/dep.rs, src/commands/feedback.rs, src/filter.rs::TaskFilter::parse, src/complete.rs (subject parsing, destination, candidates), src/model.rs::TaskId::parse, src/repo.rs::Project::locate, and tests/cli.rs. Normalize at the CLI boundary with the original local context; do not infer defaults from a context that has already been rerouted.

Verification: focused CLI coverage for equivalent bare/full local reads and writes, all ID argument families, -C and subdirectory/worktree identity, a deliberate same-suffix collision across projects, foreign subject plus a local bare dependency, explicit project/all-project scopes, outside-project rejection, canonical persistence/JSON, aliases, and invalid inputs. Confirm full IDs preserve existing behavior. Run just test-one for focused cases and just test-fast before committing.

Process: direct; the assessment settles semantics and the implementation boundary. Size m, complexity mid because routing, filters, root, feedback, and completion all need the same original-local-project rule. This is broader than changing TaskId::parse but does not require a global lookup scheme.

Related in-flight work: tasks-9b0a2e changes Project::locate for linked checkouts. Reuse that API, avoid a second cwd detector, and coordinate touching the locator; the current task does not implement linked checkouts.

Original request: assess ambiguity, misrouting, and significant complexity before adding optional prefixes; recommend against support if costs outweigh convenience. Also assess trimming trailing periods from accidentally copied sentence-ending task IDs, perhaps as a separate ticket. Recommendation: proceed with exact local suffixes; reject global/partial matching; keep independent punctuation tolerance in tasks-1e32d6.

## Notes

- 2026-10-01T16:32:15Z (main): scope: scoped; P2/m/mid/direct; exact six-hex shorthand fixed to original cwd/-C project before routing, no registry suffix search, strict stored IDs; static assessment found bounded routing/filter/completion work; trailing-period trimming split to tasks-1e32d6
