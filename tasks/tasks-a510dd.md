---
id: tasks-a510dd
title: init warns when registering a project under a temp directory
status: dropped
priority: 3
created: 2026-09-04T10:47:45Z
updated: 2026-09-05T21:10:52Z
depends: []
tags: [cli]
---

tasks init writes a machine-global registry entry that outlives the directory it names. Two entries accumulated this way (an agent scratchpad and /tmp) before unregister existed.

Idea: warn at init when the root is under $TMPDIR, /tmp, or /var/tmp -- 'registered <prefix> under a temporary directory; tasks unregister <prefix> when done'. Warn only; never refuse, and never prune.

Not obviously worth it. Reachability cannot identify these entries -- both polluting entries read as live, with a config and tasks present -- so any check must be a path heuristic, which is approximate by nature. Documenting XDG_CONFIG_HOME=$(mktemp -d) for throwaway projects targets the same cause at zero cost and has landed; this is the backstop for when someone forgets. Revisit only if pollution recurs.

## Notes

- 2026-09-05T21:10:52Z (main): registry checked 2026-09-05: 16 entries, all real project roots, no /tmp, /var/tmp or scratchpad entries; the idea's own revisit-if-pollution-recurs condition is unmet and the documented XDG_CONFIG_HOME=$(mktemp -d) mitigation is holding
