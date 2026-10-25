---
id: tasks-87eb2c
title: Settle the retrieval procedure's open cases found by the demonstrations
status: todo
priority: 2
complexity: low
process: direct
created: 2026-10-25T13:42:14Z
updated: 2026-10-25T13:42:14Z
depends: []
tags: []
source: tasks-4d39db
agent: claude-code
---

The scope skill's §2 retrieval procedure (skills/scope/SKILL.md) met every expected outcome in tasks-4d39db's demonstrations (report attached there), but the blind retriever had to rule on five cases the text does not cover: (1) a call refused by a hook before it ran (no result: exclude, or treat as unreadable?); (2) how to establish that a transcript captured stderr when Claude Code merges stderr into the tool result's stdout and leaves the stderr field empty; (3) where a Claude Code subagent's calls live (<session>/subagents/agent-*.jsonl under the parent's stamp; tooling gap filed as hq-4d77cc); (4) api-request-blob records that repeat tool_use blocks and must be deduplicated by tool_use id; (5) whether a same-argument call that starts after the note's at still fits the time. Done when the skill text settles each, keeping the spec's outcome names.
