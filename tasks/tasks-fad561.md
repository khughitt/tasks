---
id: tasks-fad561
title: "list --pretty: an option to nest children under their parent"
status: idea
priority: 2
created: 2026-10-23T18:05:10Z
updated: 2026-10-24T11:23:18Z
depends: [tasks-6fcb58]
parent: tasks-eceb1c
tags: [cli, pretty]
agent: claude-code/claude-fable-5-1
---

Idea from the user (2026-10-23): a nesting option on tasks list, so a goal's children render indented beneath it instead of as peers in the flat priority order, e.g.

    mm-1234    ....
    - mm-9228  ....
    - mm-2039  ....
    mm-9382    ....

Context: tasks tree --pretty already renders the open hierarchy indented, but it takes none of list's selection flags (status, priority, tag, --sort, --needs-review, --parked, ...). The gap is list's filtering and ordering combined with hierarchy.

Questions for scoping: a list flag (--nest / --tree) versus giving tree list's filters; what happens to a child whose parent the filter drops (show the parent dimmed as context, or hoist the child to the top level); whether sort order applies at each level or only to top-level rows; depth beyond two levels; and whether JSON changes shape (a nested view likely stays pretty-only to keep the JSON contract).

Scope handoff: [ref:brief] records verified list/tree behavior and the joint design choices. Wait on tasks-6fcb58 for a reviewed contract covering this request and its interaction with the other list display request.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-24-list-organization-brief.md — evidence and choices for list organization
