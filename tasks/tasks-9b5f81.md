---
id: tasks-9b5f81
title: init cannot re-point a prefix already registered to a stale directory; no unregister or --force
status: done
priority: 2
size: s
owner: feat/registry
created: 2026-09-04T02:43:34Z
updated: 2026-09-04T09:41:19Z
depends: []
tags: [feedback, gap, "from:beliefs", cli]
---

tasks init --prefix X refuses with 'already registered to <old path>' when the old path is a scratch directory; expected a way to re-register or remove the stale entry without hand-editing projects.toml

Triage: confirmed. Registry::register errors whenever the recorded root differs from the
new one, init has no override, and there is no unregister verb, so hand-editing
~/.config/tasks/projects.toml is the only route. init fails before Project::init, so
nothing is half-written.

Detecting staleness does not solve it. The obvious fix -- re-point automatically when the
registered root has no tasks/.config.toml -- would not fire for the reported case: a
scratch directory usually still exists and still holds a valid config, it is simply not
the project the prefix should mean any more. Two entries in this machine's registry are
exactly that shape and both still read as live. Auto-detection would also mistake an
unmounted drive for an abandoned project and silently reassign the prefix.

So the fix is an explicit override, matching the repo's explicit-over-defensive rule. Two
jobs, and the report names both: re-pointing a prefix at a new root, and removing a
prefix that should mean nothing. Decide between one flag on init, a separate registry
verb, or both, when this is picked up. Whichever lands, the current error should name the
remedy; today it states the conflict and stops.

## Notes

- 2026-09-04T09:21:04Z (main): triaged: confirmed gap, scoped P2/s; needs an explicit override, since staleness cannot be detected reliably
- 2026-09-04T09:41:19Z (feat/registry): init --force re-points a prefix and warns with the displaced root; tasks unregister drops one from anywhere; the conflict error now names both remedies
