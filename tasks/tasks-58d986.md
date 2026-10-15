---
id: tasks-58d986
title: "Project colour surfaces: deferred review minors"
status: todo
priority: 3
size: s
complexity: low
process: direct
created: 2026-10-15T16:43:16Z
updated: 2026-10-15T16:43:16Z
depends: []
parent: tasks-1f05a7
tags: [pretty]
agent: claude-code/claude-opus-5-5
---

Why: the final re-review of tasks-68cb2b accepted the branch with three minor findings, deferred under the review rules.

Done when:
- groups warns once per broken project, not once per membership (src/commands/group.rs list); color_or_warning could skip a warning already present, as src/locations.rs does with warnings.contains, which also covers show's retired-alias case. Add a two-group case to project_colors_warn_instead_of_failing_on_another_projects_bad_config.
- The README (colour section, 'Pretty show, next and groups likewise…') says the warning appears in every output mode, JSON included, since describe and groups look colours up whatever the mode; or the lookup moves behind pretty-with-colour. The comfy-tables spec §5 'colours stay out of JSON' line is made consistent.
- The README sentence is narrowed: a dependency into a project with a malformed config still fails show (resolve_registered is strict, older behaviour), so the warning covers moved-to/from ids and groups prefixes only.
- Warning wording aligned with resolve's ('… its color is not reported') in src/resolve.rs color_or_warning.
