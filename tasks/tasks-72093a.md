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
updated: 2026-10-24T17:10:12Z
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
- 2026-10-24T16:56:43Z (main): retained: {"host":"titan","state":"kept","verification":"verified","next_owner":"task:tasks-65b733","cleanup_trigger":"After verified shared reinstall and gated adoption disposition, recheck evidence preservation and run removal checks before authorized cleanup.","kind":"scratch","path":"$WORK_ROOT/tasks/tasks-65b733","purpose":"Canonical implementation and merged-tree verification evidence retained across approval and adoption.","manifest":"$WORK_ROOT/tasks/tasks-65b733/retained-manifest.json","sha256":"0a1f2c361ec8057e49bd47a23d235eb99b002d3dd879158cafa86d0647f17f5e"}
- 2026-10-24T16:56:44Z (main): retained: {"host":"titan","state":"kept","verification":"verified","next_owner":"task:tasks-65b733","cleanup_trigger":"After verified shared reinstall and gated adoption disposition, recheck evidence preservation and run removal checks before authorized cleanup.","kind":"evidence","path":"$WORK_ROOT/tasks/tasks-65b733/merged-fast.log","purpose":"Merged main 2c1ffb5 test-fast verification: 1681 passed, 4 ignored, no failures, 14 binaries.","sha256":"0c3f31fcf7b52db5a792b89ba7fd9b890e720d3ce17782efe8bc7a9b86fd4444"}
- 2026-10-24T16:56:46Z (main): retained: {"host":"titan","state":"kept","verification":"verified","next_owner":"task:tasks-65b733","cleanup_trigger":"After verified shared reinstall and gated adoption disposition, recheck evidence preservation and run removal checks before authorized cleanup.","kind":"worktree","path":".worktrees/tasks-e7424b","purpose":"Merged design/tasks-e7424b branch retained with confined verified binary and execution evidence for approved reinstall/adoption; no shared pointer targets it."}
- 2026-10-24T16:56:48Z (main): retained: {"host":"titan","state":"kept","verification":"verified","next_owner":"task:tasks-65b733","cleanup_trigger":"After verified shared reinstall and gated adoption disposition, recheck evidence preservation and run removal checks before authorized cleanup.","kind":"evidence","path":"$WORK_ROOT/tasks/.worktrees/tasks-e7424b/.superpowers/sdd/2026-10-24-bounded-session-summary","purpose":"Execution ledger, tests and full independent review preserved for reinstall/adoption ownership.","manifest":"$WORK_ROOT/tasks/.worktrees/tasks-e7424b/.superpowers/sdd/2026-10-24-bounded-session-summary/retained-manifest.json","sha256":"8c35bc66c0db840edf341dfd5177ef9dbf19daeaefdd8a75b29c00ad70c4f05b"}
- 2026-10-24T16:56:52Z (main): parked (waiting on user, approval): After user approves replacing /home/keith/.cargo/bin/tasks, the agent runs cargo install --locked --path . from registered main, verifies PATH/help/bounded read/recovery, and records installed revision on this task and tasks-7c5a4b.
  provenance: {"harness_session":"codex:01a16e34-fd27-7e51-b10c-c1250b33416d","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-24T17:10:09Z (main): resumed
  provenance: {"harness_session":"codex:01a16e34-fd27-7e51-b10c-c1250b33416d","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-24T17:10:12Z (main): User approved the prepared shared reinstall in this session. Execute cargo install --locked --path . from registered main; approval covers replacing the verified Cargo PATH binary and installed-binary verification, with startup guidance still gated.
