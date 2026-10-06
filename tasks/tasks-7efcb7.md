---
id: tasks-7efcb7
title: "list --parked --waiting-on <user|agent>: work parked on the user across projects"
status: doing
priority: 2
size: s
complexity: low
process: direct
owner: main
created: 2026-10-06T11:16:10Z
updated: 2026-10-06T11:18:58Z
started: 2026-10-06T11:18:51Z
depends: [ops-d72df3]
tags: [feedback, gap, "from:ops"]
agent: claude-code/claude-opus-5-5
---

Why: a park records waiting_on and reason, and ready hides work waiting on the user, but nothing lists that work as a queue: list --parked mixes user- and agent-waiting parks, and prime shows them one project at a time. quiet is the only filtered view, and only for idle-host work. A person wants 'everything waiting on me, everywhere' in one command.

Done: list --parked (and search --parked, which shares ListArgs) takes --waiting-on <user|agent>, the same name and values as park's option. It requires --parked (a clap error otherwise, not an empty result) and narrows the parked rows to that waiting_on; an unresolved park still carries its park, so it filters too. Works with --all-projects, --group, --project and every existing filter; JSON shape unchanged. The pretty rows already show reason and next step, so tasks list --parked --waiting-on user --all-projects --pretty is the person's queue. ops cli.toml gains the option on list and search first (shared CLI vocabulary rule), then tasks runs vendored adopt and commits copy and implementation together. The tasks skill and README name the queue command.

Rejected: a separate 'tasks waiting' command like quiet: list --parked already renders the brief, so a filter is the smaller surface. A --reason filter is left out until asked for.

Where to look: src/cli.rs ListArgs (--parked), src/commands/list.rs list_parked, src/claims.rs WaitingOn::parse, src/commands/quiet.rs (filter on row.park), tests/cli.rs parked tests, ops cli.toml [[cli.tasks.commands]] list and search, skills/tasks/SKILL.md step 5.

Source report: no view lists work parked waiting on the user across projects; workaround was list --parked --all-projects piped through jq on .park.waiting_on.

## Notes

- 2026-10-06T11:18:45Z (main): scope: scoped; title and body rewritten to a --waiting-on filter on list/search --parked (rejected a separate command since list --parked pretty rows already show reason and next step); todo P2 s low direct
- 2026-10-06T11:18:51Z (main): started
  provenance: {"harness_session":"claude-code:5cee487d-b8b1-430e-8b20-7e4be6194daf","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
