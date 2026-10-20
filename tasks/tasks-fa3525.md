---
id: tasks-fa3525
title: "A scope pass's drop-as-covered verdict tests topical overlap, not each distinct ask: a blind audit of 71 coverage drops found 11 lost an ask that no task carries, 10 of which the filed report itself stated, and 9 more called a deliberate decline 'covered'"
status: idea
priority: 2
created: 2026-10-20T11:40:51Z
updated: 2026-10-20T12:03:36Z
depends: []
tags: [feedback, friction, "from:lore"]
agent: claude-code
---

Audit method and per-report table are attached. Proposal for the scope skill's drop verdict: list each distinct ask of the report and quote where a covering task commits to it; an ask no covering task holds is added to that task as a note, or the report stays open; a covering task that rejected or narrowed the ask makes the proposal 'declined by <id>' with its reason, not 'covered', so the reporter's ask carries an answer; an ask that relays the user's own words is named in the proposal. Context capture at filing (session stamps) would have helped in only a few of these cases: 68 of 71 filed reports already stated every ask.

## Notes

- 2026-10-20T11:41:06Z (main): attached: coverage-drop-audit.md (7089 bytes): Blind audit of 71 coverage drops, 2026-10-20: method, verdict counts, the 25 partial verdicts by category, sound list
- 2026-10-20T11:41:07Z (main): context: the audit ran in the session stamped here, at the user's request. Per-report evidence packets and judge outputs are in that session's scratch directory, not committed: they hold transcript excerpts.
  provenance: {"harness_session":"claude-code:737a5757-83af-4e31-9fdd-4ce70dd9e6f1","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-20T12:03:21Z (main): follow-up of the 10 lost verdicts (2026-10-20), each re-checked against current records, code and origin sessions: 4 upheld as lost (ops-a1e8b1 for prism only, lore-c21298, ops-fec58a, hq-872750), carried as stamped notes on ops-9c7dab, lore-da5aae, tasks-1dd2a0 and hq-624fb9. 6 not upheld: ns-80a711 was edited to hold ops-11a4a6's ask 13 min after the drop, outside the snapshot window; lore-4e50e7, filed later, holds flows-79e5a0's; tasks-b9b10e named and disproved tasks-2c7979's restamp; ops-be0fff delivered ops-075ea7 (its drop note said so); ops-3b4a6e was delivered (c2d3bc6, 86f434c) with its other half declined on an owner decision; ops-30e334's timing confirms ops-31ea9b's race premise. No new feedback filed. Each outcome is a note on the dropped report or its home.
- 2026-10-20T12:03:36Z (main): correction to the previous note: ns-80a711 was edited 7.5 min after ops-11a4a6's drop (drop 02:14:17Z, edit 02:21:50Z on 2026-10-17), 13 min after the drop proposal; still outside the five-minute snapshot window.
