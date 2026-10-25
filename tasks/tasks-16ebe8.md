---
id: tasks-16ebe8
title: Deadline cleanup tests can panic before a child publishes its pid under scheduler contention
status: todo
priority: 2
size: s
complexity: mid
process: direct
created: 2026-10-25T11:54:58Z
updated: 2026-10-25T16:49:09Z
depends: []
tags: [feedback, friction, "from:tasks"]
agent: codex
---

Why: deadline-cleanup tests must establish that the child whose cleanup they assert actually started. At 31e12f6, src/doc_fetch.rs::tests::timeout_cleans_up_child_and_pipe_holder and escaped_pipe_holder_cannot_extend_deadline both start a 150 ms deadline before launching a shell, then unconditionally read/parse its pid file. The escaped-child shell waits for that file internally, but the parent deadline can kill the shell before publication. The escaped fixture also has an independent 800 ms stop timer. This code reading supports the reported startup race; host load and the original failure were not reproduced in this scope pass.

Done: make these two fixtures establish child readiness under a separate bounded startup check before measuring the deadline/cleanup behavior, using test-only synchronization and the existing local fixture structure. Keep the production deadline covering launch and pipe reads as [ref:contract] requires; do not add a public timeout control, merely increase arbitrary sleeps, or silently skip a case when its pid file is absent. Preserve both budget_limited modes, the deadline/error assertions, group-child termination checks, and bounded cleanup of the deliberately escaped child even on assertion failure. Exercise delayed startup deterministically without requiring a busy host; distinguish a bounded startup failure from a cleanup failure with a useful diagnostic. Keep any new subprocess fixture environment explicit and isolated. Verify through just test-one doc_fetch::tests (including the two affected cases) and just test-fast; record counts and ensure spawned children are reaped/stopped. Correctness and scope are established; fixture synchronization choices are bounded, so P2/s/mid/direct.

Where to look: src/doc_fetch.rs::tests (the two named tests, assert_not_running and shell_remains_unreaped_until_group_kill), fetch_inner and its existing cfg(test) observer seam; [ref:contract] sections 4.2 and 7 describe supervision and its checks; tests/external_docs.rs retains production deadline wiring. The affected tests last changed in 12beab6. No matching open cleanup/deadline task or dedicated handoff was found. No production behavior or CLI change is requested.

Original report (preserved):
The full-suite baseline passed. A subsequent unchanged-code fast-suite run failed two document-provider cleanup checks when they read a pid file absent after a 150 ms deadline. Observed host load was 135 on 32 CPUs; the fast suite still ran all cases. The checks should establish child startup separately from the deadline behavior they verify, so contention does not become a missing-file panic. No deadline or implementation change is included in the documentation work that exposed this.

## References

- contract (context, kind=spec): docs/specs/2026-10-11-external-document-references-design.md — existing provider deadline and process cleanup contract

## Notes

- 2026-10-25T16:48:52Z (main): scope: scoped; at 31e12f6 (doc_fetch.rs last changed in 12beab6), confirmed two 150 ms cleanup tests can reach unconditional pid-file reads without parent-side startup confirmation; bounded readiness synchronization and deterministic delayed-start coverage define P2/s/mid/direct work, preserving production deadlines and cleanup guarantees. Original report retained in full; no duplicate task, new brief, or implementation created. origin: filing unavailable because the record has no created occurrence or harness_session stamp; no source, parent, origin_session or other session pointer is recorded, so origin remains unknown. Reported load 135/32 CPUs and failure were not independently reproduced.
  provenance: {"harness_session":"codex:01a173f4-2451-7342-bfa4-ae5a150adde4","harness_session_source":"CODEX_THREAD_ID"}
