---
id: tasks-13d935
title: "tasks projects fails with records_unpaired when run inside a checkout whose prefix is registered but not yet paired, instead of listing every project with that row marked"
status: idea
priority: 2
reviewed: true
reviewed_at: 2026-10-24T11:16:32Z
created: 2026-10-23T12:39:31Z
updated: 2026-10-24T11:16:32Z
depends: [tasks-34424b]
parent: tasks-72e3d7
tags: [feedback, friction, "from:ops"]
agent: claude-code/claude-opus-5-5
---

Action: a migration script on the second host, after the synced removal commit and before that host's init --records, ran tasks projects (and tasks where on another path) with its working directory inside the unpaired code checkout. Happened: the whole command exited 1 with the records_unpaired error envelope, so no row was readable. Expected: a cross-project listing does not depend on the cwd's project; the unpaired row is reported (reachable false, records from the registry) as it is from any other directory. Worked around by running every tasks call from a directory outside all projects.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-24-detached-migration-recovery-brief.md — incident evidence, contract tensions and investigation

## Notes

- 2026-10-24T11:16:32Z (main): scope: briefed; projects propagates records_unpaired through registry_warnings; the reported explicit where-on-another-path failure still needs reproduction; evidence: eedf9a9; waits on tasks-34424b; brief: docs/notes/2026-10-24-detached-migration-recovery-brief.md
