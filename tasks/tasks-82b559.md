---
id: tasks-82b559
title: Stamp started and completed on task records and give park --waiting-on a small vocabulary
status: doing
priority: 2
size: m
owner: waiting-on
created: 2026-09-11T11:59:09Z
updated: 2026-09-11T12:13:58Z
depends: []
tags: [obs]
source: ops-2cb205
spec: docs/specs/2026-09-11-park-reason-and-stamps-design.md
---

The minimal telemetry change the friction diagnosis (ops docs/reports/2026-09-11-friction-diagnosis.md, §4 gaps 1 and 3, §5 item 4) asks for.

Today a record carries created and a mutable updated; completion time has to be inferred from the last note, and first-start is unrecoverable once the claim is released. Parks carry a free-text next step and an optional --waiting-on user, which cannot distinguish an aesthetic review of generated artifacts from a confirmation of the agent's own recommendation, a broken checkout, or an external dependency.

Scope:
- started: stamped by the first start of a record and never moved by later starts or takeovers; completed: stamped by done and cleared by a reopen, so a recompletion restamps it. Both visible in show/list JSON; check accepts records without them.
- park --waiting-on takes a small fixed vocabulary instead of a free value: user-review (an artifact the user must inspect and judge, e.g. art sheets), user-judgment (a decision only the user can make), user-approval (confirmation of a recommendation the agent already holds), environment (the checkout or machine is not runnable), dependency (another task or project), unknown. The value is stored with the park in the record, not only in the shared store, so history survives a resume; the park note keeps carrying the free-text next step.
- Keep the initial vocabulary small; refuse values outside it; no questionnaire beyond the one flag.

Out of scope: a full append-only transition history, session-to-task linkage, and any report over the stamps; those wait until these fields have been collected for a while. Design through brainstorming against this task before implementation.
