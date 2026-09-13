---
id: tasks-470e8c
title: "shelved status: open, hidden from default views, entered only with a wake condition"
status: todo
priority: 2
size: m
complexity: mid
created: 2026-09-13T01:42:27Z
updated: 2026-09-13T02:04:17Z
depends: []
parent: tasks-019c60
tags: [cli]
spec: docs/specs/2026-09-12-scope-pass-design.md
---

New open status. Hidden from list, prime's roadmap, sample, and ready; counted separately in prime and projects. Entered only by tasks shelve <id> "<wake condition>", which writes the note; edit --status shelved and an editor save that changes a status to shelved refuse, while edits that keep an already-shelved status succeed. Refuses on a goal with unshelved open descendants, naming them. start and park on a shelved task refuse and name unshelve; shelve clears a park entry and its escalation. tasks unshelve returns it to idea. Stays open for dependencies and hierarchy: a goal with a shelved child cannot close, and check warns when a non-shelved open task depends on a shelved one. Also: sample's pending rule recognises scope: notes beside curate: ones. JSON contract: the status enum gains a value.
