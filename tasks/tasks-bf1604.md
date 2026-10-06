---
id: tasks-bf1604
title: "tasks check reports doc_missing for a closed task whose spec or plan moved to another project, so authority documents cannot follow their code without leaving copies behind"
status: dropped
priority: 2
created: 2026-10-04T18:11:01Z
updated: 2026-10-06T10:39:15Z
depends: []
tags: [feedback, gap, "from:tack"]
agent: claude-code/claude-fable-5-1
---

Moving two specs and two plans with their code to a new project left 13 done tasks in the old project pointing at them; check refused the deletion, so the old project keeps frozen copies. A closed task's document is a historical pointer; check could warn (or accept a recorded move) instead of erroring, or edit could repoint a closed task's spec/plan to a qualified path in another registered project.

## Notes

- 2026-10-06T09:45:47Z (main): scope: drop; current evidence covers this report; proposal: drop because 8861cd5 already exempts closed records from document-link drift; just test-one --test cli check_holds_only_open_records_to_their_doc_links passed on 2026-10-06; retain the original report; obtain a current binary/version capture if it recurs
- 2026-10-06T10:39:15Z (main): dropped
  provenance: {"harness_session":"codex:01a11097-1a86-7c90-878d-b71e466416fd","harness_session_source":"CODEX_SESSION_ID"}
- 2026-10-06T10:39:15Z (main): Closed-record document-link checks were fixed in 8861cd5; current focused regression passes. Historical show failure remains tracked separately in tasks-e068a2.
  provenance: {"harness_session":"codex:01a11097-1a86-7c90-878d-b71e466416fd","harness_session_source":"CODEX_SESSION_ID"}
