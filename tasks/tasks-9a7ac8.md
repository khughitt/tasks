---
id: tasks-9a7ac8
title: Large evidence manifests require manual multipart attachment and reconstruction
status: idea
priority: 2
created: 2026-10-24T19:19:16Z
updated: 2026-10-24T19:19:16Z
depends: []
tags: [feedback, gap, "from:ops"]
agent: codex
---

A frozen evidence manifest exceeded the configured attachment byte cap. Keeping every row and running normal checks requires manually splitting it into bounded JSON parts, writing a catalog with hashes, and verifying lossless reconstruction. A supported multipart attachment/export route would preserve exact evidence identity without raising the cap or hiding text in an archive.
