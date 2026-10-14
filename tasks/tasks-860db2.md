---
id: tasks-860db2
title: Judge claim liveness only from the PID view that recorded it
status: todo
priority: 1
size: m
complexity: mid
process: planned
created: 2026-10-14T19:05:20Z
updated: 2026-10-14T19:05:20Z
depends: []
parent: tasks-2fa8c6
tags: []
source: tasks-fe6c59
agent: claude-code/claude-opus-5-5
---

Defect found by tasks-fe6c59 (evidence: tasks/files/tasks-fe6c59/pidns-proof-evidence.json, cases prune-*, codex-*, ns-explicit-pid1, host-view-of-ns-pid).

`claims::liveness_with` reads `/proc/<claim.pid>/stat` in the caller's own PID view, and nothing records which view the pid came from. Every claim write calls `ClaimStore::prune_dead`. From a command in a child PID namespace with a private /proc (Codex workspace-write sandbox shape), a live host-view claim reads "pid gone". Any claim write there deletes it silently: no warning, no takeover note. The task record stays doing, and another session then starts it without --force. The reverse also holds. A pid recorded inside a namespace (explicit TASKS_SESSION_PID=1) reads as "reused" from the host, which takes it over without --force. Affected: relay claims, explicit-pid claims, CLAUDE_PID claims and sid:<pid> claims. Pid-less TTL claims are not affected. Reproduced with unshare and inside the genuine `codex sandbox` (0.162.1).

Exposure: a host whose Codex permission profile makes the claim store writable from the sandbox, while Claude Code claims carry CLAUDE_PID host pids. This repository's author host has both. Not yet observed in a real model session.

Design questions to settle: how a claim records its pid view (the pid-namespace inode from /proc/self/ns/pid; the initial namespace is 4026531836; inodes are reused once a namespace dies, so pair it with the start check); legacy claims without a view; whether a cross-view verdict falls to the TTL and is never pruned (the lean: yes, fail closed); whether to refuse TASKS_SESSION_PID from a non-initial namespace. Also whether a relay walk that ends Outside at a pid 1 in a non-initial namespace should refuse instead of silently falling to the native level (truncated ancestry; case ns-plain-private-proc). Tests: inject the view the way liveness_with injects stat.
