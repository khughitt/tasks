---
id: tasks-57425c
title: Design park-scoped resume inputs from the brief
status: todo
priority: 2
size: m
complexity: high
process: planned
created: 2026-10-17T16:47:13Z
updated: 2026-10-17T16:47:13Z
depends: []
parent: tasks-037538
tags: []
agent: claude-code
---

Question: How should `park --input <path>` record, check and report the files a parked task needs to resume, so the quiet/next/prime queues stop offering work whose inputs are gone?
Where to start: docs/notes/2026-10-16-park-resume-inputs-brief.md (option 2 and questions 2–3); src/commands/park.rs, ParkInfo in src/output.rs, the host-local park store in src/claims.rs; docs/specs/2026-09-13-quiet-queue-design.md §4; the quiet-park protocol in skills/tasks/SKILL.md step 5.
Bound: Design only, with spec and plan review before code. Settle how a volatile mount is decided (statfs filesystem type, a fixed path list, or both) and whether park warns or refuses; how an input is reported when read on a host other than the recorded one (unknown, not missing); re-park replacing and start clearing the list; stat-only checks on hot readers, hashing only on request. Add the guidance line to the quiet-park protocol. No record-wide artifacts field and no done/drop scratch listing; no idle detection or launching (quiet queue §8). The CLI rows start on ops cli.toml.
Expected result: A reviewed spec (amending quiet queue §4 or standing beside it) and plan, with executable child tasks filed after plan review; the brief updated with the decisions.
Ideas it wakes: On completion, run `tasks note tasks-2b5461 --no-reviewed "<finding>"` before this result's branch merges.
