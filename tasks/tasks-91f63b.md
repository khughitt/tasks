---
id: tasks-91f63b
title: "External document references: scheme-prefixed plan/spec, fetched per scheme from host config"
status: doing
priority: 1
size: l
complexity: high
process: planned
owner: main
created: 2026-10-10T23:02:26Z
updated: 2026-10-11T09:51:00Z
started: 2026-10-11T09:51:00Z
depends: []
tags: []
source: "hq:docs/specs/2026-10-10-agent-docs-collection-design.md"
agent: claude-code/claude-fable-5-1
---

Implements §4 of the hq spec (spec round 4 accepted): plan/spec values whose first path segment contains a colon are external references (`mindful:thought:<32 hex>`); parse succeeds without scheme config; the host config `[docs.schemes] <scheme> = "<command>"` names the fetcher, run as `sh -c "<cmd> \"\$1\"" -- <rest>` returning `{"id","body"}` JSON on stdout (capped, 30 s); one `Resolver::read(kind, value)` behind step_exists, check reads and historical plans; `tasks doc <id> --plan|--spec` prints the body; `tasks list --plan <value>` exact filter with rows carrying `step`; errors Config/DocNotFound, check warning `doc_unreachable`, show warns on an unavailable historical plan; tests per §4.6. The resolver is a design decision in this project's terms, hence planned. Install the binary on both hosts.

## Notes

- 2026-10-11T09:51:00Z (main): started
  provenance: {"harness_session":"codex:01a12a32-51fc-79a0-b2db-a72d9a089cc1","harness_session_source":"CODEX_THREAD_ID"}
