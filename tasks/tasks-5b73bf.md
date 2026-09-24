---
id: tasks-5b73bf
title: Derive task kinds from curation passes and write the kinds section of the curate skill
status: idea
priority: 2
created: 2026-09-09T03:01:18Z
updated: 2026-09-24T13:35:22Z
depends: []
tags: [curation, skill]
---

After a handful of curate passes, cluster what was seen into a small set of task kinds (guess: defect, decision-needed idea, goal, plan step, feedback; the last two are already templated by plan headings and the feedback command). Each kind states the two or three elements a body must answer, as prose in skills/curate/SKILL.md, not headings to fill in. No CLI validation, no new field, no kind tag until the kinds prove stable.

## Notes

- 2026-09-09T10:33:04Z (design/curation): the curate skill's write path (revalidate, edit, curate: note) has not yet run against a real task: the first pass drew an empty pool. The first non-empty draw doubles as that acceptance test.
- 2026-09-09T11:36:50Z (main): first real pass ran 2026-09-09 on three tasks records (two refined, one decision); kinds seen so far: a scoped defect, a decision-needed idea, an unscoped idea with open questions
- 2026-09-24T13:35:22Z (main): Pass 3 (2026-09-24) kinds: a feedback report whose premise an earlier commit resolved (settled only by reproducing on a scratch project; left a residual error-message fix), a scoped todo whose body predated a precedent that landed later, and an idea whose figures went stale while the thing it measures grew.
