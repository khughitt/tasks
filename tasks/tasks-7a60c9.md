---
id: tasks-7a60c9
title: "show and doc have no read that never invokes a document provider: show fetches an external plan when a step is set, so a caller that must stay read-only cannot read a task's body, notes or stored links without possibly running the configured fetch command"
status: todo
priority: 2
size: s
complexity: low
process: direct
created: 2026-10-15T15:05:35Z
updated: 2026-10-15T18:27:27Z
depends: []
tags: [feedback, gap, "from:crew"]
agent: claude-code/claude-opus-5-5
---

Why: a read-only sweep (crew's) must read a task's body, notes and stored links without running host-configured commands. Today `show` fetches an external plan whenever a step is set (`src/commands/show.rs:126`, the `step_found` match, through `Resolver::read` → `doc_fetch::fetch`), and `doc` always fetches an external reference (`src/commands/doc.rs:17`). No flag turns this off. Local documents never spawn anything.

Done:
- `show --no-fetch`, `next --no-fetch` and `doc --no-fetch` (`src/cli.rs` `Show`/`Next`/`Doc`). With the flag, nothing spawns a `[docs.schemes]` command.
- Mechanism: a no-fetch mode on `Resolver` (`src/resolve.rs`) where `read` of an external reference returns `DocRead::Unavailable` with a new `UnavailableCause::FetchDisabled` ("fetch disabled by --no-fetch") without loading host config or spawning. `prefetch_external` is not on these paths and stays as it is.
- `show --no-fetch` and `next --no-fetch`: both describe through `show::describe` (`next` at `src/commands/list.rs:623`, for the one task it picks), so the flag is a parameter of `describe`. `spec_path`/`plan_path` come back as stored. For an external plan with a step, `step_found` is omitted for open and closed tasks alike, with no warning: a skipped fetch says nothing about availability, so the closed-task `false` from the design spec must not apply. Local plans still resolve `step_found` as they do now. The JSON shape is unchanged (`step_found` is already optional).
- `doc --no-fetch`: an external reference fails as `doc_not_found` with the FetchDisabled detail (`DocRead::required`, `src/resolve.rs:46`). A local plan or spec prints as it does now.
- `skills/tasks/SKILL.md` and the external-references design spec (`docs/specs/2026-10-11-external-document-references-design.md`, the show/describe section near line 320) name the flag.

Check: in `tests/external_docs.rs`, configure a stub provider whose command writes a sentinel file. Assert that the sentinel is absent after `show --no-fetch` on an open and a closed stepped external-plan task (`step_found` absent, no warning), after `next --no-fetch` picking an open stepped external-plan task (same expectation), and after `doc --plan --no-fetch` (error `doc_not_found`). Also assert that `doc --spec --no-fetch` on a local spec prints its body, and that plain `show` still writes the sentinel (the control).

Out of scope: `check` fetches by design. `next` was added to the scope at the user's request after the original ask, which named only `show` and `doc`.

Source: crew feedback. Original ask: "Wanted: a flag (e.g. --no-fetch) on show and doc. With it, external references come back as stored, step_found is omitted, doc reports an external reference as unavailable, and no provider command is ever spawned. Local documents still read. Use: a read-only sweep that must not run host-configured commands. Expected: a host with [docs.schemes] configured can be read without spawning the command; test with a provider that writes a sentinel file."

## Notes

- 2026-10-15T18:16:19Z (main): scope: scoped; body rewritten with mechanism (Resolver no-fetch mode, UnavailableCause::FetchDisabled), step_found omitted for open and closed tasks, doc fails doc_not_found, sentinel-provider test; next kept out of scope; checked against main 52346fc
- 2026-10-15T18:27:27Z (main): scope: next --no-fetch added at user request; flag is a parameter of show::describe, test covers next; size and complexity unchanged
