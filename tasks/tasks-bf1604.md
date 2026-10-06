---
id: tasks-bf1604
title: "tasks check reports doc_missing for a closed task whose spec or plan moved to another project, so authority documents cannot follow their code without leaving copies behind"
status: idea
priority: 2
created: 2026-10-04T18:11:01Z
updated: 2026-10-06T09:45:47Z
depends: []
tags: [feedback, gap, "from:tack"]
agent: claude-code/claude-fable-5-1
---

Moving two specs and two plans with their code to a new project left 13 done tasks in the old project pointing at them; check refused the deletion, so the old project keeps frozen copies. A closed task's document is a historical pointer; check could warn (or accept a recorded move) instead of erroring, or edit could repoint a closed task's spec/plan to a qualified path in another registered project.

## Notes

- 2026-10-06T09:45:47Z (main): scope: drop; current evidence covers this report; proposal: drop because 8861cd5 already exempts closed records from document-link drift; just test-one --test cli check_holds_only_open_records_to_their_doc_links passed on 2026-10-06; retain the original report; obtain a current binary/version capture if it recurs
