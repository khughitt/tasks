---
id: tasks-eec7d5
title: "Fast-suite CLI fixtures intermittently reject init in non-Git temporary directories with a git-discovery error: 68 failures in one run, the named focused test and an unchanged full rerun then pass."
status: idea
priority: 2
created: 2026-10-08T21:02:30Z
updated: 2026-10-09T00:42:57Z
depends: []
tags: [feedback, friction, "from:tasks"]
agent: codex
---

## Notes

- 2026-10-09T00:42:57Z (fa0276-git-marker): Likely cause: tasks-fa0276's report says a host process recreates an empty .git in TMPDIR every minute or two, and git-discovery io on init in temp dirs is that failure; intermittent because the marker comes and goes. fa0276 makes the error name the marker path; it does not stop the flake.
