---
id: tasks-0005a6
title: Stamp gate notes with session provenance
status: todo
priority: 2
size: s
complexity: mid
process: direct
created: 2026-09-18T19:09:47Z
updated: 2026-10-02T14:37:27Z
depends: []
tags: [quick-add, cli, observability]
source: "mindful:thought:7b1b0c108103464388357acb293ad0ed"
agent: claude-code/claude-opus-5
---

Why: Flow gate notes currently use ordinary note text, so they omit the optional native session provenance already supported on lifecycle notes. Stage attribution must infer the session instead of using the producer's explicit key.

Done: Add an opt-in --stamp flag to tasks note <id> <text>. With it, reuse the existing provenance resolver and append helper to attach the same optional harness_session/harness_session_source pair used on lifecycle notes. Without it, preserve plain-note behavior exactly. Missing native input leaves both fields absent; invalid/conflicting input emits the existing name-only warning and omits the pair, without making provenance failure itself fatal. Ordinary claim/store/identity errors still follow note's existing behavior. Preserve text normalization, append ordering, stale-copy checks, claim heartbeat and ownership rules, lifecycle classification by generated text, and the existing reader/file/JSON schema. Arbitrary --stamp text is allowed; do not recognize gate prefixes or parse flow states. Existing ambiguity when user text equals a generated marker remains documented rather than solved through new metadata.

Approach and context: src/cli.rs::Command::Note and src/commands/mod.rs dispatch; src/commands/status.rs::note; src/commands/mod.rs::append_note and append_lifecycle_note; src/provenance.rs::resolve_from; Note's optional pair in src/model.rs and codec in src/format.rs. Stamped notes already round-trip in the installed reader contract; lifecycle writer landed in 8f13112. The local docs/plans/2026-09-17-lifecycle-provenance.md and README.md describe the pair and independent claim behavior. Keep explicit --stamp rather than automatic prefix recognition, so tasks remains ignorant of flow's vocabulary. Update note help, README.md, skills/tasks/SKILL.md, and the command reference when implementation lands.

Verification: Extend existing CLI provenance fixtures to check the same gate text with and without --stamp, Codex and Claude native input, agreeing Codex variables, missing input, and conflicts/invalid input with value-free warnings. Assert text survives show/edit/show, metadata is the existing pair only, no task status transition is generated, and claim behavior matches ordinary note. Reuse resolver tests for the complete input table and existing note ownership/store-failure regression coverage. Run just test-one --test cli <filter>, then just test-fast; reinstall after the CLI change.

Boundary: This scoped task supplies the CLI capability. Adoption by flow and stage-cost consumers is separate owner work; their code was not inspected in this fixed-checkout pass. Stamping cannot repair conflicting worker native identities. Related evidence and its existing investigation: docs/notes/2026-09-30-delegated-identity-brief.md.

Source availability: mindful --json show 7b1b0c108103464388357acb293ad0ed returned no matching thought; the captured body and source reference below are preserved.

Original report:
Lifecycle markers (started, resumed, parked, done) carry harness_session provenance; plain notes do not. The flow skill records every state transition as a plain note with a gate: prefix, so a stage boundary has a timestamp but no session id, and per-stage cost attribution (obs-a6c7d4) has to infer the session from the nearest marker. Give a note a way to carry the same provenance: a --stamp flag on note, or a small set of recognised prefixes, whichever keeps the reader contract intact. Do not interpret gate text; tasks stays ignorant of flow's states.

Consumers use generated text, not the presence of provenance fields, to identify transitions, so stamping a user note must not make it look like a marker.

Source: mindful:thought:7b1b0c108103464388357acb293ad0ed

## Notes

- 2026-10-02T14:37:26Z (main): scope: scoped; opt-in note --stamp reuses the existing native provenance pair and warnings; plain notes and claim identity stay independent; P2/s/mid/direct; original source retained but lookup returned no match; brief: docs/notes/2026-09-30-delegated-identity-brief.md
