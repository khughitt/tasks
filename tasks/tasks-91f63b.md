---
id: tasks-91f63b
title: "External document references: scheme-prefixed plan/spec, fetched per scheme from host config"
status: todo
priority: 1
size: l
complexity: high
process: planned
created: 2026-10-10T23:02:26Z
updated: 2026-10-10T23:02:26Z
depends: []
tags: []
source: "hq:docs/specs/2026-10-10-agent-docs-collection-design.md"
agent: claude-code/claude-fable-5-1
---

Implements §4 of the hq spec (spec round 4 accepted): plan/spec values whose first path segment contains a colon are external references (`mindful:thought:<32 hex>`); parse succeeds without scheme config; the host config `[docs.schemes] <scheme> = "<command>"` names the fetcher, run as `sh -c "<cmd> \"\$1\"" -- <rest>` returning `{"id","body"}` JSON on stdout (capped, 30 s); one `Resolver::read(kind, value)` behind step_exists, check reads and historical plans; `tasks doc <id> --plan|--spec` prints the body; `tasks list --plan <value>` exact filter with rows carrying `step`; errors Config/DocNotFound, check warning `doc_unreachable`, show warns on an unavailable historical plan; tests per §4.6. The resolver is a design decision in this project's terms, hence planned. Install the binary on both hosts.
