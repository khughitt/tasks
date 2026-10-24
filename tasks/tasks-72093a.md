---
id: tasks-72093a
title: Reinstall merged bounded-summary CLI on this host
status: doing
priority: 2
size: s
complexity: low
process: direct
owner: main
created: 2026-10-24T16:10:36Z
updated: 2026-10-24T16:54:44Z
started: 2026-10-24T16:54:11Z
depends: [tasks-7c5a4b]
parent: tasks-65b733
tags: []
source: "tasks-e7424b:reinstall"
agent: codex
---

After Task 2 is reviewed and merged, resolve current tasks PATH target and Cargo install destination. Prepare documented locked install from registered main and obtain host-action approval before replacing the shared binary. Verify installed --summary support and bounded read; keep this task open if approval is pending. Independent of the trial gate; Task 3 needs verified reinstall.

## Notes

- 2026-10-24T16:54:11Z (main): started
  provenance: {"harness_session":"codex:01a16e34-fd27-7e51-b10c-c1250b33416d","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-24T16:54:44Z (main): Merged binary delivery: main 2c1ffb55f02b8c74f08fc184031a4d127e4ca868, feature 9cfb0bb. Verified PATH and resolved target are /home/keith/.cargo/bin/tasks; Cargo registry entry is tasks 0.4.0 from registered main; CARGO_HOME and CARGO_INSTALL_ROOT unset, no install.root override found. Proposed command from /mnt/ssd/Dropbox/tasks: cargo install --locked --path . . This replaces the shared PATH binary and requires user approval under accepted plan Task 2 Step 7 and the host-action rule. Do not run until approval. After approval verify PATH target, prime --help exposes --summary, bounded local read and fresh recovery; then record installed revision/results here and on tasks-7c5a4b. The real-consumer pilot and Task 3 adoption remain gated.
