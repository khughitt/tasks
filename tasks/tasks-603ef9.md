---
id: tasks-603ef9
title: A tag dictionary and a tag-to-field promotion path
status: idea
priority: 2
created: 2026-09-11T20:44:15Z
updated: 2026-09-11T20:44:15Z
depends: []
tags: [obs]
---

Raised 2026-09-11. Mechanism: (1) a concise tag dictionary — the tags with a specific meaning in tasks, each with one line; (2) a new metadata idea starts life as a tag plus a dictionary entry, cheap and reversible; (3) a tag that proves useful is promoted to a proper field with validation, and the dictionary entry records the promotion. The park --reason vocabulary is the precedent for step 3 done directly.

First candidates, all friction-related, to try as tags before any field: gpu_intensive / resource_intensive / benchmarking (needs the machine to itself), requires_session_restart (niri, TTY), requires_solo_resource_ownership, requires_user_audit (or _feedback: an artifact the user must judge — overlaps park --reason review), est_time_hrs, est_mem_bytes. Estimates are values, not tags, so they may be the case that forces the field path early.
